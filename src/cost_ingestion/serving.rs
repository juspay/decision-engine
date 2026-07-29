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

/// One recovered amount tier of a capped/tiered cluster: a `{pct_bps, fixed}` line over `[lo, hi)`.
#[derive(Debug, Clone, Copy)]
struct ServingSegment {
    lo: f64,
    hi: f64,
    cost: ServingCost,
    /// Whether this piece's own fit is trustworthy. A ladder can contain a thin micro tail; pricing
    /// on it would be worse than pricing on the ladder's blend.
    good: bool,
}

/// A cluster a single line cannot price — an absolute interchange cap or a tiered schedule makes
/// `fee ~ gross` piecewise-linear — recovered into ordered amount tiers.
///
/// These clusters are exactly the ones the whole-cluster fit grades `NON_LINEAR`, so they are absent
/// from [`MerchantModels::fine`] (which is GOOD-only) and, before this existed, fell through to the
/// coarse regional blend. On real UAE/Saudi capped debit that fallback is badly wrong at small
/// tickets: the single line's fixed term is an artifact of bending a straight line around a cap, and
/// it overstated a 50 SAR transaction's fee by ~5x.
#[derive(Debug, Clone)]
struct SegmentedCost {
    /// Ascending by `lo`, contiguous, non-empty.
    segments: Vec<ServingSegment>,
    /// Volume-weighted blend of the GOOD pieces — the fallback when the amount lands on a piece we
    /// don't trust, and the headline the dashboard shows for the cluster.
    blend: ServingCost,
    /// Settled volume behind the GOOD pieces. Only used to weight this ladder against sibling card
    /// products when the display key has to be priced without knowing which product a card is.
    weight: f64,
}

/// One card product's contribution to a display key, priced at a given amount.
#[derive(Debug, Clone)]
enum MemberCost {
    /// A GOOD cluster: one line at every amount.
    Flat(ServingCost),
    /// A capped/tiered cluster: the rate depends on the amount.
    Tiered(SegmentedCost),
}

/// Every card product under one display key, with the settled volume behind each — the fallback for
/// when the card product **cannot be resolved**.
///
/// `fine` and `segmented` are keyed by display-key + `card_product`, and `card_product` comes from the
/// issuer BIN. A decide request without a BIN (`cardIsin: null`) resolves it to `""`, so those exact
/// lookups miss even when the display key is a perfect match, and the request fell all the way to the
/// coarse regional blend — or, when the coarse table had nothing either, to the seed table.
///
/// Blending across products is amount-aware rather than a single averaged line: a display key can
/// hold a GOOD product and a capped one at once, and averaging a tiered product's *headline* would
/// reintroduce the flat-rate error tiering exists to remove. Each member is priced at the actual
/// amount first, then volume-weighted.
#[derive(Debug, Clone)]
struct FineBlend {
    /// `(settled volume, that product's cost)`. Non-empty; weights are > 0.
    members: Vec<(f64, MemberCost)>,
    total_weight: f64,
}

impl FineBlend {
    fn for_amount(&self, amount: f64) -> ServingCost {
        if self.total_weight <= 0.0 {
            return ServingCost {
                pct_bps: 0.0,
                fixed: 0.0,
            };
        }
        let (mut pct, mut fixed) = (0.0, 0.0);
        for (w, m) in &self.members {
            let c = match m {
                MemberCost::Flat(c) => *c,
                MemberCost::Tiered(s) => s.for_amount(amount),
            };
            pct += c.pct_bps * w;
            fixed += c.fixed * w;
        }
        ServingCost {
            pct_bps: pct / self.total_weight,
            fixed: fixed / self.total_weight,
        }
    }
}

impl SegmentedCost {
    /// The tier that prices `amount`.
    ///
    /// Amounts outside the observed range clamp to the nearest end tier rather than abstaining: a
    /// transaction just below the smallest or above the largest settled amount is far better priced
    /// by the adjacent tier than by the regional blend. Clamping upward does assume no *further*
    /// cap beyond the top tier — true for the cap shapes this recovers (above the knot interchange
    /// is already flat), and still strictly closer than the alternative.
    fn for_amount(&self, amount: f64) -> ServingCost {
        let seg = self
            .segments
            .iter()
            .find(|s| amount >= s.lo && amount < s.hi)
            .or_else(|| {
                // Below the first tier, or at/above the last.
                if amount < self.segments[0].lo {
                    self.segments.first()
                } else {
                    self.segments.last()
                }
            });
        match seg {
            Some(s) if s.good => s.cost,
            // An untrusted piece prices worse than the ladder's own blend.
            _ => self.blend,
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
    /// `fine_key_model` → amount tiers, for capped/tiered clusters one line can't price. Disjoint
    /// from `fine` by construction: the segmenter only runs on clusters the whole-cluster fit graded
    /// non-GOOD, and `fine` carries only GOOD ones.
    segmented: HashMap<String, SegmentedCost>,
    /// Display key (NO `card_product`) → every product under it, volume-weighted. Serves a request
    /// whose card product can't be resolved — no BIN, or a BIN not yet in the map.
    fine_blend: HashMap<String, FineBlend>,
    /// Predictor back-off levels (most specific first): level-key → modal `ic_category`.
    predictor: Vec<HashMap<String, String>>,
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
            && self.segmented.is_empty()
            && self.fine_blend.is_empty()
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

/// Global BIN → dominant-`card_product` map (canonical 6-digit BIN → interchange-rate tier), shared
/// across all merchants — a card's product is universal. Refreshed alongside the per-merchant models
/// so the decide-time key resolves the same `card_product` the fit stamped at ingest.
fn bin_cache() -> &'static RwLock<Arc<HashMap<String, String>>> {
    static BIN_CACHE: OnceLock<RwLock<Arc<HashMap<String, String>>>> = OnceLock::new();
    BIN_CACHE.get_or_init(|| RwLock::new(Arc::new(HashMap::new())))
}

/// Resolve a card's `card_product` tier from its BIN, canonicalised to the same 6-digit key the
/// ingest side stored. `""` when the BIN is empty or unmapped — the fine lookup then only matches
/// blended (`card_product = ""`) clusters and otherwise falls through to the coarse blend.
fn resolve_bin_product(bin: &str) -> String {
    if bin.is_empty() {
        return String::new();
    }
    let key = crate::cost_ingestion::SettledFeeRow::bin_key(bin);
    bin_cache()
        .read()
        .ok()
        .and_then(|m| m.get(&key).cloned())
        .unwrap_or_default()
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

/// The learned-model fine key: the display [`fine_key`] plus the fan-separating `card_product` tier.
/// The learned per-category cost table keys on this (so 135 and 180 stay distinct), while the manual
/// cluster overrides stay on the card_product-less display key (a merchant sets one fee for the whole
/// displayed cluster, across its sub-products).
fn fine_key_model(display_key: &str, card_product: &str) -> String {
    format!("{}|{}", display_key, card_product.to_lowercase())
}

/// The same display key with the `variant` field blanked — `adyen|visa||debit|ae|aed|…`.
///
/// `variant` is the one cluster field that is *reconstructed* at decide time rather than observed:
/// [`reconstruct_variant`] builds `{network}{program}{funding}` and hopes the settlement report spelled
/// it identically. Adyen's own exports do (`visastandarddebit`), but any report using a different
/// vocabulary — `visadebit`, a co-badge rail, a connector with its own naming — produces a key that
/// can never match, and the request silently falls to the coarse blend or the seed table despite an
/// exact cluster existing on every other dimension.
///
/// Blanking the field (rather than dropping it) keeps the shape and position of every other field, so
/// this can never alias onto a real key: a genuine variant is non-empty for card clusters.
fn without_variant(display_key: &str) -> Option<String> {
    // connector | network | variant | funding | issuer | currency | ic_category
    let i1 = display_key.find('|')?;
    let i2 = display_key[i1 + 1..].find('|')? + i1 + 1;
    let i3 = display_key[i2 + 1..].find('|')? + i2 + 1;
    Some(format!("{}{}", &display_key[..=i2], &display_key[i3..]))
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
    bin: &str,
    amount: f64,
) -> Option<InhouseMatch> {
    let snapshot = cache().read().ok()?.clone();
    let m = snapshot.get(merchant_id)?;

    // Resolve the card's product tier from its BIN (the fan-separating dimension) — the same value
    // the fit stamped at ingest. `""` when the BIN is unknown/unmapped, which limits the fine match
    // to blended clusters and otherwise degrades to the coarse blend.
    let card_product = resolve_bin_product(bin);

    // Resolve the fine cluster key once (needs a raw issuer + a predicted interchange category).
    // Reused for both the highest-precedence cluster-override check (display key) and the learned
    // fine-model lookup (display key + card_product), so the two can never disagree on the cluster.
    let fine = if issuer.is_empty() {
        None
    } else {
        let variant = reconstruct_variant(network, program, funding, wallet);
        let band = amount_band(amount);
        predict_category(m, network, &variant, funding, issuer, &band, channel).map(|cat| {
            let key = fine_key(
                connector, network, &variant, funding, issuer, currency, &cat,
            );
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

    // The invoice-derived add-on for this connector (if any), layered onto the *learned* models
    // below — never onto the overrides above, which are already all-in contract rates.
    let addon = m.addons.get(&connector.to_lowercase());

    // Both learned tables key on display-key + card_product. Build that key ONCE: `lookup` runs per
    // candidate PSP on the decide path, so formatting it per table would allocate a String per PSP
    // per decision for nothing.
    if let Some((key, variant, cat)) = &fine {
        let model_key = fine_key_model(key, &card_product);

        // 3. Amount-aware tiers: a capped/tiered cluster is priced by the piece this AMOUNT falls
        //    in, not by one line across the whole range. Checked before the flat fine model because
        //    it is the more specific answer for the same key (in practice the two are disjoint — the
        //    segmenter only runs where the whole-cluster fit was not GOOD, and `fine` is GOOD-only).
        if let Some(seg) = m.segmented.get(&model_key) {
            let cost = seg.for_amount(amount).with_addon(addon);
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

        // 4. Learned fine model: the specific fitted cluster (display key + card_product, so the
        //    fan's 135/180 tiers resolve to their own rate), plus the invoice add-on.
        if let Some(cost) = m.fine.get(&model_key) {
            let cost = cost.with_addon(addon);
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

        // 5. Card product unresolved: price the display key by blending every product under it,
        //    each evaluated at THIS amount. Reached when the request carries no BIN, or a BIN we
        //    haven't mapped yet — previously that abstained and fell through to the regional blend
        //    (or, with nothing there, to the seed table) despite an exact display-key match existing.
        //    The variant-blanked key is the last rung: it also covers a report whose variant
        //    vocabulary we can't reconstruct (see `without_variant`), blending that cluster's card
        //    programs by volume rather than abstaining to a regional average or the seed table.
        let blend = m.fine_blend.get(key).or_else(|| {
            without_variant(key).and_then(|k| m.fine_blend.get(&k).map(|b| b as &FineBlend))
        });
        if let Some(blend) = blend {
            let cost = blend.for_amount(amount).with_addon(addon);
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

    // 6. Fallback: the coarse region blend (previous behavior) — no single variant/issuer/category.
    let key = coarse_key(connector, network, funding, currency, region);
    m.coarse.get(&key).map(|cost| {
        let cost = cost.with_addon(addon);
        InhouseMatch {
            effective_bps: cost.effective_cost_bps(amount),
            pct_bps: cost.pct_bps,
            fixed: cost.fixed,
            brand: normalize_network(&network.to_lowercase()).to_string(),
            currency: currency.to_uppercase(),
            variant: None,
            issuer: None,
            ic_category: None,
        }
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
    card_product,
    sum(pct_bps * gross_sum) AS pct_num,
    sum(fixed * gross_sum)   AS fixed_num,
    sum(gross_sum)           AS w
FROM __DB__.cost_fee_model FINAL
WHERE verdict = 'GOOD' AND gross_sum > 0{merchant_filter}
  AND (merchant_id, connector, account, report_date) IN (
      SELECT merchant_id, connector, account, max(report_date)
      FROM __DB__.cost_fee_model{merchant_filter_sub} GROUP BY merchant_id, connector, account)
GROUP BY merchant_id, connector, card_network, variant, funding, issuer_country, currency, ic_category,
    card_product
FORMAT TSV
"#;

/// The recovered amount tiers of every capped/tiered cluster — the amount-aware half of serving.
///
/// Only the DOMINANT ladder per cluster is served. A cluster key can carry tiers from more than one
/// connector-side account, and two accounts' ladders have different knots, so they cannot be
/// averaged the way flat rates are; the ladder carrying the most settled volume is the one the
/// cluster's money actually rode on. (Same rule the dashboard applies — see `blended::SEGMENTS_SQL`.)
///
/// `{merchant_filter}` / `{merchant_filter_sub}` mirror `COST_SQL`: a predicate for a single-merchant
/// refresh, or `""` for the periodic global rebuild.
const SEGMENT_SQL: &str = r#"
WITH latest AS (
    SELECT merchant_id, connector, account, card_network, variant, funding, issuer_country, currency,
           ic_category, card_product, seg_idx, lo, hi, pct_bps, fixed, gross_sum, verdict
    FROM __DB__.cost_fee_model_segment FINAL
    WHERE gross_sum > 0{merchant_filter}
      AND (merchant_id, connector, account, report_date) IN (
          SELECT merchant_id, connector, account, max(report_date)
          FROM __DB__.cost_fee_model_segment{merchant_filter_sub}
          GROUP BY merchant_id, connector, account)
),
ranked AS (
    SELECT *, sum(gross_sum) OVER (
        PARTITION BY merchant_id, connector, card_network, variant, funding, issuer_country,
                     currency, ic_category, card_product, account) AS ladder_vol
    FROM latest
),
topped AS (
    SELECT *, max(ladder_vol) OVER (
        PARTITION BY merchant_id, connector, card_network, variant, funding, issuer_country,
                     currency, ic_category, card_product) AS top_vol
    FROM ranked
)
SELECT merchant_id, connector, card_network, variant, funding, issuer_country, currency,
       ic_category, card_product, seg_idx, lo, hi, pct_bps, fixed, gross_sum, verdict
FROM topped
WHERE ladder_vol = top_vol
ORDER BY merchant_id, connector, card_network, variant, funding, issuer_country, currency,
         ic_category, card_product, seg_idx
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
    // Refresh the GLOBAL BIN → dominant-card_product map (shared across all merchants — a card's
    // product is universal). A single small read, done on every refresh so decide-time resolution
    // tracks BINs as new reports land. Empty on failure ⇒ card_product resolves to "" ⇒ coarse blend.
    let bin_map = super::sink::load_bin_product(cfg).await;
    if let Ok(mut guard) = bin_cache().write() {
        *guard = bin_map;
    }

    // Splice the merchant predicate into the queries (or clear the placeholders for a global rebuild).
    let (cost_sql, pred_sql, seg_sql) = match merchant {
        Some(_) => (
            COST_SQL
                .replace("{merchant_filter}", " AND merchant_id = {merchant:String}")
                .replace(
                    "{merchant_filter_sub}",
                    " WHERE merchant_id = {merchant:String}",
                ),
            PREDICTOR_SQL.replace("{merchant_filter}", "WHERE merchant_id = {merchant:String}"),
            SEGMENT_SQL
                .replace("{merchant_filter}", " AND merchant_id = {merchant:String}")
                .replace(
                    "{merchant_filter_sub}",
                    " WHERE merchant_id = {merchant:String}",
                ),
        ),
        None => (
            COST_SQL
                .replace("{merchant_filter}", "")
                .replace("{merchant_filter_sub}", ""),
            PREDICTOR_SQL.replace("{merchant_filter}", ""),
            SEGMENT_SQL
                .replace("{merchant_filter}", "")
                .replace("{merchant_filter_sub}", ""),
        ),
    };
    let cost_rows = query(cfg, &cost_sql, merchant).await?;
    let pred_rows = query(cfg, &pred_sql, merchant).await?;
    // Segments are an enrichment layered on the cost tables: a merchant with none (or a database
    // predating the segment table) must still serve its flat models, so a failure here degrades to
    // "no tiers" rather than failing the whole refresh.
    let seg_rows = match query(cfg, &seg_sql, merchant).await {
        Ok(r) => r,
        Err(e) => {
            logger::warn!(tag = "cost_serving", "segment load skipped: {}", e);
            String::new()
        }
    };

    let mut snap: Snapshot = HashMap::new();

    // 1. Cost tables (coarse blend + fine per-category), volume-weighted.
    let mut coarse_acc: HashMap<String, HashMap<String, (f64, f64, f64)>> = HashMap::new(); // merchant -> key -> (pct_num, fix_num, w)
    let mut fine_acc: HashMap<String, HashMap<String, (f64, f64, f64)>> = HashMap::new();
    for line in cost_rows.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 12 {
            continue;
        }
        let (merchant, connector, network, variant, funding, issuer, currency, ic, card_product) =
            (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8]);
        let pct_num: f64 = f[9].trim().parse().unwrap_or(0.0);
        let fix_num: f64 = f[10].trim().parse().unwrap_or(0.0);
        let w: f64 = f[11].trim().parse().unwrap_or(0.0);
        if w <= 0.0 {
            continue;
        }
        // Coarse blend deliberately drops card_product (and variant/issuer): it is the fallback that
        // averages a connector's whole region, so the fan's tiers merge back into one number here.
        let region = issuer_region(issuer);
        let ck = coarse_key(connector, network, funding, currency, &region);
        accumulate(
            coarse_acc.entry(merchant.to_string()).or_default(),
            ck,
            pct_num,
            fix_num,
            w,
        );
        // Fine table keeps the tiers distinct — display key + card_product.
        let fk = fine_key_model(
            &fine_key(connector, network, variant, funding, issuer, currency, ic),
            card_product,
        );
        accumulate(
            fine_acc.entry(merchant.to_string()).or_default(),
            fk,
            pct_num,
            fix_num,
            w,
        );
    }
    for (merchant, keys) in coarse_acc {
        let m = snap.entry(merchant).or_default();
        m.coarse = finalize(keys);
    }
    // The pre-`finalize` weights, kept so the display-key blend below can weight each card product by
    // the settled volume behind it.
    let fine_weight: HashMap<String, HashMap<String, f64>> = fine_acc
        .iter()
        .map(|(m, keys)| {
            (
                m.clone(),
                keys.iter().map(|(k, (_, _, w))| (k.clone(), *w)).collect(),
            )
        })
        .collect();
    for (merchant, keys) in fine_acc {
        let m = snap.entry(merchant).or_default();
        m.fine = finalize(keys);
    }

    // 1b. Amount tiers for capped/tiered clusters.
    for (merchant, ladders) in segments_from_tsv(&seg_rows) {
        snap.entry(merchant).or_default().segmented = ladders;
    }

    // 1c. Per-display-key blend across card products, for requests whose product can't be resolved
    //     (no BIN, or an unmapped one). Members come from BOTH tables and they are disjoint by
    //     construction: `fine` holds only GOOD clusters, and the segmenter only runs on non-GOOD
    //     ones, so no cluster is counted twice.
    for (merchant, models) in snap.iter_mut() {
        let mut members: HashMap<String, Vec<(f64, MemberCost)>> = HashMap::new();
        // Every member is indexed under BOTH its display key and the variant-blanked one, so the
        // variant-agnostic rung is a plain lookup rather than a scan. The blanked bucket therefore
        // spans a cluster's card programs, weighted by the volume actually seen in each.
        // Scoped so the closure's mutable borrow of `members` ends before it is consumed below.
        {
            let mut push = |display: &str, w: f64, cost: MemberCost| {
                if w <= 0.0 {
                    return;
                }
                if let Some(k) = without_variant(display) {
                    members.entry(k).or_default().push((w, cost.clone()));
                }
                members
                    .entry(display.to_string())
                    .or_default()
                    .push((w, cost));
            };
            if let Some(weights) = fine_weight.get(merchant) {
                for (model_key, cost) in &models.fine {
                    let Some((display, _)) = model_key.rsplit_once('|') else {
                        continue;
                    };
                    let w = weights.get(model_key).copied().unwrap_or(0.0);
                    push(display, w, MemberCost::Flat(*cost));
                }
            }
            for (model_key, ladder) in &models.segmented {
                let Some((display, _)) = model_key.rsplit_once('|') else {
                    continue;
                };
                push(display, ladder.weight, MemberCost::Tiered(ladder.clone()));
            }
        }
        models.fine_blend = members
            .into_iter()
            .map(|(k, members)| {
                let total_weight = members.iter().map(|(w, _)| w).sum();
                (
                    k,
                    FineBlend {
                        members,
                        total_weight,
                    },
                )
            })
            .collect();
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
            *levels[i]
                .entry(key)
                .or_default()
                .entry(ic.to_string())
                .or_insert(0) += c;
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
                        cats.into_iter()
                            .max_by_key(|(_, n)| *n)
                            .map(|(cat, _)| (key, cat))
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

/// Build each merchant's amount-tier ladders from [`SEGMENT_SQL`]'s TSV.
///
/// Pure over its input so the whole decode path — including the Nullable and verdict handling — is
/// unit-testable against a captured ClickHouse response, with no database in the loop (the same
/// reason `fit::segment_clusters_from_tsv` is factored out).
///
/// Returns `merchant → fine_key_model → ladder`.
fn segments_from_tsv(rows: &str) -> HashMap<String, HashMap<String, SegmentedCost>> {
    let mut acc: HashMap<String, HashMap<String, Vec<ServingSegment>>> = HashMap::new();
    let mut blend_acc: HashMap<String, HashMap<String, (f64, f64, f64)>> = HashMap::new();

    for line in rows.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 16 {
            continue;
        }
        let (merchant, connector, network, variant, funding, issuer, currency, ic, card_product) =
            (f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8]);
        let (Ok(lo), Ok(hi)) = (f[10].trim().parse::<f64>(), f[11].trim().parse::<f64>()) else {
            continue;
        };
        // `pct_bps`/`fixed` are Nullable — an unfittable piece (no amount spread) arrives as `\N`.
        // Such a piece can never price anything, so it is dropped from the ladder entirely; the
        // neighbouring tier then covers its amounts through the clamp in `for_amount`.
        let (Ok(pct_bps), Ok(fixed)) = (f[12].trim().parse::<f64>(), f[13].trim().parse::<f64>())
        else {
            continue;
        };
        let gross: f64 = f[14].trim().parse().unwrap_or(0.0);
        let good = f[15].trim().eq_ignore_ascii_case("GOOD");
        let key = fine_key_model(
            &fine_key(connector, network, variant, funding, issuer, currency, ic),
            card_product,
        );
        acc.entry(merchant.to_string())
            .or_default()
            .entry(key.clone())
            .or_default()
            .push(ServingSegment {
                lo,
                hi,
                cost: ServingCost { pct_bps, fixed },
                good,
            });
        // The ladder's fallback blend is weighted over its GOOD pieces only — averaging in a piece we
        // don't trust is exactly what the fallback exists to avoid.
        if good && gross > 0.0 {
            accumulate(
                blend_acc.entry(merchant.to_string()).or_default(),
                key,
                pct_bps * gross,
                fixed * gross,
                gross,
            );
        }
    }

    let mut out: HashMap<String, HashMap<String, SegmentedCost>> = HashMap::new();
    for (merchant, ladders) in acc {
        let raw = blend_acc.remove(&merchant).unwrap_or_default();
        // Keep the raw weights before `finalize` divides them away — they weight this ladder against
        // sibling card products when the product can't be resolved at decide time.
        let blends_w: HashMap<String, f64> =
            raw.iter().map(|(k, (_, _, w))| (k.clone(), *w)).collect();
        let blends = finalize(raw);
        let entry = out.entry(merchant).or_default();
        for (key, mut segments) in ladders {
            // A ladder with no trustworthy piece has nothing to serve and no honest blend to fall
            // back on — leave that cluster to the coarse blend rather than invent a rate for it.
            let Some(&blend) = blends.get(&key) else {
                continue;
            };
            // SQL already orders by seg_idx; sorting makes the invariant `for_amount` relies on
            // (ascending, contiguous) local to this function rather than a property of the query.
            segments.sort_by(|a, b| a.lo.total_cmp(&b.lo));
            let weight = blends_w.get(&key).copied().unwrap_or(0.0);
            entry.insert(
                key,
                SegmentedCost {
                    segments,
                    blend,
                    weight,
                },
            );
        }
    }
    out
}

fn accumulate(
    map: &mut HashMap<String, (f64, f64, f64)>,
    key: String,
    pct_num: f64,
    fix_num: f64,
    w: f64,
) {
    let e = map.entry(key).or_insert((0.0, 0.0, 0.0));
    e.0 += pct_num;
    e.1 += fix_num;
    e.2 += w;
}

fn finalize(keys: HashMap<String, (f64, f64, f64)>) -> HashMap<String, ServingCost> {
    keys.into_iter()
        .filter(|(_, (_, _, w))| *w > 0.0)
        .map(|(k, (pn, fn_, w))| {
            (
                k,
                ServingCost {
                    pct_bps: pn / w,
                    fixed: fn_ / w,
                },
            )
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

    /// The real Mastercard UAE consumer-debit ladder: 1% interchange capped at AED 50, so the knot
    /// sits near AED 5,000 — below it the full ~170 bps applies; above it interchange is flat, which
    /// leaves ~70 bps of scheme+markup as the rate and moves the cap into the fixed term.
    fn uae_debit() -> SegmentedCost {
        SegmentedCost {
            segments: vec![
                ServingSegment {
                    lo: 19.95,
                    hi: 5011.87,
                    cost: ServingCost {
                        pct_bps: 170.1,
                        fixed: 0.42,
                    },
                    good: true,
                },
                ServingSegment {
                    lo: 5011.87,
                    hi: 25118.86,
                    cost: ServingCost {
                        pct_bps: 69.7,
                        fixed: 50.72,
                    },
                    good: true,
                },
            ],
            blend: ServingCost {
                pct_bps: 94.6,
                fixed: 38.23,
            },
            // Real settled volume behind this ladder, so it weights correctly against siblings.
            weight: 1_803_717.0,
        }
    }

    #[test]
    fn tier_is_selected_by_amount() {
        let s = uae_debit();
        assert_eq!(s.for_amount(100.0).pct_bps, 170.1, "below the knot");
        assert_eq!(s.for_amount(5000.0).pct_bps, 170.1, "just below the knot");
        assert_eq!(s.for_amount(5011.87).pct_bps, 69.7, "lo is inclusive");
        assert_eq!(s.for_amount(20000.0).pct_bps, 69.7, "above the knot");
    }

    #[test]
    fn amounts_outside_the_observed_range_clamp_to_the_end_tiers() {
        let s = uae_debit();
        assert_eq!(
            s.for_amount(5.0).pct_bps,
            170.1,
            "below the smallest settled amount"
        );
        assert_eq!(s.for_amount(1_000_000.0).pct_bps, 69.7, "above the largest");
        assert_eq!(
            s.for_amount(0.0).pct_bps,
            170.1,
            "zero clamps rather than panicking"
        );
    }

    /// The whole point of tiering: a single line fitted across a capped curve carries a fixed term
    /// that is an artifact, and it dominates small tickets. Here the one-line model would charge a
    /// 50-unit transaction ~5x what the correct tier does.
    #[test]
    fn small_tickets_are_not_priced_by_the_one_line_artifact() {
        let s = uae_debit();
        let tiered = s.for_amount(50.0).effective_cost_bps(50.0);
        let one_line = ServingCost {
            pct_bps: 106.4,
            fixed: 7.06,
        }
        .effective_cost_bps(50.0);
        assert!(
            (tiered - 254.1).abs() < 1.0,
            "170.1 bps + 0.42 on 50 = ~254 bps all-in, got {tiered}"
        );
        assert!(
            one_line > tiered * 4.0,
            "the un-tiered line should be far worse here: {one_line} vs {tiered}"
        );
    }

    #[test]
    fn an_untrusted_tier_falls_back_to_the_ladders_blend() {
        let mut s = uae_debit();
        s.segments[0].good = false;
        assert_eq!(s.for_amount(100.0).pct_bps, 94.6, "thin piece → blend");
        assert_eq!(
            s.for_amount(20000.0).pct_bps,
            69.7,
            "the good piece still serves itself"
        );
    }

    /// Decode a VERBATIM `SEGMENT_SQL` response — captured from ClickHouse against the real
    /// UAE/Saudi capped-debit clusters — and price through it end to end. This is the whole path the
    /// refresh takes, minus the network call.
    #[test]
    fn real_clickhouse_segment_rows_decode_and_price() {
        let tsv = "\
merchant_a10515216087\tadyen\tmada\tmadadebit\tdebit\tSA\tSAR\tmada Domestic Debit\t50\t0\t19.952623149688797\t7943.282347242814\t120.00676737830256\t0.41410296285395265\t741432.7033999999\tGOOD\n\
merchant_a10515216087\tadyen\tmada\tmadadebit\tdebit\tSA\tSAR\tmada Domestic Debit\t50\t1\t7943.282347242814\t31622.776601683792\t69.34584552758952\t41.326093066911476\t2186293.9519\tGOOD\n\
merchant_a10515216087\tadyen\tmc\tmcdebit\tdebit\tAE\tAED\tMastercard UAE Consumer Debit\t100\t0\t19.952623149688797\t5011.872336272725\t170.0619336742149\t0.41785428151799975\t514795.7669\tGOOD\n\
merchant_a10515216087\tadyen\tmc\tmcdebit\tdebit\tAE\tAED\tMastercard UAE Consumer Debit\t100\t1\t5011.872336272725\t25118.864315095823\t69.70785186034963\t50.715018957304224\t1558739.5159999996\tGOOD\n";

        let out = segments_from_tsv(tsv);
        let m = out.get("merchant_a10515216087").expect("merchant present");
        assert_eq!(m.len(), 2, "two capped clusters");

        let mada = m
            .get(&fine_key_model(
                &fine_key(
                    "adyen",
                    "mada",
                    "madadebit",
                    "debit",
                    "SA",
                    "SAR",
                    "mada Domestic Debit",
                ),
                "50",
            ))
            .expect("mada ladder keyed exactly as lookup builds it");
        assert_eq!(mada.segments.len(), 2);

        // Below the cap knot: the full domestic rate, not the one-line artifact.
        assert!((mada.for_amount(1_000.0).pct_bps - 120.0).abs() < 0.1);
        // Above it: interchange has flattened into the fixed term.
        let hi = mada.for_amount(20_000.0);
        assert!((hi.pct_bps - 69.3).abs() < 0.1 && (hi.fixed - 41.33).abs() < 0.01);

        // The blend is volume-weighted over the GOOD pieces — matches the dashboard's headline.
        assert!(
            (mada.blend.pct_bps - 82.2).abs() < 0.2,
            "expected ~82.2 bps, got {}",
            mada.blend.pct_bps
        );
    }

    /// An unfittable piece arrives as a SQL NULL and must be dropped rather than parsed as 0 bps —
    /// a free tier would look like the cheapest possible route to the optimizer.
    #[test]
    fn null_rate_pieces_are_dropped_not_read_as_zero() {
        let tsv = "\
m1\tadyen\tvisa\tvisadebit\tdebit\tAE\tAED\tVisa UAE Consumer Debit\t100\t0\t20\t5000\t\\N\t\\N\t900\tTHIN\n\
m1\tadyen\tvisa\tvisadebit\tdebit\tAE\tAED\tVisa UAE Consumer Debit\t100\t1\t5000\t25000\t70.0\t50.0\t1000\tGOOD\n";
        let out = segments_from_tsv(tsv);
        let ladder = out
            .get("m1")
            .and_then(|m| m.values().next())
            .expect("ladder");
        assert_eq!(ladder.segments.len(), 1, "the NULL piece is not a tier");
        // A 100-unit txn now clamps onto the surviving tier instead of pricing at zero.
        assert_eq!(ladder.for_amount(100.0).pct_bps, 70.0);
    }

    /// Every piece untrusted ⇒ no honest blend ⇒ serve nothing and let the coarse blend answer.
    #[test]
    fn a_ladder_with_no_good_piece_is_not_served() {
        let tsv = "\
m1\tadyen\tvisa\tvisadebit\tdebit\tAE\tAED\tVisa UAE Consumer Debit\t100\t0\t20\t5000\t170.0\t0.4\t900\tTHIN\n";
        assert!(segments_from_tsv(tsv)
            .get("m1")
            .is_none_or(|m| m.is_empty()));
    }

    /// A display key holding one flat product and one capped product must be priced by evaluating
    /// BOTH at the actual amount and then weighting — not by averaging the tiered product's headline,
    /// which would smuggle the flat-rate error back in.
    #[test]
    fn unresolved_card_product_blends_members_at_the_request_amount() {
        let blend = FineBlend {
            members: vec![
                // 1M of a plain 200 bps product.
                (
                    1_000_000.0,
                    MemberCost::Flat(ServingCost {
                        pct_bps: 200.0,
                        fixed: 0.0,
                    }),
                ),
                // 1M of the capped UAE debit ladder: 170.1 bps under the knot, 69.7 above.
                (1_000_000.0, MemberCost::Tiered(uae_debit())),
            ],
            total_weight: 2_000_000.0,
        };
        // Small ticket → the capped product is in its high-rate tier: (200 + 170.1) / 2.
        assert!((blend.for_amount(100.0).pct_bps - 185.05).abs() < 0.01);
        // Large ticket → it has crossed the cap: (200 + 69.7) / 2. The SAME key prices differently
        // by amount, which a single averaged line could never do.
        assert!((blend.for_amount(20_000.0).pct_bps - 134.85).abs() < 0.01);
    }

    /// The reported case: a 6,046 AED Visa debit with `cardIsin: null`. The stored cluster sits under
    /// `card_product = "100"`, so the exact key misses; the blend must still price it from the tier
    /// the amount lands in rather than abstaining to the seed table.
    #[test]
    fn missing_bin_still_prices_a_capped_cluster() {
        let blend = FineBlend {
            members: vec![(1_803_717.0, MemberCost::Tiered(uae_debit()))],
            total_weight: 1_803_717.0,
        };
        let cost = blend.for_amount(6_046.0);
        assert!(
            (cost.pct_bps - 69.7).abs() < 0.01,
            "6,046 lands in the upper tier"
        );
        assert!((cost.fixed - 50.72).abs() < 0.01);
        // ~153 bps all-in, vs the 137.8 bps the seed table was claiming for this route.
        assert!(
            cost.effective_cost_bps(6_046.0) > 145.0,
            "seed under-reported this route: got {}",
            cost.effective_cost_bps(6_046.0)
        );
    }

    /// The reported AED case: the settlement report spells the variant `visadebit`, but decide time
    /// reconstructs `visastandarddebit` from network+program+funding. Blanking the variant is what
    /// lets the two meet; everything else about the key already agrees.
    #[test]
    fn variant_blanked_key_bridges_a_report_spelling_we_cannot_reconstruct() {
        let stored = fine_key(
            "adyen",
            "visa",
            "visadebit",
            "debit",
            "AE",
            "AED",
            "Visa UAE Consumer Debit",
        );
        let decide_time = fine_key(
            "adyen",
            "visa",
            &reconstruct_variant("VISA", "STANDARD", "DEBIT", ""),
            "debit",
            "AE",
            "AED",
            "Visa UAE Consumer Debit",
        );
        assert_ne!(stored, decide_time, "the variants genuinely differ");
        assert_eq!(
            without_variant(&stored).unwrap(),
            without_variant(&decide_time).unwrap(),
            "blanking the variant makes them the same bucket"
        );
        assert_eq!(
            without_variant(&stored).unwrap(),
            "adyen|visa||debit|ae|aed|visa uae consumer debit"
        );
    }

    /// An `ic_category` containing a `|` must not shift the fields — the variant is located from the
    /// LEFT, so trailing content is untouched whatever it holds.
    #[test]
    fn without_variant_is_unaffected_by_separators_in_the_category() {
        let k = fine_key(
            "adyen",
            "visa",
            "visadebit",
            "debit",
            "AE",
            "AED",
            "odd|category",
        );
        assert_eq!(
            without_variant(&k).unwrap(),
            "adyen|visa||debit|ae|aed|odd|category"
        );
        // Too few fields to locate a variant ⇒ no key rather than a malformed one.
        assert!(without_variant("adyen|visa").is_none());
    }

    #[test]
    fn a_zero_weight_blend_cannot_divide_by_zero() {
        let blend = FineBlend {
            members: vec![],
            total_weight: 0.0,
        };
        assert_eq!(blend.for_amount(100.0).pct_bps, 0.0);
    }

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
    fn fine_model_key_extends_the_display_override_key_with_card_product() {
        let display = fine_key(
            "adyen",
            "mc",
            "mccommercialdebit",
            "commercial",
            "IT",
            "EUR",
            "Intra EEA Enhanced Electronic",
        );
        let k135 = fine_key_model(&display, "135");
        let k180 = fine_key_model(&display, "180");
        // The fan's two tiers get distinct learned keys, but both extend the one display key a
        // cluster override is set on — so an override still covers the whole displayed cluster.
        assert_ne!(k135, k180);
        assert!(k135.starts_with(&display));
        assert!(k180.starts_with(&display));
        // An unresolved BIN (card_product "") yields the blended-cluster key, not a fan tier.
        assert_eq!(fine_key_model(&display, ""), format!("{display}|"));
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
        // Log buckets (10/decade): bucket k = floor(log10(amount)*10). Currency-native resolution.
        assert_eq!(amount_band(1.0), "0"); // 10^0
        assert_eq!(amount_band(10.0), "10"); // 10^1
        assert_eq!(amount_band(100.0), "20"); // 10^2
        assert_eq!(amount_band(1000.0), "30"); // 10^3
                                               // adjacent amounts share a bucket; a decade later is +10 buckets, in ANY currency
        assert_eq!(amount_band(100.0), amount_band(120.0));
        assert_eq!(amount_band(0.0), "0"); // guard
    }
}
