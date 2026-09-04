//! Redis storage for sticky routing: per-customer connector success counts.
//!
//! Logical model: `merchant:customer -> { pm:pmt -> { connector -> success_count } }`, stored as
//! ONE Redis hash per (merchant, customer) with flattened `pm:pmt:connector` integer fields, so
//! HINCRBY stays atomic under the concurrent feedback tasks, listpack encoding keeps a customer
//! at ~250 bytes, and a single HGETALL serves the exact-combo lookup. Deliberately no
//! cross-combo fallback — transaction-level fallbacks already cover a missing combo.
//!
//! Eviction safety — sticky keys must never crowd other routing state out of Redis:
//! 1. every hash carries a sliding TTL, re-armed in the same MULTI as the write, so idle
//!    customers self-evict and every sticky key stays volatile (safe under `volatile-*`
//!    maxmemory policies);
//! 2. a per-customer combo cap prunes the lowest-count field instead of growing unbounded,
//!    which also keeps the hash under the listpack encoding threshold;
//! 3. a per-merchant admission budget (two-window creation counter) stops NEW customer hashes
//!    once the per-TTL-window budget is spent — existing customers keep updating, so the
//!    worst-case footprint is bounded up front instead of relying on Redis eviction.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::get_tenant_app_state;
use crate::logger;
use crate::redis::cache::findByNameFromRedis;

type RedisResult<T> = Result<T, error_stack::Report<redis_interface::errors::RedisError>>;

/// Hash key per (merchant, customer): `sticky_gw_{merchant_id}_{customer_id}`.
pub const STICKY_KEY_PREFIX: &str = "sticky_gw_";
/// Per-window new-customer counter: `sticky_gw_adm_{merchant_id}_{window_index}`.
const ADMISSION_COUNTER_PREFIX: &str = "sticky_gw_adm_";

/// 90 days — sliding "as long as the customer keeps paying" retention.
pub const DEFAULT_STICKY_KEY_TTL_SECS: i64 = 90 * 24 * 60 * 60;
/// Combo-field cap per customer; stays below hash-max-listpack-entries (128).
pub const DEFAULT_MAX_COMBOS_PER_CUSTOMER: usize = 30;
/// New customer hashes allowed per merchant per TTL window (~250B each => ~250MB/window ceiling).
pub const DEFAULT_MAX_NEW_CUSTOMERS_PER_WINDOW: i64 = 1_000_000;

/// Service-configuration overrides, read through the same cache chain as the SR score TTLs.
const SC_STICKY_KEY_TTL: &str = "STICKY_ROUTING_KEY_TTL";
const SC_MAX_COMBOS: &str = "STICKY_ROUTING_MAX_COMBOS_PER_CUSTOMER";

/// FeatureConf key gating sticky writes/reads per merchant — the ops kill switch.
pub const STICKY_ROUTING_FEATURE: &str = "sticky_routing_enabled";
/// NX guard key `sticky_gw_lock_{merchant}_{payment}`: at most one sticky write per payment
/// inside this window, deduping duplicate/retried feedback. Deliberately short — a lifecycle
/// status past the window re-increments the same succeeding connector, inflating magnitude
/// uniformly (ranking holds), which is cheaper than a 90-day lock key per payment.
pub const STICKY_WRITE_LOCK_TTL_SECS: i64 = 1800;

pub fn write_lock_key(merchant_id: &str, payment_id: &str) -> String {
    format!("sticky_gw_lock_{merchant_id}_{payment_id}")
}

fn sc_merchant_budget_key(merchant_id: &str) -> String {
    format!("STICKY_ROUTING_MAX_CUSTOMERS_{merchant_id}")
}

/// Outcome of a sticky write; `SkippedOverBudget` means the merchant's new-key budget is spent.
#[derive(Debug, PartialEq, Eq)]
pub enum StickyWriteOutcome {
    Recorded { new_customer: bool },
    SkippedOverBudget,
}

/// The nested view of one customer's hash: `"pm:pmt" -> connector -> success_count`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StickyData {
    pub combos: HashMap<String, HashMap<String, i64>>,
}

impl StickyData {
    /// Rebuild the nested map from raw HGETALL output; unparsable fields are skipped.
    pub fn from_raw(raw: HashMap<String, String>) -> Self {
        let mut combos: HashMap<String, HashMap<String, i64>> = HashMap::new();
        for (field, value) in raw {
            let (Some((pm, pmt, connector)), Ok(count)) =
                (decode_field(&field), value.parse::<i64>())
            else {
                continue;
            };
            combos
                .entry(format!("{pm}:{pmt}"))
                .or_default()
                .insert(connector, count);
        }
        Self { combos }
    }

    /// Connectors for the exact (pm, pmt) combo, highest success count first.
    pub fn connectors_for_combo(
        &self,
        payment_method: &str,
        payment_method_type: &str,
    ) -> Vec<(String, i64)> {
        self.combos
            .get(&combo_key(payment_method, payment_method_type))
            .map(|counts| sorted_desc(counts.iter().map(|(c, n)| (c.clone(), *n))))
            .unwrap_or_default()
    }
}

/// Key inside `routing_algorithm.metadata` holding this algorithm's sticky config.
pub const ALGO_METADATA_STICKY_KEY: &str = "sticky_routing";

/// Sticky config carried by the profile's active routing algorithm, so fork/activate/rollback
/// move it atomically with the rule. Strictly validated at save time; read leniently at
/// evaluate time — absent or malformed metadata means disabled.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StickyRoutingConfig {
    #[serde(default)]
    pub enabled: bool,
}

pub fn config_from_algorithm_metadata(metadata: Option<&serde_json::Value>) -> StickyRoutingConfig {
    metadata
        .and_then(|meta| meta.get(ALGO_METADATA_STICKY_KEY))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

/// Record one successful payment: HINCRBY the combo:connector field and re-arm the key TTL.
pub async fn record_success(
    merchant_id: &str,
    customer_id: &str,
    payment_method: &str,
    payment_method_type: &str,
    connector: &str,
) -> RedisResult<StickyWriteOutcome> {
    let app_state = get_tenant_app_state().await;
    let key = sticky_key(merchant_id, customer_id);
    let ttl = key_ttl_secs().await;

    let is_new_customer = !app_state.redis_conn.exists(&key).await?;
    if is_new_customer && !admit_new_customer(merchant_id, ttl).await {
        logger::warn!(
            action = "sticky_routing",
            merchant_id = merchant_id,
            "sticky write skipped: merchant new-customer budget exhausted for this window"
        );
        return Ok(StickyWriteOutcome::SkippedOverBudget);
    }

    let field = encode_field(payment_method, payment_method_type, connector);
    let new_count = app_state
        .redis_conn
        .hincrby_with_expire(&key, &field, 1, ttl)
        .await?;

    if is_new_customer {
        note_new_customer(merchant_id, ttl).await;
    }
    // First success on this combo:connector — enforce the field cap (drains any prior excess too).
    if new_count == 1 {
        prune_if_over_cap(&key, &field).await;
    }

    Ok(StickyWriteOutcome::Recorded {
        new_customer: is_new_customer,
    })
}

/// One HGETALL: the customer's full nested sticky map, or None if nothing is stored.
pub async fn read_sticky_data(
    merchant_id: &str,
    customer_id: &str,
) -> RedisResult<Option<StickyData>> {
    let app_state = get_tenant_app_state().await;
    let raw = app_state
        .redis_conn
        .hgetall_map(&sticky_key(merchant_id, customer_id))
        .await?;
    Ok((!raw.is_empty()).then(|| StickyData::from_raw(raw)))
}

/// Erase one customer's sticky state (merchant offboarding / data-erasure requests).
pub async fn delete_sticky_data(merchant_id: &str, customer_id: &str) -> RedisResult<()> {
    let app_state = get_tenant_app_state().await;
    app_state
        .redis_conn
        .delete_key(&sticky_key(merchant_id, customer_id))
        .await?;
    Ok(())
}

fn sticky_key(merchant_id: &str, customer_id: &str) -> String {
    format!("{STICKY_KEY_PREFIX}{merchant_id}_{customer_id}")
}

fn combo_key(payment_method: &str, payment_method_type: &str) -> String {
    format!(
        "{}:{}",
        canonical_dimension(payment_method),
        canonical_dimension(payment_method_type)
    )
}

fn encode_field(payment_method: &str, payment_method_type: &str, connector: &str) -> String {
    format!(
        "{}:{}",
        combo_key(payment_method, payment_method_type),
        sanitize(connector)
    )
}

fn decode_field(field: &str) -> Option<(String, String, String)> {
    let mut parts = field.splitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(pm), Some(pmt), Some(connector))
            if !pm.is_empty() && !pmt.is_empty() && !connector.is_empty() =>
        {
            Some((pm.to_string(), pmt.to_string(), connector.to_string()))
        }
        _ => None,
    }
}

// ':' is the field separator, so it can never appear inside a component.
fn sanitize(part: &str) -> String {
    part.trim().replace(':', "_")
}

// pm/pmt arrive from two sources (feedback payload verbatim; decide-time snapshot uppercased),
// so they are case-folded here or the same combo fragments across fields. The connector is NOT
// case-folded: the read path must compare it verbatim against the caller's eligible list.
fn canonical_dimension(part: &str) -> String {
    sanitize(part).to_uppercase()
}

fn sorted_desc(entries: impl Iterator<Item = (String, i64)>) -> Vec<(String, i64)> {
    let mut list: Vec<(String, i64)> = entries.collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list
}

/// Victims for the field cap: the `excess` lowest counts (name as tie-break), never the
/// just-written field. Draining the full excess (not one victim) means a hash that got over
/// the cap through concurrent writes or a config change converges back on the next trigger.
fn select_prune_victims(
    fields: &HashMap<String, i64>,
    protect: &str,
    excess: usize,
) -> Vec<String> {
    let mut candidates: Vec<(&String, i64)> = fields
        .iter()
        .filter(|(name, _)| name.as_str() != protect)
        .map(|(name, count)| (name, *count))
        .collect();
    candidates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(b.0)));
    candidates
        .into_iter()
        .take(excess)
        .map(|(name, _)| name.clone())
        .collect()
}

// Best-effort: a failed prune leaves the hash transiently over the cap; the next new-field
// write drains it. A concurrent writer's fresh count-1 field can be picked as a victim — rare,
// and self-healing since its next success recreates it.
async fn prune_if_over_cap(key: &str, protect_field: &str) {
    let app_state = get_tenant_app_state().await;
    let max_combos = max_combos_per_customer().await;
    let raw = match app_state.redis_conn.hgetall_map(key).await {
        Ok(raw) => raw,
        Err(err) => {
            logger::error!(
                action = "sticky_routing",
                "prune read failed for {key}: {err}"
            );
            return;
        }
    };
    if raw.len() <= max_combos {
        return;
    }
    let excess = raw.len() - max_combos;
    let counts: HashMap<String, i64> = raw
        .into_iter()
        .filter_map(|(field, value)| value.parse::<i64>().ok().map(|count| (field, count)))
        .collect();
    for victim in select_prune_victims(&counts, protect_field, excess) {
        if let Err(err) = app_state.redis_conn.hdel_field(key, &victim).await {
            logger::error!(
                action = "sticky_routing",
                "prune hdel failed for {key}: {err}"
            );
        }
    }
}

async fn key_ttl_secs() -> i64 {
    findByNameFromRedis::<f64>(SC_STICKY_KEY_TTL.to_string())
        .await
        .map(|v| v as i64)
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_STICKY_KEY_TTL_SECS)
}

async fn max_combos_per_customer() -> usize {
    findByNameFromRedis::<f64>(SC_MAX_COMBOS.to_string())
        .await
        .map(|v| v as usize)
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_MAX_COMBOS_PER_CUSTOMER)
}

async fn merchant_new_customer_budget(merchant_id: &str) -> i64 {
    findByNameFromRedis::<f64>(sc_merchant_budget_key(merchant_id))
        .await
        .map(|v| v as i64)
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_MAX_NEW_CUSTOMERS_PER_WINDOW)
}

fn window_index(ttl_secs: i64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now / (ttl_secs.max(1) as u64)
}

fn admission_key(merchant_id: &str, window: u64) -> String {
    format!("{ADMISSION_COUNTER_PREFIX}{merchant_id}_{window}")
}

// Creation-rate cap: current + previous window creations must stay under the budget. This bounds
// growth rate rather than exact live count — long-lived active customers outlive their window's
// counter, but those keys are earning their keep and still expire once the customer goes idle.
async fn admit_new_customer(merchant_id: &str, ttl_secs: i64) -> bool {
    let budget = merchant_new_customer_budget(merchant_id).await;
    let window = window_index(ttl_secs);
    let current = read_admission_counter(&admission_key(merchant_id, window)).await;
    let previous =
        read_admission_counter(&admission_key(merchant_id, window.saturating_sub(1))).await;
    current.saturating_add(previous) < budget
}

async fn read_admission_counter(key: &str) -> i64 {
    let app_state = get_tenant_app_state().await;
    // A missing counter (or a read error) reads as zero — admission stays best-effort.
    app_state
        .redis_conn
        .get_key::<i64>(key, "i64")
        .await
        .unwrap_or(0)
}

async fn note_new_customer(merchant_id: &str, ttl_secs: i64) {
    let app_state = get_tenant_app_state().await;
    let key = admission_key(merchant_id, window_index(ttl_secs));
    match app_state.redis_conn.increment_key(&key).await {
        // First increment created the counter — bound its lifetime to the two-window horizon.
        Ok(1) => {
            if let Err(err) = app_state
                .redis_conn
                .expire_key(&key, ttl_secs.saturating_mul(2))
                .await
            {
                logger::error!(
                    action = "sticky_routing",
                    "admission counter expire failed for {key}: {err}"
                );
            }
        }
        Ok(_) => {}
        Err(err) => {
            logger::error!(
                action = "sticky_routing",
                "admission counter incr failed for {key}: {err}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_encoding_roundtrips() {
        let field = encode_field("INTERAC", "RTP", "GIGADAT");
        assert_eq!(field, "INTERAC:RTP:GIGADAT");
        assert_eq!(
            decode_field(&field),
            Some(("INTERAC".into(), "RTP".into(), "GIGADAT".into()))
        );
        assert_eq!(decode_field("only:two"), None);
        assert_eq!(decode_field("a:b:"), None);
    }

    #[test]
    fn sanitize_keeps_separator_unambiguous() {
        let field = encode_field("PM:WITH:COLONS", "PMT", "CON");
        assert_eq!(
            decode_field(&field),
            Some(("PM_WITH_COLONS".into(), "PMT".into(), "CON".into()))
        );
    }

    #[test]
    fn pm_and_pmt_case_fold_but_connector_stays_verbatim() {
        // Payload-sourced ("interac"/"rtp") and snapshot-sourced ("INTERAC"/"RTP") writes
        // must land on the same field, or one customer's combo fragments.
        assert_eq!(
            encode_field("interac", "rtp", "gigadat"),
            "INTERAC:RTP:gigadat"
        );
        assert_eq!(
            encode_field(" INTERAC ", "RTP", "gigadat"),
            "INTERAC:RTP:gigadat"
        );
        let raw = HashMap::from([("INTERAC:RTP:gigadat".to_string(), "2".to_string())]);
        let data = StickyData::from_raw(raw);
        assert_eq!(
            data.connectors_for_combo("interac", "rtp"),
            vec![("gigadat".to_string(), 2)]
        );
    }

    #[test]
    fn from_raw_builds_nested_map_and_skips_junk() {
        let raw = HashMap::from([
            ("INTERAC:RTP:GIGADAT".to_string(), "3".to_string()),
            ("INTERAC:RTP:LOONIO".to_string(), "2".to_string()),
            ("INTERAC:BANK:PAYPER".to_string(), "5".to_string()),
            ("bad_field".to_string(), "1".to_string()),
            ("INTERAC:RTP:BROKEN".to_string(), "not_a_number".to_string()),
        ]);
        let data = StickyData::from_raw(raw);
        assert_eq!(data.combos.len(), 2);
        assert_eq!(data.combos["INTERAC:RTP"]["GIGADAT"], 3);
        assert_eq!(data.combos["INTERAC:BANK"]["PAYPER"], 5);
        assert!(!data.combos["INTERAC:RTP"].contains_key("BROKEN"));
    }

    #[test]
    fn combo_lookup_sorts_by_count_then_name() {
        let raw = HashMap::from([
            ("INTERAC:RTP:GIGADAT".to_string(), "3".to_string()),
            ("INTERAC:RTP:LOONIO".to_string(), "2".to_string()),
            ("INTERAC:RTP:APEX".to_string(), "3".to_string()),
        ]);
        let data = StickyData::from_raw(raw);
        assert_eq!(
            data.connectors_for_combo("INTERAC", "RTP"),
            vec![
                ("APEX".to_string(), 3),
                ("GIGADAT".to_string(), 3),
                ("LOONIO".to_string(), 2)
            ]
        );
        assert!(data.connectors_for_combo("CARD", "CREDIT").is_empty());
    }

    #[test]
    fn algorithm_metadata_config_parses_leniently() {
        let meta = serde_json::json!({ "sticky_routing": { "enabled": true } });
        assert_eq!(
            config_from_algorithm_metadata(Some(&meta)),
            StickyRoutingConfig { enabled: true }
        );
        // Absent metadata, absent key, and malformed values all read as disabled.
        assert!(!config_from_algorithm_metadata(None).enabled);
        assert!(!config_from_algorithm_metadata(Some(&serde_json::json!({}))).enabled);
        assert!(
            !config_from_algorithm_metadata(Some(&serde_json::json!({ "sticky_routing": "yes" })))
                .enabled
        );
        // Unknown fields (e.g. a legacy "fallback") fail strict parse — fail-closed to disabled.
        let meta = serde_json::json!({ "sticky_routing": { "enabled": true, "fallback": ["pm"] } });
        assert!(!config_from_algorithm_metadata(Some(&meta)).enabled);
    }

    #[test]
    fn prune_victims_are_lowest_counts_and_never_the_new_field() {
        let fields = HashMap::from([
            ("INTERAC:RTP:GIGADAT".to_string(), 3),
            ("INTERAC:RTP:LOONIO".to_string(), 1),
            ("CARD:CREDIT:STRIPE".to_string(), 1),
        ]);
        // Lowest count with name tie-break: CARD:CREDIT:STRIPE < INTERAC:RTP:LOONIO.
        assert_eq!(
            select_prune_victims(&fields, "INTERAC:RTP:GIGADAT", 1),
            vec!["CARD:CREDIT:STRIPE".to_string()]
        );
        // The full excess drains in one pass, lowest counts first.
        assert_eq!(
            select_prune_victims(&fields, "INTERAC:RTP:GIGADAT", 2),
            vec![
                "CARD:CREDIT:STRIPE".to_string(),
                "INTERAC:RTP:LOONIO".to_string()
            ]
        );
        // The just-written field is protected even when it has the lowest count.
        let fields = HashMap::from([("A:B:NEW".to_string(), 1), ("A:B:OLD".to_string(), 7)]);
        assert_eq!(
            select_prune_victims(&fields, "A:B:NEW", 1),
            vec!["A:B:OLD".to_string()]
        );
        // Excess larger than the candidate pool never panics or touches the protected field.
        assert_eq!(
            select_prune_victims(&fields, "A:B:NEW", 5),
            vec!["A:B:OLD".to_string()]
        );
    }

    #[test]
    fn window_math_is_stable() {
        assert_eq!(admission_key("m1", 42), "sticky_gw_adm_m1_42");
        // ttl <= 0 must not divide by zero.
        let _ = window_index(0);
    }
}
