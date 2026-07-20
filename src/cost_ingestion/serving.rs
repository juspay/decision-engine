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

#[derive(Debug, Clone, Copy)]
struct ServingSegment {
    cost: ServingCost,
    segment_idx: u16,
    amount_lo: f64,
    amount_hi: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, std::hash::Hash)]
struct PredictedIc {
    category: String,
    interchange_bps: String,
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
    coarse: HashMap<String, Vec<ServingSegment>>,
    /// `connector|network|variant|funding|issuer|currency|ic_category|ic_rate` → fitted segments.
    fine: HashMap<String, Vec<ServingSegment>>,
    /// Predictor back-off levels (most specific first): level-key → modal category/rate pair.
    predictor: Vec<HashMap<String, PredictedIc>>,
    /// Manual per-connector blended-fee overrides (lowercase connector → flat cost). When present
    /// for a connector, it wins over the learned model at [`lookup`] — the merchant told us the
    /// contract rate, so every EV calculation on that connector uses it.
    overrides: HashMap<String, ServingCost>,
    /// Manual per-cluster overrides (fine_key → flat cost). Highest precedence: a surgical fee for
    /// one specific segment, checked before the connector override and the learned model.
    cluster_overrides: HashMap<String, ServingCost>,
    /// Invoice-derived cost add-on per connector (lowercase connector → `{pct_addon_bps, fixed}`).
    /// Added on top of the **learned** fine/coarse model at [`lookup`] to recover the invoice-only
    /// fees the settlement report can't (flat per-txn processing/risk fees + amortized periodic
    /// fees). Deliberately *not* applied to the manual overrides above — those are all-in contract
    /// rates a merchant stated, already inclusive of everything.
    addons: HashMap<String, ServingCost>,
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
            && self.addons.is_empty()
    }
}

impl ServingCost {
    /// Layer an invoice add-on on top of a learned cost: the add-on's amortized periodic rate joins
    /// `pct_bps` and its flat per-txn fee joins `fixed`. The identity when `addon` is `None`.
    fn with_addon(self, addon: Option<&Self>) -> Self {
        match addon {
            Some(a) => Self {
                pct_bps: self.pct_bps + a.pct_bps,
                fixed: self.fixed + a.fixed,
            },
            None => self,
        }
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

fn coarse_key(
    connector: &str,
    network: &str,
    funding: &str,
    currency: &str,
    region: &str,
) -> String {
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
    interchange_bps: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        connector.to_lowercase(),
        normalize_network(&network.to_lowercase()),
        variant.to_lowercase(),
        funding.to_lowercase(),
        issuer.to_lowercase(),
        currency.to_lowercase(),
        ic_category.to_lowercase(),
        interchange_bps.to_lowercase(),
    )
}

#[allow(clippy::too_many_arguments)]
fn override_keys(
    connector: &str,
    network: &str,
    variant: &str,
    funding: &str,
    issuer: &str,
    currency: &str,
    ic_category: &str,
    interchange_bps: &str,
    segment_idx: Option<u16>,
) -> Vec<String> {
    let base = fine_key(
        connector,
        network,
        variant,
        funding,
        issuer,
        currency,
        ic_category,
        interchange_bps,
    );
    let legacy = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        connector.to_lowercase(),
        normalize_network(&network.to_lowercase()),
        variant.to_lowercase(),
        funding.to_lowercase(),
        issuer.to_lowercase(),
        currency.to_lowercase(),
        ic_category.to_lowercase(),
    );
    let mut out = Vec::new();
    if let Some(idx) = segment_idx {
        out.push(format!("{base}|{idx}"));
        out.push(format!("{legacy}|{idx}"));
    }
    out.push(legacy);
    out
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
    pub interchange_bps: Option<String>,
    pub segment_idx: Option<u16>,
    pub amount_lo: Option<f64>,
    pub amount_hi: Option<f64>,
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

    let brand = normalize_network(&network.to_lowercase()).to_string();
    let connector_l = connector.to_lowercase();

    // Resolve the fine cluster key once (needs a raw issuer + predicted category/rate pair).
    // The same predicted pair is used for overrides and learned models, so they cannot drift.
    let fine = if issuer.is_empty() {
        None
    } else {
        let variant = reconstruct_variant(network, program, funding, wallet);
        let band = amount_band(amount);
        predict_category(m, network, &variant, funding, issuer, band, channel).map(|pred| {
            let key = fine_key(
                connector,
                network,
                &variant,
                funding,
                issuer,
                currency,
                &pred.category,
                &pred.interchange_bps,
            );
            (key, variant, pred)
        })
    };

    let fine_segment = fine
        .as_ref()
        .and_then(|(key, _, _)| m.fine.get(key))
        .and_then(|segments| pick_segment(segments, amount));

    // 1. Cluster override — exact segment first, then legacy unsegmented keys.
    if let Some((_, variant, pred)) = &fine {
        let keys = override_keys(
            connector,
            network,
            variant,
            funding,
            issuer,
            currency,
            &pred.category,
            &pred.interchange_bps,
            fine_segment.map(|s| s.segment_idx),
        );
        if let Some(cost) = keys.iter().find_map(|key| m.cluster_overrides.get(key)) {
            return Some(InhouseMatch {
                effective_bps: cost.effective_cost_bps(amount),
                pct_bps: cost.pct_bps,
                fixed: cost.fixed,
                brand,
                currency: currency.to_uppercase(),
                variant: Some(variant.clone()),
                issuer: Some(issuer.to_uppercase()),
                ic_category: Some(pred.category.clone()),
                interchange_bps: Some(pred.interchange_bps.clone()),
                segment_idx: fine_segment.map(|s| s.segment_idx),
                amount_lo: fine_segment.map(|s| s.amount_lo),
                amount_hi: fine_segment.map(|s| s.amount_hi),
            });
        }
    }

    // 2. Connector override: the merchant gave us this connector's blanket contract rate, so use it
    //    flat for every transaction not covered by a cluster override above. `connector` is already
    //    lowercased by the caller; lowercase again defensively so the key always matches.
    if let Some(cost) = m.overrides.get(&connector_l) {
        return Some(InhouseMatch {
            effective_bps: cost.effective_cost_bps(amount),
            pct_bps: cost.pct_bps,
            fixed: cost.fixed,
            brand,
            currency: currency.to_uppercase(),
            variant: None,
            issuer: None,
            ic_category: None,
            interchange_bps: None,
            segment_idx: None,
            amount_lo: None,
            amount_hi: None,
        });
    }

    // The invoice-derived add-on for this connector (if any), layered onto the *learned* models
    // below — never onto the overrides above, which are already all-in contract rates.
    let addon = m.addons.get(&connector_l);

    // 3. Learned fine model: serve the specific fitted cluster, plus the invoice add-on.
    if let (Some(segment), Some((_, variant, pred))) = (fine_segment, fine.as_ref()) {
        let cost = segment.cost.with_addon(addon);
        return Some(InhouseMatch {
            effective_bps: cost.effective_cost_bps(amount),
            pct_bps: cost.pct_bps,
            fixed: cost.fixed,
            brand,
            currency: currency.to_uppercase(),
            variant: Some(variant.clone()),
            issuer: Some(issuer.to_uppercase()),
            ic_category: Some(pred.category.clone()),
            interchange_bps: Some(pred.interchange_bps.clone()),
            segment_idx: Some(segment.segment_idx),
            amount_lo: Some(segment.amount_lo),
            amount_hi: Some(segment.amount_hi),
        });
    }

    // 4. Fallback: the coarse region blend (previous behavior) — no single variant/issuer/category.
    let key = coarse_key(connector, network, funding, currency, region);
    m.coarse.get(&key).and_then(|segments| {
        let segment = pick_segment(segments, amount)?;
        let cost = segment.cost.with_addon(addon);
        Some(InhouseMatch {
            effective_bps: cost.effective_cost_bps(amount),
            pct_bps: cost.pct_bps,
            fixed: cost.fixed,
            brand,
            currency: currency.to_uppercase(),
            variant: None,
            issuer: None,
            ic_category: None,
            interchange_bps: None,
            segment_idx: Some(segment.segment_idx),
            amount_lo: Some(segment.amount_lo),
            amount_hi: Some(segment.amount_hi),
        })
    })
}

fn pick_segment(segments: &[ServingSegment], amount: f64) -> Option<ServingSegment> {
    let mut fallback = None;
    let mut best = None;
    let mut best_width = f64::INFINITY;
    for segment in segments {
        if segment.amount_lo == 0.0 && segment.amount_hi == 0.0 {
            fallback = Some(*segment);
            continue;
        }
        if amount >= segment.amount_lo && amount < segment.amount_hi {
            let width = segment.amount_hi - segment.amount_lo;
            if width < best_width {
                best = Some(*segment);
                best_width = width;
            }
        }
    }
    best.or(fallback)
}

/// Predict the interchange category/rate pair by trying each back-off level, most specific first.
fn predict_category(
    m: &MerchantModels,
    network: &str,
    variant: &str,
    funding: &str,
    issuer: &str,
    band: &str,
    channel: &str,
) -> Option<PredictedIc> {
    let keys = predictor_level_keys(network, variant, funding, issuer, band, channel);
    for (i, key) in keys.iter().enumerate() {
        if let Some(table) = m.predictor.get(i) {
            if let Some(pred) = table.get(key) {
                return Some(pred.clone());
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
    interchange_bps, segment_idx, amount_lo, amount_hi,
    sum(pct_bps * gross_sum) AS pct_num,
    sum(fixed * gross_sum)   AS fixed_num,
    sum(gross_sum)           AS w
FROM __DB__.cost_fee_model FINAL
WHERE verdict = 'GOOD' AND gross_sum > 0{merchant_filter}
  AND (merchant_id, connector, account, report_date) IN (
      SELECT merchant_id, connector, account, max(report_date)
      FROM __DB__.cost_fee_model{merchant_filter_sub} GROUP BY merchant_id, connector, account)
GROUP BY merchant_id, connector, card_network, variant, funding, issuer_country, currency,
         ic_category, interchange_bps, segment_idx, amount_lo, amount_hi
FORMAT TSV
"#;

/// Per-(merchant, network, variant, funding, issuer, band, channel) category counts, for the
/// predictor. `channel` (pos/ecom) is the strongest disambiguator between in-person and online
/// interchange categories. `band` is a stored column of the rollup (stamped at ingestion by the same
/// `amount_band` thresholds this file uses at decide time). Positive micro transactions are kept;
/// fixed-fee tails are handled by the fitter.
/// `{merchant_filter}` is a `WHERE merchant_id = {merchant:String}` for a single-merchant refresh,
/// or `""` for the global rebuild.
const PREDICTOR_SQL: &str = r#"
SELECT
    merchant_id, card_network, variant, funding, issuer_country, band, channel,
    ic_category, interchange_bps,
    sum(n) AS c
FROM __DB__.cost_daily_stats FINAL
{merchant_filter}
GROUP BY merchant_id, card_network, variant, funding, issuer_country, band, channel,
         ic_category, interchange_bps
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
                .replace(
                    "{merchant_filter_sub}",
                    " WHERE merchant_id = {merchant:String}",
                ),
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

    // 1. Cost tables (coarse blend + fine per-category), volume-weighted per amount segment.
    let mut coarse_acc: HashMap<String, HashMap<String, HashMap<String, SegmentAcc>>> =
        HashMap::new();
    let mut fine_acc: HashMap<String, HashMap<String, HashMap<String, SegmentAcc>>> =
        HashMap::new();
    for line in cost_rows.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 15 {
            continue;
        }
        let (merchant, connector, network, variant, funding, issuer, currency, ic, ic_bps) =
            (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8]);
        let segment_idx: u16 = f[9].trim().parse().unwrap_or(0);
        let amount_lo: f64 = f[10].trim().parse().unwrap_or(0.0);
        let amount_hi: f64 = f[11].trim().parse().unwrap_or(0.0);
        let pct_num: f64 = f[12].trim().parse().unwrap_or(0.0);
        let fix_num: f64 = f[13].trim().parse().unwrap_or(0.0);
        let w: f64 = f[14].trim().parse().unwrap_or(0.0);
        if w <= 0.0 {
            continue;
        }
        let region = issuer_region(issuer);
        let ck = coarse_key(connector, network, funding, currency, &region);
        accumulate_segment(
            coarse_acc.entry(merchant.to_string()).or_default(),
            ck,
            segment_idx,
            amount_lo,
            amount_hi,
            pct_num,
            fix_num,
            w,
        );
        let fk = fine_key(
            connector, network, variant, funding, issuer, currency, ic, ic_bps,
        );
        accumulate_segment(
            fine_acc.entry(merchant.to_string()).or_default(),
            fk,
            segment_idx,
            amount_lo,
            amount_hi,
            pct_num,
            fix_num,
            w,
        );
    }
    for (merchant, keys) in coarse_acc {
        let m = snap.entry(merchant).or_default();
        m.coarse = finalize_segments(keys);
    }
    for (merchant, keys) in fine_acc {
        let m = snap.entry(merchant).or_default();
        m.fine = finalize_segments(keys);
    }

    // 2. Predictor tables: accumulate category counts per back-off level, keep the modal category
    //    with >= MIN_SUPPORT total observations.
    let mut pred_acc: HashMap<String, Vec<HashMap<String, HashMap<PredictedIc, u64>>>> =
        HashMap::new();
    for line in pred_rows.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 10 {
            continue;
        }
        let (merchant, network, variant, funding, issuer, band, channel, ic, ic_bps) =
            (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8]);
        let c: u64 = f[9].trim().parse().unwrap_or(0);
        if c == 0 {
            continue;
        }
        let pred = PredictedIc {
            category: ic.to_string(),
            interchange_bps: ic_bps.to_string(),
        };
        let levels = pred_acc
            .entry(merchant.to_string())
            .or_insert_with(|| vec![HashMap::new(); PREDICTOR_LEVELS]);
        for (i, key) in predictor_level_keys(network, variant, funding, issuer, band, channel)
            .into_iter()
            .enumerate()
        {
            *levels[i]
                .entry(key)
                .or_default()
                .entry(pred.clone())
                .or_insert(0) += c;
        }
    }
    for (merchant, levels) in pred_acc {
        let tables: Vec<HashMap<String, PredictedIc>> = levels
            .into_iter()
            .map(|level| {
                level
                    .into_iter()
                    .filter_map(|(key, cats)| {
                        let total: u64 = cats.values().sum();
                        if total < MIN_SUPPORT {
                            return None;
                        }
                        cats.into_iter()
                            .max_by_key(|(_, n)| *n)
                            .map(|(pred, _)| (key, pred))
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
        Some(mid) => load_overlays_into(&mut snap, mid).await,
        None => {
            // Union the override-merchant and invoice-add-on-merchant indices, so an overlay-only
            // merchant (manual override *or* invoice add-on, no ClickHouse data) is still hydrated.
            let mut merchants = super::overrides::list_merchants()
                .await
                .unwrap_or_else(|e| {
                    logger::warn!(tag = "cost_serving", "override index load failed: {:?}", e);
                    Vec::new()
                });
            match super::invoice::store::list_merchants().await {
                Ok(addon_merchants) => {
                    for mid in addon_merchants {
                        if !merchants.contains(&mid) {
                            merchants.push(mid);
                        }
                    }
                }
                Err(e) => logger::warn!(tag = "cost_serving", "add-on index load failed: {:?}", e),
            }
            for mid in merchants {
                load_overlays_into(&mut snap, &mid).await;
            }
        }
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
            let mut guard = cache()
                .write()
                .map_err(|_| "serving cache poisoned".to_string())?;
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

/// Load a merchant's serving-time overlays — manual overrides *and* the invoice-derived cost add-on
/// — and set them on its snapshot entry (creating the entry when the merchant has overlays but no
/// ClickHouse-derived models). Each overlay is non-fatal on error.
async fn load_overlays_into(snap: &mut Snapshot, merchant_id: &str) {
    // Connector-level overrides (lowercase connector → flat cost).
    match super::overrides::list(merchant_id).await {
        Ok(list) if !list.is_empty() => {
            let overrides = list
                .into_iter()
                .map(|(connector, ov)| {
                    (
                        connector.to_lowercase(),
                        ServingCost {
                            pct_bps: ov.pct_bps,
                            fixed: ov.fixed,
                        },
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

    // Cluster-level overrides, keyed by the same wire key the lookup probes at decide time.
    match super::overrides::list_clusters(merchant_id).await {
        Ok(list) if !list.is_empty() => {
            let cluster_overrides = list
                .into_iter()
                .map(|c| {
                    let key = super::overrides::key_of_dims(&c.dims);
                    (
                        key,
                        ServingCost {
                            pct_bps: c.pct_bps,
                            fixed: c.fixed,
                        },
                    )
                })
                .collect();
            snap.entry(merchant_id.to_string())
                .or_default()
                .cluster_overrides = cluster_overrides;
        }
        Ok(_) => {}
        Err(e) => logger::warn!(
            tag = "cost_serving",
            "cluster override load failed for {}: {:?}",
            merchant_id,
            e
        ),
    }

    // Invoice-derived add-ons (lowercase connector → {pct_addon_bps, fixed}). Layered onto the
    // learned models at lookup; stored connector keys are already lowercased by the invoice store.
    match super::invoice::store::list(merchant_id).await {
        Ok(list) if !list.is_empty() => {
            let addons = list
                .into_iter()
                .map(|(connector, a)| {
                    (
                        connector.to_lowercase(),
                        ServingCost {
                            pct_bps: a.pct_addon_bps,
                            fixed: a.fixed_addon,
                        },
                    )
                })
                .collect();
            snap.entry(merchant_id.to_string()).or_default().addons = addons;
        }
        Ok(_) => {}
        Err(e) => logger::warn!(
            tag = "cost_serving",
            "invoice add-on load failed for {}: {:?}",
            merchant_id,
            e
        ),
    }
}

#[derive(Debug, Clone, Copy)]
struct SegmentAcc {
    segment_idx: u16,
    amount_lo: f64,
    amount_hi: f64,
    pct_num: f64,
    fix_num: f64,
    w: f64,
}

fn accumulate_segment(
    map: &mut HashMap<String, HashMap<String, SegmentAcc>>,
    key: String,
    segment_idx: u16,
    amount_lo: f64,
    amount_hi: f64,
    pct_num: f64,
    fix_num: f64,
    w: f64,
) {
    let segment_key = format!("{segment_idx}|{amount_lo:.12}|{amount_hi:.12}");
    let e = map
        .entry(key)
        .or_default()
        .entry(segment_key)
        .or_insert(SegmentAcc {
            segment_idx,
            amount_lo,
            amount_hi,
            pct_num: 0.0,
            fix_num: 0.0,
            w: 0.0,
        });
    e.pct_num += pct_num;
    e.fix_num += fix_num;
    e.w += w;
}

fn finalize_segments(
    keys: HashMap<String, HashMap<String, SegmentAcc>>,
) -> HashMap<String, Vec<ServingSegment>> {
    keys.into_iter()
        .filter_map(|(k, segments)| {
            let mut out: Vec<ServingSegment> = segments
                .into_values()
                .filter(|s| s.w > 0.0)
                .map(|s| ServingSegment {
                    cost: ServingCost {
                        pct_bps: s.pct_num / s.w,
                        fixed: s.fix_num / s.w,
                    },
                    segment_idx: s.segment_idx,
                    amount_lo: s.amount_lo,
                    amount_hi: s.amount_hi,
                })
                .collect();
            out.sort_by(|a, b| {
                a.amount_lo
                    .total_cmp(&b.amount_lo)
                    .then(a.amount_hi.total_cmp(&b.amount_hi))
                    .then(a.segment_idx.cmp(&b.segment_idx))
            });
            if out.is_empty() {
                None
            } else {
                Some((k, out))
            }
        })
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
        return Err(format!(
            "clickhouse serving query failed ({status}): {text}"
        ));
    }
    resp.text().await.map_err(|e| e.to_string())
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| super::ch_http::client(QUERY_TIMEOUT))
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
        assert_eq!(
            reconstruct_variant("VISA", "STANDARD", "DEBIT", ""),
            "visastandarddebit"
        );
        assert_eq!(
            reconstruct_variant("MASTERCARD", "PREMIUM", "CREDIT", ""),
            "mcpremiumcredit"
        );
        // A wallet is its own report variant.
        assert_eq!(
            reconstruct_variant("VISA", "STANDARD", "DEBIT", "APPLE_PAY"),
            "visa_applepay"
        );
    }

    #[test]
    fn invoice_addon_adds_to_pct_and_fixed() {
        let learned = ServingCost {
            pct_bps: 40.0,
            fixed: 0.10,
        };
        let addon = ServingCost {
            pct_bps: 0.06,
            fixed: 0.04,
        }; // ~invoice add-on
        let combined = learned.with_addon(Some(&addon));
        assert!((combined.pct_bps - 40.06).abs() < 1e-9);
        assert!((combined.fixed - 0.14).abs() < 1e-9);
        // On a €50 sale the flat add-on moves effective cost by 0.04/50·1e4 = 8 bps.
        let before = learned.effective_cost_bps(50.0);
        let after = combined.effective_cost_bps(50.0);
        assert!((after - before - (0.06 + 8.0)).abs() < 1e-9);
        // No add-on is the identity.
        assert_eq!(learned.with_addon(None).pct_bps, learned.pct_bps);
        assert_eq!(learned.with_addon(None).fixed, learned.fixed);
    }

    #[test]
    fn amount_bands() {
        assert_eq!(amount_band(15.0), "lo");
        assert_eq!(amount_band(40.0), "b50");
        assert_eq!(amount_band(60.0), "b100"); // the €60 AUD case → "> AUD 50" tier
        assert_eq!(amount_band(500.0), "hi");
    }
}
