//! In-house cost serving with an interchange-category predictor (architecture doc §5, §8, §9).
//!
//! At decide time we don't yet know the settlement report's `ic_category`, so we predict it from
//! features we *do* have — network, variant, issuer country, and the amount band — using a modal
//! lookup with back-off learned from history, then serve that **specific** fitted cluster's cost.
//! This is the §9 path: it prices e.g. a €60 AU debit as the "> AUD 50" tier (~58 bps) rather than
//! a blend across all tiers. When the fine path can't resolve (missing raw issuer country, unseen
//! combination), it gracefully falls back to the **coarse region blend** — the previous behavior —
//! and only then to the seed/hypersense sources. So this strictly improves precision without
//! losing coverage.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use masking::PeekInterface;

use crate::config::ClickHouseAnalyticsConfig;
use crate::decider::gatewaydecider::multi_objective::cluster_key::issuer_region;
use crate::logger;
// Shared with the rollup aggregator so decide-time bucketing and the stored `band` column (which is
// stamped by the same thresholds at ingestion) can never diverge.
use super::types::amount_band;

const REFRESH_INTERVAL: Duration = Duration::from_secs(300);
const QUERY_TIMEOUT: Duration = Duration::from_secs(60);
/// Minimum observations before a predictor level is trusted (mirrors the prototype's MIN_SUPPORT).
const MIN_SUPPORT: u64 = 20;

/// Amount-independent `{pct_bps, fixed}` cost for one cluster.
#[derive(Debug, Clone, Copy)]
struct ServingCost {
    pct_bps: f64,
    fixed: f64,
}

impl ServingCost {
    fn effective_cost_bps(&self, amount: f64) -> f64 {
        if amount > 0.0 {
            self.pct_bps + (self.fixed / amount) * 10_000.0
        } else {
            self.pct_bps
        }
    }
}

/// Everything served for one merchant: the coarse region blend (fallback), the fine per-category
/// clusters, and the category predictor's back-off tables.
#[derive(Default, Clone)]
struct MerchantModels {
    /// `connector|network|funding|currency|region` → blended cost (graceful fallback).
    coarse: HashMap<String, ServingCost>,
    /// `connector|network|variant|funding|issuer|currency|ic_category` → specific cluster cost.
    fine: HashMap<String, ServingCost>,
    /// Predictor back-off levels (most specific first): level-key → modal `ic_category`.
    predictor: Vec<HashMap<String, String>>,
    /// Manual per-connector blended-fee overrides (lowercase connector → flat cost). When present
    /// for a connector, it wins over the learned model at [`lookup`] — the merchant told us the
    /// contract rate, so every EV calculation on that connector uses it.
    overrides: HashMap<String, ServingCost>,
    /// Manual per-cluster overrides (fine_key → flat cost). Highest precedence: a surgical fee for
    /// one specific segment, checked before the connector override and the learned model.
    cluster_overrides: HashMap<String, ServingCost>,
}

impl MerchantModels {
    /// No served models at all — the merchant should be absent from the cache (e.g. after its last
    /// ingestion was deleted), not left as a stale entry. An override alone keeps the merchant
    /// present (an override-only connector like Stripe must still price).
    fn is_empty(&self) -> bool {
        self.coarse.is_empty()
            && self.fine.is_empty()
            && self.predictor.iter().all(|l| l.is_empty())
            && self.overrides.is_empty()
            && self.cluster_overrides.is_empty()
    }
}

type Snapshot = HashMap<String, MerchantModels>;

fn cache() -> &'static RwLock<Arc<Snapshot>> {
    static CACHE: OnceLock<RwLock<Arc<Snapshot>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(Arc::new(HashMap::new())))
}

// ── keys ────────────────────────────────────────────────────────────────────────────────────

/// Canonicalize network aliases (report `mc`/`amex` vs router `mastercard`/`american express`).
fn normalize_network(network: &str) -> &str {
    match network {
        "mastercard" | "master" => "mc",
        "american express" | "americanexpress" => "amex",
        "diners club" | "dinersclub" => "diners",
        other => other,
    }
}


fn coarse_key(connector: &str, network: &str, funding: &str, currency: &str, region: &str) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        connector.to_lowercase(),
        normalize_network(&network.to_lowercase()),
        funding.to_lowercase(),
        currency.to_lowercase(),
        region.to_lowercase(),
    )
}

#[allow(clippy::too_many_arguments)]
fn fine_key(
    connector: &str,
    network: &str,
    variant: &str,
    funding: &str,
    issuer: &str,
    currency: &str,
    ic_category: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        connector.to_lowercase(),
        normalize_network(&network.to_lowercase()),
        variant.to_lowercase(),
        funding.to_lowercase(),
        issuer.to_lowercase(),
        currency.to_lowercase(),
        ic_category.to_lowercase(),
    )
}


/// Reconstruct the report's `variant` string from decide-time card attributes
/// (`visa` + `standard` + `debit` → `visastandarddebit`). A wallet is its own variant in the report
/// (`visa_applepay`), so it takes precedence over the network+program+funding form.
fn reconstruct_variant(network: &str, program: &str, funding: &str, wallet: &str) -> String {
    let net_l = network.to_lowercase();
    let net = normalize_network(&net_l);
    let w = wallet.to_lowercase();
    if w.contains("apple") {
        return format!("{net}_applepay");
    }
    if w.contains("google") {
        return format!("{net}_googlepay");
    }
    format!("{net}{}{}", program.to_lowercase(), funding.to_lowercase())
}

/// Predictor back-off level keys, most specific first. Channel-bearing levels come first (channel is
/// the strongest signal); when channel is unknown they simply miss and fall through to the
/// channel-less levels, which reproduce the previous behavior.
fn predictor_level_keys(
    network: &str,
    variant: &str,
    funding: &str,
    issuer: &str,
    band: &str,
    channel: &str,
) -> Vec<String> {
    let net_l = network.to_lowercase();
    let net = normalize_network(&net_l);
    let var = variant.to_lowercase();
    let fun = funding.to_lowercase();
    let iss = issuer.to_lowercase();
    let ch = channel.to_lowercase();
    vec![
        format!("0|{net}|{var}|{iss}|{ch}|{band}"),
        format!("1|{net}|{var}|{iss}|{ch}"),
        format!("2|{net}|{var}|{ch}|{band}"),
        format!("3|{net}|{var}|{iss}|{band}"), // channel-less fallback
        format!("4|{net}|{var}|{iss}"),
        format!("5|{net}|{fun}|{iss}|{ch}|{band}"),
        format!("6|{net}|{fun}|{ch}|{band}"),
        format!("7|{net}|{fun}|{band}"),
        format!("8|{net}|{fun}"),
    ]
}
const PREDICTOR_LEVELS: usize = 9;

// ── lookup (hot path) ─────────────────────────────────────────────────────────────────────────

/// The in-house cost matched for a candidate, with the model behind it (for observability). Mirrors
/// the `par_clusters_ic.csv` columns. `variant` / `issuer` / `ic_category` are set only on the fine
/// (category-predicted) path; the coarse blend leaves them `None`.
#[derive(Debug, Clone)]
pub struct InhouseMatch {
    /// Amount-adjusted cost for this transaction (what EV ranks on).
    pub effective_bps: f64,
    pub pct_bps: f64,
    pub fixed: f64,
    pub brand: String,
    pub currency: String,
    pub variant: Option<String>,
    pub issuer: Option<String>,
    pub ic_category: Option<String>,
}

/// Look up an in-house cost at decide time. Tries the fine, category-predicted cluster first, then
/// the coarse region blend; returns `None` when neither covers the key (caller falls back to
/// seed/hypersense). `issuer` is the raw ISO country when known (for the fine path); `region` is the
/// bucketed pricing region (for the coarse fallback).
#[allow(clippy::too_many_arguments)]
pub fn lookup(
    merchant_id: &str,
    connector: &str,
    network: &str,
    funding: &str,
    program: &str,
    currency: &str,
    issuer: &str,
    region: &str,
    channel: &str,
    wallet: &str,
    amount: f64,
) -> Option<InhouseMatch> {
    let snapshot = cache().read().ok()?.clone();
    let m = snapshot.get(merchant_id)?;

    // Resolve the fine cluster key once (needs a raw issuer + a predicted interchange category).
    // Reused for both the highest-precedence cluster-override check and the learned fine-model
    // lookup, so the two can never disagree on which cluster this transaction is.
    let fine = if issuer.is_empty() {
        None
    } else {
        let variant = reconstruct_variant(network, program, funding, wallet);
        let band = amount_band(amount);
        predict_category(m, network, &variant, funding, issuer, band, channel).map(|cat| {
            let key = fine_key(connector, network, &variant, funding, issuer, currency, &cat);
            (key, variant, cat)
        })
    };

    // 1. Cluster override — the merchant set a fee for this exact segment (including its card
    //    program). Most specific, wins over everything (connector override + learned model).
    if let Some((key, variant, cat)) = &fine {
        if let Some(cost) = m.cluster_overrides.get(key) {
            return Some(InhouseMatch {
                effective_bps: cost.effective_cost_bps(amount),
                pct_bps: cost.pct_bps,
                fixed: cost.fixed,
                brand: normalize_network(&network.to_lowercase()).to_string(),
                currency: currency.to_uppercase(),
                variant: Some(variant.clone()),
                issuer: Some(issuer.to_uppercase()),
                ic_category: Some(cat.clone()),
            });
        }
    }

    // 2. Connector override: the merchant gave us this connector's blanket contract rate, so use it
    //    flat for every transaction not covered by a cluster override above. `connector` is already
    //    lowercased by the caller; lowercase again defensively so the key always matches.
    if let Some(cost) = m.overrides.get(&connector.to_lowercase()) {
        return Some(InhouseMatch {
            effective_bps: cost.effective_cost_bps(amount),
            pct_bps: cost.pct_bps,
            fixed: cost.fixed,
            brand: normalize_network(&network.to_lowercase()).to_string(),
            currency: currency.to_uppercase(),
            variant: None,
            issuer: None,
            ic_category: None,
        });
    }

    // 3. Learned fine model: serve the specific fitted cluster.
    if let Some((key, variant, cat)) = &fine {
        if let Some(cost) = m.fine.get(key) {
            return Some(InhouseMatch {
                effective_bps: cost.effective_cost_bps(amount),
                pct_bps: cost.pct_bps,
                fixed: cost.fixed,
                brand: normalize_network(&network.to_lowercase()).to_string(),
                currency: currency.to_uppercase(),
                variant: Some(variant.clone()),
                issuer: Some(issuer.to_uppercase()),
                ic_category: Some(cat.clone()),
            });
        }
    }

    // 4. Fallback: the coarse region blend (previous behavior) — no single variant/issuer/category.
    let key = coarse_key(connector, network, funding, currency, region);
    m.coarse.get(&key).map(|cost| InhouseMatch {
        effective_bps: cost.effective_cost_bps(amount),
        pct_bps: cost.pct_bps,
        fixed: cost.fixed,
        brand: normalize_network(&network.to_lowercase()).to_string(),
        currency: currency.to_uppercase(),
        variant: None,
        issuer: None,
        ic_category: None,
    })
}

/// Predict the interchange category by trying each back-off level, most specific first.
fn predict_category(
    m: &MerchantModels,
    network: &str,
    variant: &str,
    funding: &str,
    issuer: &str,
    band: &str,
    channel: &str,
) -> Option<String> {
    let keys = predictor_level_keys(network, variant, funding, issuer, band, channel);
    for (i, key) in keys.iter().enumerate() {
        if let Some(table) = m.predictor.get(i) {
            if let Some(cat) = table.get(key) {
                return Some(cat.clone());
            }
        }
    }
    None
}

// ── refresh (background) ──────────────────────────────────────────────────────────────────────

pub fn spawn(clickhouse: ClickHouseAnalyticsConfig) {
    tokio::spawn(async move {
        logger::info!(
            tag = "cost_serving",
            "in-house cost serving refresh started; interval {:?}",
            REFRESH_INTERVAL
        );
        let mut ticker = tokio::time::interval(REFRESH_INTERVAL);
        loop {
            ticker.tick().await;
            match refresh(&clickhouse).await {
                Ok(n) => logger::info!(
                    tag = "cost_serving",
                    "refreshed in-house cost models: {} merchant(s)",
                    n
                ),
                Err(e) => logger::warn!(tag = "cost_serving", "refresh failed: {}", e),
            }
        }
    });
}

/// Latest GOOD clusters (per merchant/connector/account snapshot), for the coarse blend and the
/// fine per-category table. Per-country weighted numerators so we finish region bucketing here.
/// `{merchant_filter}` / `{merchant_filter_sub}` are replaced with a `merchant_id = {merchant:String}`
/// predicate for a single-merchant refresh (cheap, scans only that merchant — including the
/// `max(report_date)` subquery), or with `""` for the periodic global rebuild.
const COST_SQL: &str = r#"
SELECT
    merchant_id, connector, card_network, variant, funding, issuer_country, currency, ic_category,
    sum(pct_bps * gross_sum) AS pct_num,
    sum(fixed * gross_sum)   AS fixed_num,
    sum(gross_sum)           AS w
FROM __DB__.cost_fee_model FINAL
WHERE verdict = 'GOOD' AND gross_sum > 0{merchant_filter}
  AND (merchant_id, connector, account, report_date) IN (
      SELECT merchant_id, connector, account, max(report_date)
      FROM __DB__.cost_fee_model{merchant_filter_sub} GROUP BY merchant_id, connector, account)
GROUP BY merchant_id, connector, card_network, variant, funding, issuer_country, currency, ic_category
FORMAT TSV
"#;

/// Per-(merchant, network, variant, funding, issuer, band, channel) category counts, for the
/// predictor. `channel` (pos/ecom) is the strongest disambiguator between in-person and online
/// interchange categories. `band` is a stored column of the rollup (stamped at ingestion by the same
/// `amount_band` thresholds this file uses at decide time); the €5 floor was applied at aggregation.
/// `{merchant_filter}` is a `WHERE merchant_id = {merchant:String}` for a single-merchant refresh,
/// or `""` for the global rebuild.
const PREDICTOR_SQL: &str = r#"
SELECT
    merchant_id, card_network, variant, funding, issuer_country, band, channel, ic_category,
    sum(n) AS c
FROM __DB__.cost_daily_stats FINAL
{merchant_filter}
GROUP BY merchant_id, card_network, variant, funding, issuer_country, band, channel, ic_category
FORMAT TSV
"#;

/// Rebuild the **entire** served-model cache from ClickHouse. Used by the periodic background ticker
/// (off the request path). `O(all merchants)` — for the inline post-ingest/-delete refresh prefer
/// [`refresh_merchant`], which touches only the affected merchant.
pub async fn refresh(cfg: &ClickHouseAnalyticsConfig) -> Result<usize, String> {
    refresh_inner(cfg, None).await
}

/// Rebuild **one merchant's** served models and merge the result into the cache, leaving every other
/// merchant untouched. This is what runs inline after an ingest or delete: the ClickHouse queries
/// scan only that merchant (including the `max(report_date)` subquery), turning the old ~2s global
/// rebuild into a small filtered read. If the merchant now has no models (its data was deleted), it
/// is removed from the cache rather than left stale.
pub async fn refresh_merchant(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
) -> Result<usize, String> {
    refresh_inner(cfg, Some(merchant_id)).await
}

async fn refresh_inner(
    cfg: &ClickHouseAnalyticsConfig,
    merchant: Option<&str>,
) -> Result<usize, String> {
    // Splice the merchant predicate into the queries (or clear the placeholders for a global rebuild).
    let (cost_sql, pred_sql) = match merchant {
        Some(_) => (
            COST_SQL
                .replace("{merchant_filter}", " AND merchant_id = {merchant:String}")
                .replace("{merchant_filter_sub}", " WHERE merchant_id = {merchant:String}"),
            PREDICTOR_SQL.replace("{merchant_filter}", "WHERE merchant_id = {merchant:String}"),
        ),
        None => (
            COST_SQL
                .replace("{merchant_filter}", "")
                .replace("{merchant_filter_sub}", ""),
            PREDICTOR_SQL.replace("{merchant_filter}", ""),
        ),
    };
    let cost_rows = query(cfg, &cost_sql, merchant).await?;
    let pred_rows = query(cfg, &pred_sql, merchant).await?;

    let mut snap: Snapshot = HashMap::new();

    // 1. Cost tables (coarse blend + fine per-category), volume-weighted.
    let mut coarse_acc: HashMap<String, HashMap<String, (f64, f64, f64)>> = HashMap::new(); // merchant -> key -> (pct_num, fix_num, w)
    let mut fine_acc: HashMap<String, HashMap<String, (f64, f64, f64)>> = HashMap::new();
    for line in cost_rows.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 11 {
            continue;
        }
        let (merchant, connector, network, variant, funding, issuer, currency, ic) =
            (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7]);
        let pct_num: f64 = f[8].trim().parse().unwrap_or(0.0);
        let fix_num: f64 = f[9].trim().parse().unwrap_or(0.0);
        let w: f64 = f[10].trim().parse().unwrap_or(0.0);
        if w <= 0.0 {
            continue;
        }
        let region = issuer_region(issuer);
        let ck = coarse_key(connector, network, funding, currency, &region);
        accumulate(coarse_acc.entry(merchant.to_string()).or_default(), ck, pct_num, fix_num, w);
        let fk = fine_key(connector, network, variant, funding, issuer, currency, ic);
        accumulate(fine_acc.entry(merchant.to_string()).or_default(), fk, pct_num, fix_num, w);
    }
    for (merchant, keys) in coarse_acc {
        let m = snap.entry(merchant).or_default();
        m.coarse = finalize(keys);
    }
    for (merchant, keys) in fine_acc {
        let m = snap.entry(merchant).or_default();
        m.fine = finalize(keys);
    }

    // 2. Predictor tables: accumulate category counts per back-off level, keep the modal category
    //    with >= MIN_SUPPORT total observations.
    let mut pred_acc: HashMap<String, Vec<HashMap<String, HashMap<String, u64>>>> = HashMap::new();
    for line in pred_rows.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 9 {
            continue;
        }
        let (merchant, network, variant, funding, issuer, band, channel, ic) =
            (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7]);
        let c: u64 = f[8].trim().parse().unwrap_or(0);
        if c == 0 {
            continue;
        }
        let levels = pred_acc
            .entry(merchant.to_string())
            .or_insert_with(|| vec![HashMap::new(); PREDICTOR_LEVELS]);
        for (i, key) in predictor_level_keys(network, variant, funding, issuer, band, channel)
            .into_iter()
            .enumerate()
        {
            *levels[i].entry(key).or_default().entry(ic.to_string()).or_insert(0) += c;
        }
    }
    for (merchant, levels) in pred_acc {
        let tables: Vec<HashMap<String, String>> = levels
            .into_iter()
            .map(|level| {
                level
                    .into_iter()
                    .filter_map(|(key, cats)| {
                        let total: u64 = cats.values().sum();
                        if total < MIN_SUPPORT {
                            return None;
                        }
                        cats.into_iter().max_by_key(|(_, n)| *n).map(|(cat, _)| (key, cat))
                    })
                    .collect()
            })
            .collect();
        snap.entry(merchant).or_default().predictor = tables;
    }

    // 3. Manual blended-fee overrides (Postgres, not ClickHouse). Attach them to the snapshot so
    //    `lookup` can prefer them. A single-merchant refresh loads just that merchant; the global
    //    rebuild walks the override-merchant index so override-only connectors (no ClickHouse data)
    //    still price. Failures here are logged but non-fatal — the learned models are still served.
    match merchant {
        Some(mid) => load_overrides_into(&mut snap, mid).await,
        None => match super::overrides::list_merchants().await {
            Ok(merchants) => {
                for mid in merchants {
                    load_overrides_into(&mut snap, &mid).await;
                }
            }
            Err(e) => logger::warn!(tag = "cost_serving", "override index load failed: {:?}", e),
        },
    }

    // Global rebuild → replace the whole cache. Single-merchant → merge just that merchant's entry
    // into the existing cache (clone-modify-swap under the write lock, so readers see one atomic
    // switch). An absent/empty result removes the merchant so stale models don't linger.
    match merchant {
        None => {
            let n = snap.len();
            if let Ok(mut guard) = cache().write() {
                *guard = Arc::new(snap);
            }
            Ok(n)
        }
        Some(mid) => {
            let models = snap.remove(mid);
            let mut guard = cache().write().map_err(|_| "serving cache poisoned".to_string())?;
            let mut merged: Snapshot = (**guard).clone();
            match models {
                Some(m) if !m.is_empty() => {
                    merged.insert(mid.to_string(), m);
                }
                _ => {
                    merged.remove(mid);
                }
            }
            let n = merged.len();
            *guard = Arc::new(merged);
            Ok(n)
        }
    }
}

/// Load a merchant's manual overrides and set them on its snapshot entry (creating the entry when
/// the merchant has overrides but no ClickHouse-derived models). Non-fatal on error.
async fn load_overrides_into(snap: &mut Snapshot, merchant_id: &str) {
    // Connector-level overrides (lowercase connector → flat cost).
    match super::overrides::list(merchant_id).await {
        Ok(list) if !list.is_empty() => {
            let overrides = list
                .into_iter()
                .map(|(connector, ov)| {
                    (
                        connector.to_lowercase(),
                        ServingCost { pct_bps: ov.pct_bps, fixed: ov.fixed },
                    )
                })
                .collect();
            snap.entry(merchant_id.to_string()).or_default().overrides = overrides;
        }
        Ok(_) => {}
        Err(e) => logger::warn!(
            tag = "cost_serving",
            "connector override load failed for {}: {:?}",
            merchant_id,
            e
        ),
    }

    // Cluster-level overrides, keyed by the same fine_key the lookup builds at decide time.
    match super::overrides::list_clusters(merchant_id).await {
        Ok(list) if !list.is_empty() => {
            let cluster_overrides = list
                .into_iter()
                .map(|c| {
                    let key = fine_key(
                        &c.dims.connector,
                        &c.dims.card_network,
                        &c.dims.variant,
                        &c.dims.funding,
                        &c.dims.issuer_country,
                        &c.dims.currency,
                        &c.dims.ic_category,
                    );
                    (key, ServingCost { pct_bps: c.pct_bps, fixed: c.fixed })
                })
                .collect();
            snap.entry(merchant_id.to_string()).or_default().cluster_overrides = cluster_overrides;
        }
        Ok(_) => {}
        Err(e) => logger::warn!(
            tag = "cost_serving",
            "cluster override load failed for {}: {:?}",
            merchant_id,
            e
        ),
    }
}

fn accumulate(map: &mut HashMap<String, (f64, f64, f64)>, key: String, pct_num: f64, fix_num: f64, w: f64) {
    let e = map.entry(key).or_insert((0.0, 0.0, 0.0));
    e.0 += pct_num;
    e.1 += fix_num;
    e.2 += w;
}

fn finalize(keys: HashMap<String, (f64, f64, f64)>) -> HashMap<String, ServingCost> {
    keys.into_iter()
        .filter(|(_, (_, _, w))| *w > 0.0)
        .map(|(k, (pn, fn_, w))| (k, ServingCost { pct_bps: pn / w, fixed: fn_ / w }))
        .collect()
}

async fn query(
    cfg: &ClickHouseAnalyticsConfig,
    sql: &str,
    merchant: Option<&str>,
) -> Result<String, String> {
    let sql = sql.replace("__DB__", &cfg.database);
    let mut req = client().post(cfg.url.trim_end_matches('/')).body(sql);
    // Bound as `param_merchant` for the `{merchant:String}` placeholder in a single-merchant refresh.
    if let Some(m) = merchant {
        req = req.query(&[("param_merchant", m)]);
    }
    if !cfg.user.is_empty() {
        req = req.basic_auth(&cfg.user, cfg.password.as_ref().map(|p| p.peek().clone()));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("clickhouse serving query failed ({status}): {text}"));
    }
    resp.text().await.map_err(|e| e.to_string())
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(QUERY_TIMEOUT)
            .build()
            .expect("failed to build clickhouse serving client")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_aliases_and_casing_map_to_one_key() {
        assert_eq!(
            coarse_key("adyen", "mastercard", "debit", "EUR", "eu"),
            coarse_key("adyen", "mc", "debit", "EUR", "eu"),
        );
        assert_eq!(
            coarse_key("ADYEN", "VISA", "DEBIT", "eur", "EU"),
            coarse_key("adyen", "visa", "debit", "EUR", "eu"),
        );
    }

    #[test]
    fn variant_reconstruction_matches_report() {
        assert_eq!(reconstruct_variant("VISA", "STANDARD", "DEBIT", ""), "visastandarddebit");
        assert_eq!(reconstruct_variant("MASTERCARD", "PREMIUM", "CREDIT", ""), "mcpremiumcredit");
        // A wallet is its own report variant.
        assert_eq!(reconstruct_variant("VISA", "STANDARD", "DEBIT", "APPLE_PAY"), "visa_applepay");
    }

    #[test]
    fn amount_bands() {
        assert_eq!(amount_band(15.0), "lo");
        assert_eq!(amount_band(40.0), "b50");
        assert_eq!(amount_band(60.0), "b100"); // the €60 AUD case → "> AUD 50" tier
        assert_eq!(amount_band(500.0), "hi");
    }
}
