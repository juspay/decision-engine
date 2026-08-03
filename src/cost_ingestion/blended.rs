//! Per-connector blended fee from the latest fitted snapshot.
//!
//! Aggregates a merchant's GOOD `cost_fee_model` clusters (each connector's most recent snapshot)
//! into one volume-weighted `{pct_bps, fixed}` per connector — the headline "what does this
//! connector cost me on average" number shown on the dashboard next to each connector. It is
//! purely a display roll-up of what the router already serves fine-grained; the manual override
//! (`super::overrides`) is layered on top by the route handler.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;

use crate::config::ClickHouseAnalyticsConfig;

use super::types::IngestError;

const TIMEOUT: Duration = Duration::from_secs(30);

/// Volume-weighted blended cost for one connector, plus the settled volume it was fit from.
#[derive(Debug, Clone)]
pub struct ConnectorBlend {
    pub pct_bps: f64,
    pub fixed: f64,
    /// GOOD-cluster settled gross the blend was weighted by (a rough "how much do we trust this").
    pub good_gross: f64,
    /// An account this connector was fit from (the latest snapshot's). Lets the dashboard offer the
    /// per-segment drill-down for connectors that were ingested via manual upload (which stores no
    /// credentials), not just credentialed ones.
    pub account: Option<String>,
}

// Same latest-snapshot-per-(connector,account) selection the serving refresh uses, rolled up to
// one row per connector (volume-weighted across its networks/tiers/accounts).
const BLEND_SQL: &str = r#"
SELECT
    connector,
    sum(pct_bps * gross_sum) / sum(gross_sum) AS pct_bps,
    sum(fixed * gross_sum)   / sum(gross_sum) AS fixed,
    sum(gross_sum)           AS good_gross,
    any(account)             AS fit_account
FROM __DB__.cost_fee_model FINAL
WHERE verdict = 'GOOD' AND gross_sum > 0 AND merchant_id = {merchant_id:String}
  AND (merchant_id, connector, account, report_date) IN (
      SELECT merchant_id, connector, account, max(report_date)
      FROM __DB__.cost_fee_model
      WHERE merchant_id = {merchant_id:String}
      GROUP BY merchant_id, connector, account)
GROUP BY connector
FORMAT TSV
"#;

/// One fitted cost segment. `variant` (the card program/tier, e.g. `visastandarddebit` vs
/// `visasuperpremiumdebit`) is kept as its own dimension so the dashboard can distinguish products
/// that share a category — they price the same, but a merchant still wants to see them.
#[derive(Debug, Clone)]
pub struct TopCluster {
    pub connector: String,
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub issuer_country: String,
    pub currency: String,
    pub ic_category: String,
    pub pct_bps: f64,
    pub fixed: f64,
    /// Transaction count (so a small-ticket/high-txn segment stays visible next to GMV).
    pub n: u64,
    /// Settled GMV — the ranking weight.
    pub gross_sum: f64,
    /// Fit grade: `"GOOD"` | `"THIN"` | `"NON_LINEAR"`. The grade of the row carrying the most
    /// settled money in this rollup (see `TOP_CLUSTERS_SQL`). Non-GOOD clusters are returned so the
    /// dashboard can show and correct them; only GOOD ones are trusted by the routing blend.
    pub verdict: String,
    /// The issuer BIN's dominant interchange rate as an integer-bps string (`"115"`, `"190"`), or
    /// `""` for a report with no PAN to resolve a BIN from. A card-program proxy: within one
    /// network+funding+country, the rate tier *is* the program (Classic vs Platinum vs commercial).
    pub card_product: String,
}

/// Narrow the top-clusters query. Any subset of fields may be set:
///  - all empty → merchant-wide, latest snapshot per connector (nothing set),
///  - `connector` (+ `account`) only → that connector/account's *latest* snapshot (the override
///    targets shown under a connector),
///  - `report_date` too → one exact ingested snapshot (a specific ingestion's segments).
///
/// When `report_date` is unset we still restrict to the latest snapshot per (connector, account), so
/// a connector-only scope returns its current segments rather than every historical fit.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClusterScope<'a> {
    pub connector: Option<&'a str>,
    pub account: Option<&'a str>,
    pub report_date: Option<&'a str>,
    /// Narrow to the clusters ONE ingestion contributed transactions to.
    ///
    /// `(connector, account, report_date)` alone cannot do this: the fit is a rolling-window re-fit
    /// that replaces the whole snapshot, so two uploads on the same day under the same account share
    /// one snapshot. Listing a 4.2k-row UAE report's "fitted segments" then showed all 1,109 display
    /// clusters, 1,103 of them from an earlier European report. `cost_daily_stats.ingestion_id`
    /// records which ingest last wrote each bucket, which is the only per-upload attribution there is.
    pub ingestion_id: Option<&'a str>,
}

/// Restrict to cluster keys one ingestion touched. Written against the shared cluster columns so it
/// applies unchanged to `cost_fee_model` and `cost_fee_model_segment`.
///
/// Caveat inherent to the source: `cost_daily_stats` is a `ReplacingMergeTree` keyed on the bucket,
/// so a later report re-delivering the same day REWRITES those buckets with its own id. Attribution
/// therefore reflects whichever ingest most recently wrote each bucket, not ingest history.
const INGESTION_FILTER: &str = r#"
  AND (card_network, variant, funding, issuer_country, currency, ic_category, card_product) IN (
      SELECT DISTINCT card_network, variant, funding, issuer_country, currency, ic_category,
          card_product
      FROM __DB__.cost_daily_stats FINAL
      WHERE merchant_id = {merchant_id:String} AND ingestion_id = {ingestion_id:String})"#;

/// Per-column narrowing of the cluster list. Every field is an EXACT, case-insensitive match on one
/// of the seven display dimensions, plus `verdict` and a free-text `q` that scans all of them.
///
/// This exists because a merchant's cluster list is long-tailed — a real single-account report fits
/// 1,566 clusters, of which 1,408 carry fewer than 10 transactions each. Ranking by GMV and cutting
/// at a top-N makes that tail unreachable, yet a THIN cluster is exactly the one worth giving a
/// contract rate to. Filtering happens in ClickHouse (not in the browser) so the tail stays reachable
/// without shipping every row: the same query serves a merchant with 20k clusters.
///
/// `verdict` is matched against the GROUP's grade (`argMax(verdict, gross_sum)`), so it filters what
/// the row actually displays rather than any member row.
/// Every column is MULTI-valued — the values within one column are OR'd (`IN`), and the columns are
/// AND'd. So "Visa or Mastercard, in Italy" is one query. Each value becomes its own bound parameter
/// (`{f_ic_category_0:String}`, `_1`, …) rather than being packed into one delimited string: 44 of
/// this merchant's `ic_category` values contain a comma (`Intra-Europe Consumer - EEA,UK Merchant …`),
/// so any delimiter-joining scheme would split a legitimate value in half.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClusterFilter<'a> {
    pub card_network: &'a [String],
    pub variant: &'a [String],
    pub funding: &'a [String],
    pub issuer_country: &'a [String],
    pub currency: &'a [String],
    pub ic_category: &'a [String],
    pub verdict: &'a [String],
    /// Free-text substring, case-insensitive, across all seven dimensions at once.
    pub q: Option<&'a str>,
}

/// The grade a cluster is DISPLAYED as, which is what the Fit filter and its suggestions speak in.
/// Derived from the stored verdict plus whether usable tiers were recovered — never the raw enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitClass {
    Good,
    /// Stored `NON_LINEAR` (or `THIN`), but recovered into amount tiers the router prices with.
    Tiered,
    Thin,
    /// No single rate and no clean tiers.
    PoorFit,
}

impl FitClass {
    /// Accepts the display label the UI round-trips (`"Poor fit"`) and the raw enum, so an older
    /// bookmarked URL carrying `verdict=NON_LINEAR` keeps working.
    pub fn parse(s: &str) -> Option<Self> {
        match s
            .trim()
            .to_ascii_uppercase()
            .replace([' ', '-'], "_")
            .as_str()
        {
            "GOOD" => Some(Self::Good),
            "TIERED" => Some(Self::Tiered),
            "THIN" => Some(Self::Thin),
            "POOR_FIT" | "NON_LINEAR" => Some(Self::PoorFit),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "Good",
            Self::Tiered => "Tiered",
            Self::Thin => "Thin",
            Self::PoorFit => "Poor fit",
        }
    }
}

/// The GROUP BY key columns, in the order the filter fragments and the bindings both walk them —
/// keeping the two in lockstep is what guarantees every generated placeholder gets a value.
const FILTER_COLS: [&str; 6] = [
    "card_network",
    "variant",
    "funding",
    "issuer_country",
    "currency",
    "ic_category",
];

impl<'a> ClusterFilter<'a> {
    fn dim_values(&self) -> [&'a [String]; 6] {
        [
            self.card_network,
            self.variant,
            self.funding,
            self.issuer_country,
            self.currency,
            self.ic_category,
        ]
    }

    /// No column narrowed — the caller is asking for the plain top-N.
    pub fn is_empty(&self) -> bool {
        self.dim_values().iter().all(|v| v.is_empty())
            && self.verdict.is_empty()
            && self.q.is_none()
    }

    /// Predicates on the GROUP BY key columns — these belong in `WHERE` (pre-aggregation).
    /// `lower()` on both sides makes every match case-insensitive, matching the values the UI offers.
    fn where_sql(&self) -> String {
        let mut s = String::new();
        for (col, vals) in FILTER_COLS.iter().zip(self.dim_values()) {
            if vals.is_empty() {
                continue;
            }
            let list = (0..vals.len())
                .map(|i| format!("lower({{f_{col}_{i}:String}})"))
                .collect::<Vec<_>>()
                .join(", ");
            s.push_str(&format!(" AND lower({col}) IN ({list})"));
        }
        if self.q.is_some() {
            s.push_str(
                " AND (positionCaseInsensitive(connector, {f_q:String}) > 0 \
                 OR positionCaseInsensitive(card_network, {f_q:String}) > 0 \
                 OR positionCaseInsensitive(variant, {f_q:String}) > 0 \
                 OR positionCaseInsensitive(funding, {f_q:String}) > 0 \
                 OR positionCaseInsensitive(issuer_country, {f_q:String}) > 0 \
                 OR positionCaseInsensitive(currency, {f_q:String}) > 0 \
                 OR positionCaseInsensitive(ic_category, {f_q:String}) > 0)",
            );
        }
        s
    }

    /// `verdict` is an aggregate of the group, so it can only be filtered AFTER aggregation.
    /// `toString` is required: the column is an `Enum8`, and ClickHouse rejects `upper()` on an enum.
    /// `verdict` filters on the grade the TABLE SHOWS, not the raw enum stored in ClickHouse.
    ///
    /// The two differ for exactly one case, and it is the common one: a capped/tiered cluster is
    /// stored `NON_LINEAR` (that grades the single-line fit) but displays as **Tiered**, because the
    /// recovered amount tiers price it fine. Offering the raw enum meant the chip read `NON_LINEAR`
    /// — an internal name — and, worse, picking "Poor fit" would have returned rows badged Tiered.
    ///
    /// So the four filter values mirror the badge: `Good`, `Tiered`, `Thin`, `Poor fit`. `is_tiered`
    /// is computed per group in the SELECT (any member cluster having a GOOD segment), matching
    /// `coverage.rs` and `serving::segments_from_tsv`'s definition of "usable".
    fn having_sql(&self) -> String {
        if self.verdict.is_empty() {
            return String::new();
        }
        let mut clauses: Vec<String> = Vec::new();
        for v in self.verdict {
            clauses.push(match FitClass::parse(v) {
                Some(FitClass::Good) => "top_verdict = 'GOOD'".to_string(),
                Some(FitClass::Tiered) => "(top_verdict != 'GOOD' AND is_tiered)".to_string(),
                Some(FitClass::Thin) => "(top_verdict = 'THIN' AND NOT is_tiered)".to_string(),
                Some(FitClass::PoorFit) => {
                    "(top_verdict = 'NON_LINEAR' AND NOT is_tiered)".to_string()
                }
                // Unrecognised value: match nothing rather than everything, so a typo narrows to an
                // empty list instead of silently ignoring the filter.
                None => "0".to_string(),
            });
        }
        format!(" HAVING ({})", clauses.join(" OR "))
    }

    /// Bind exactly the placeholders the two fragments above generated. Names are owned because they
    /// are positional (`param_f_variant_2`) and so cannot be `'static`.
    fn bind(&self, params: &mut Vec<(String, String)>) {
        for (col, vals) in FILTER_COLS.iter().zip(self.dim_values()) {
            for (i, v) in vals.iter().enumerate() {
                params.push((format!("param_f_{col}_{i}"), v.clone()));
            }
        }
        for (i, v) in self.verdict.iter().enumerate() {
            params.push((format!("param_f_verdict_{i}"), v.clone()));
        }
        if let Some(q) = self.q {
            params.push(("param_f_q".to_string(), q.to_string()));
        }
    }
}

/// How the top-N is ranked before the `LIMIT` cuts it. This selects *which* segments appear, not just
/// their display order — top-20-by-GMV and top-20-by-txns can be materially different sets (a snapshot
/// full of small-value, high-count debit segments ranks very differently by money vs. count).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClusterOrder {
    /// Settled GMV — money moved (default; the cost-impact ranking).
    #[default]
    Gross,
    /// Transaction count.
    Txns,
}

impl ClusterOrder {
    /// The `ORDER BY` expression — a fixed SELECT-alias, so it's safe to interpolate into the SQL.
    fn column(self) -> &'static str {
        match self {
            Self::Gross => "total_gross",
            Self::Txns => "txns",
        }
    }
}

// Highest-GMV segments, ranked by settled money (gross_sum). NOT filtered to `verdict = 'GOOD'`:
// a THIN or NON_LINEAR cluster is exactly the one a merchant most wants to see and correct with a
// per-cluster override, so it's returned with its grade and badged in the UI. (The routing blend in
// `BLENDED_SQL` stays GOOD-only — this relaxation is for display/override, not for what we trust.)
// One row per fitted segment (grouped including `variant`, so card products stay distinct); the fit
// rolls up across `card_product`, which isn't a key here. Fees are volume-weighted; n/gross summed.
// `{snapshot_filter}` is either an exact (connector, account, report_date) match (scoped) or the
// "latest snapshot per connector" subquery. Aggregate outputs are aliased to distinct names
// (`blended_*`, `txns`, `total_gross`) so they don't shadow the `gross_sum` column in WHERE —
// reusing a column name as an aggregate alias trips ClickHouse's ILLEGAL_AGGREGATION check. Parsing
// below is positional, so the names are free.
// The rollup can span rows of differing grade (one `card_product` GOOD, another THIN), so the group's
// verdict is `argMax(verdict, gross_sum)` — the grade of the piece carrying the most money, i.e. the
// one the displayed blended rate mostly reflects. `card_product` (the issuer BIN's dominant
// interchange rate, in integer bps) is reported the same way: it is a fit key but NOT part of the
// seven-field display/override key, so the group can span several — the dominant one is the card
// program this cluster's money actually rode on.
//
// The weighted mean SKIPS unfittable rows (`isNaN(pct_bps)`). Because this query is deliberately not
// GOOD-only, a group routinely contains a one-transaction THIN row, and a single-point cluster has no
// amount spread — `denom = n·Σx² − (Σx)² = 0` — so the fit stores `pct_bps = nan`. NaN propagates
// through `sum()`, so ONE such row turned the whole group's rate into NaN and the UI rendered `—`
// beside a `Good` badge (the badge comes from `argMax(verdict, gross_sum)`, which correctly picks the
// dominant row). Real case: 8.76M of volume over 91,809 txns displaying no rate because of a single
// €780 transaction from an exotic issuer. Weighting only the rows that produced a rate is also the
// honest number — an unpriceable row has no rate to contribute. `txns`/`total_gross` still sum
// EVERY row, since those transactions did happen. When no row in a group is fittable, both weights
// collapse to 0/0 = NaN and `—` is then the truthful display.
const TOP_CLUSTERS_SQL: &str = r#"
WITH tiered_keys AS (
    SELECT DISTINCT card_network, variant, funding, issuer_country, currency, ic_category,
        card_product
    FROM __DB__.cost_fee_model_segment FINAL
    WHERE merchant_id = {merchant_id:String} AND verdict = 'GOOD'
)
SELECT
    connector, card_network, variant, funding, issuer_country, currency, ic_category,
    sum(if(isNaN(pct_bps), 0, pct_bps * gross_sum)) / sum(if(isNaN(pct_bps), 0, gross_sum)) AS blended_pct_bps,
    sum(if(isNaN(fixed),   0, fixed   * gross_sum)) / sum(if(isNaN(fixed),   0, gross_sum)) AS blended_fixed,
    sum(n)                   AS txns,
    sum(gross_sum)           AS total_gross,
    argMax(verdict, gross_sum) AS top_verdict,
    argMax(card_product, gross_sum) AS top_card_product,
    -- Does ANY member of this display group carry usable tiers? That is what makes the row badge
    -- "Tiered" rather than "Poor fit", so the Fit filter has to agree with it.
    max((card_network, variant, funding, issuer_country, currency, ic_category, card_product)
        IN (SELECT * FROM tiered_keys)) AS is_tiered
FROM __DB__.cost_fee_model FINAL
WHERE gross_sum > 0 AND merchant_id = {merchant_id:String}{snapshot_filter}{dim_filter}
GROUP BY connector, card_network, variant, funding, issuer_country, currency, ic_category
{having_filter}
ORDER BY {order_col} DESC
LIMIT {limit:UInt32}
FORMAT TSV
"#;

/// Distinct values per filterable column, with the transaction count behind each — the source of the
/// UI's per-column autosuggestions. Returned as `(dim, value, txns)` triples rather than one row of
/// arrays so the TSV stays trivially parseable. Ordered by `txns DESC` within each dimension, so the
/// suggestion list leads with the values that actually carry the merchant's traffic.
///
/// Narrowed by the same `{snapshot_filter}` as the cluster list, so suggestions on a snapshot-scoped
/// view only offer values present in that snapshot. Deliberately NOT narrowed by the active column
/// filters: a user clearing one filter should still see the full set of options for it.
const CLUSTER_FACETS_SQL: &str = r#"
WITH tiered_keys AS (
    SELECT DISTINCT card_network, variant, funding, issuer_country, currency, ic_category,
        card_product
    FROM __DB__.cost_fee_model_segment FINAL
    WHERE merchant_id = {merchant_id:String} AND verdict = 'GOOD'
),
src AS (
    SELECT connector, card_network, variant, funding, issuer_country, currency, ic_category, n,
        -- The DISPLAYED grade, not the raw enum: a capped cluster stores NON_LINEAR but shows as
        -- Tiered. Suggesting `NON_LINEAR` here leaked an internal name into the filter chip AND
        -- would have made "Poor fit" select rows the table badges Tiered.
        multiIf(
            verdict = 'GOOD', 'Good',
            (card_network, variant, funding, issuer_country, currency, ic_category, card_product)
                IN (SELECT * FROM tiered_keys), 'Tiered',
            verdict = 'THIN', 'Thin',
            'Poor fit'
        ) AS fit_class
    FROM __DB__.cost_fee_model FINAL
    WHERE gross_sum > 0 AND merchant_id = {merchant_id:String}{snapshot_filter}
)
SELECT dim, value, sum(cnt) AS txns
FROM (
    SELECT 'connector' AS dim,      connector      AS value, n AS cnt FROM src
    UNION ALL SELECT 'card_network',   card_network,   n FROM src
    UNION ALL SELECT 'variant',        variant,        n FROM src
    UNION ALL SELECT 'funding',        funding,        n FROM src
    UNION ALL SELECT 'issuer_country', issuer_country, n FROM src
    UNION ALL SELECT 'currency',       currency,       n FROM src
    UNION ALL SELECT 'ic_category',    ic_category,    n FROM src
    UNION ALL SELECT 'verdict',        fit_class,      n FROM src
)
WHERE value != ''
GROUP BY dim, value
ORDER BY dim, txns DESC
FORMAT TSV
"#;

// Restrict to the latest snapshot per (connector, account) — used whenever an exact `report_date`
// isn't pinned, so connector-scoped and merchant-wide views both show current segments.
const LATEST_SNAPSHOT_FILTER: &str = r#"
  AND (merchant_id, connector, account, report_date) IN (
      SELECT merchant_id, connector, account, max(report_date)
      FROM __DB__.cost_fee_model
      WHERE merchant_id = {merchant_id:String}
      GROUP BY merchant_id, connector, account)"#;

// `report_date` is a ClickHouse `Date`; the param arrives as a 'YYYY-MM-DD' String, so convert it
// (a bare `Date = String` comparison errors and would silently yield zero segments).
fn build_snapshot_filter(scope: &ClusterScope<'_>) -> String {
    let mut filter = String::new();
    if scope.connector.is_some() {
        filter.push_str(" AND connector = {connector:String}");
    }
    if scope.account.is_some() {
        filter.push_str(" AND account = {account:String}");
    }
    if scope.report_date.is_some() {
        // Exact snapshot — no need for the latest-per-connector restriction.
        filter.push_str(" AND report_date = toDate({report_date:String})");
    } else {
        filter.push_str(LATEST_SNAPSHOT_FILTER);
    }
    if scope.ingestion_id.is_some() {
        filter.push_str(INGESTION_FILTER);
    }
    filter
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| super::ch_http::client(TIMEOUT))
}

/// The merchant's top segments ranked by `order` (settled GMV by default, or transaction count),
/// narrowed by `scope` (empty = merchant-wide; connector/account = that connector's latest snapshot;
/// plus report_date = one exact ingested snapshot). Every grade is returned — read
/// `TopCluster::verdict` before treating a rate as trustworthy.
pub async fn top_clusters(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
    limit: u32,
    scope: ClusterScope<'_>,
    order: ClusterOrder,
    filter: ClusterFilter<'_>,
) -> Result<Vec<TopCluster>, IngestError> {
    let sql = TOP_CLUSTERS_SQL
        .replace("{snapshot_filter}", &build_snapshot_filter(&scope))
        .replace("{dim_filter}", &filter.where_sql())
        .replace("{having_filter}", &filter.having_sql())
        .replace("{order_col}", order.column())
        .replace("__DB__", &cfg.database);
    let mut params: Vec<(String, String)> = vec![
        ("param_merchant_id".into(), merchant_id.to_string()),
        ("param_limit".into(), limit.to_string()),
    ];
    // Bind only the params the filter actually references.
    if let Some(c) = scope.connector {
        params.push(("param_connector".into(), c.to_string()));
    }
    if let Some(a) = scope.account {
        params.push(("param_account".into(), a.to_string()));
    }
    if let Some(d) = scope.report_date {
        params.push(("param_report_date".into(), d.to_string()));
    }
    if let Some(i) = scope.ingestion_id {
        params.push(("param_ingestion_id".into(), i.to_string()));
    }
    filter.bind(&mut params);
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&params)
        .body(sql);
    if !cfg.user.is_empty() {
        req = req.basic_auth(&cfg.user, cfg.password.as_ref().map(|p| p.peek().clone()));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(IngestError::Storage(format!(
            "clickhouse top-clusters query failed ({status}): {text}"
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;

    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 13 {
            continue;
        }
        out.push(TopCluster {
            connector: f[0].trim().to_lowercase(),
            card_network: f[1].trim().to_string(),
            variant: f[2].trim().to_string(),
            funding: f[3].trim().to_string(),
            issuer_country: f[4].trim().to_string(),
            currency: f[5].trim().to_string(),
            ic_category: f[6].trim().to_string(),
            pct_bps: f[7].trim().parse().unwrap_or(0.0),
            fixed: f[8].trim().parse().unwrap_or(0.0),
            n: f[9].trim().parse().unwrap_or(0),
            gross_sum: f[10].trim().parse().unwrap_or(0.0),
            verdict: f[11].trim().to_string(),
            card_product: f[12].trim().to_string(),
        });
    }
    Ok(out)
}

/// One recovered piece of a capped/tiered cluster, for the per-segment dashboard view. Mirrors a
/// `cost_fee_model_segment` row. `pct_bps`/`fixed`/`bps_rmse` are optional (a degenerate single-band
/// piece is unfittable).
#[derive(Debug, Clone)]
pub struct ClusterSegment {
    pub seg_idx: u8,
    pub lo: f64,
    pub hi: f64,
    pub pct_bps: Option<f64>,
    pub fixed: Option<f64>,
    pub bps_rmse: Option<f64>,
    pub n: u64,
    pub gross_sum: f64,
    pub verdict: String,
}

/// A cluster a single line couldn't fit, recovered into an ordered list of segments. `n`/`gross_sum`
/// are summed from its pieces (the parent NON_LINEAR `cost_fee_model` row isn't needed for display).
#[derive(Debug, Clone)]
pub struct SegmentedCluster {
    pub connector: String,
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub issuer_country: String,
    pub currency: String,
    pub ic_category: String,
    pub n: u64,
    pub gross_sum: f64,
    pub segments: Vec<ClusterSegment>,
}

// Latest snapshot per (connector, account) restricted to the SEGMENT table, so a segmented cluster
// shows its current pieces (mirrors LATEST_SNAPSHOT_FILTER, which is over cost_fee_model).
const SEG_LATEST_SNAPSHOT_FILTER: &str = r#"
  AND (merchant_id, connector, account, report_date) IN (
      SELECT merchant_id, connector, account, max(report_date)
      FROM __DB__.cost_fee_model_segment
      WHERE merchant_id = {merchant_id:String}
      GROUP BY merchant_id, connector, account)"#;

// All segment rows for the merchant (optionally scoped), ordered so a cluster's pieces are
// contiguous and ascending — the parse groups on the seven key fields.
const SEGMENTS_SQL: &str = r#"
SELECT
    connector, card_network, variant, funding, issuer_country, currency, ic_category,
    seg_idx, lo, hi, pct_bps, fixed, bps_rmse, n, gross_sum, verdict
FROM __DB__.cost_fee_model_segment FINAL
WHERE merchant_id = {merchant_id:String}{snapshot_filter}
ORDER BY connector, card_network, variant, funding, issuer_country, currency, ic_category, seg_idx
FORMAT TSV
"#;

fn build_seg_snapshot_filter(scope: &ClusterScope<'_>) -> String {
    let mut filter = String::new();
    if scope.connector.is_some() {
        filter.push_str(" AND connector = {connector:String}");
    }
    if scope.account.is_some() {
        filter.push_str(" AND account = {account:String}");
    }
    if scope.report_date.is_some() {
        filter.push_str(" AND report_date = toDate({report_date:String})");
    } else {
        filter.push_str(SEG_LATEST_SNAPSHOT_FILTER);
    }
    if scope.ingestion_id.is_some() {
        filter.push_str(INGESTION_FILTER);
    }
    filter
}

/// Parse a ClickHouse Nullable(Float64) TSV cell — `\N` (or blank) is `None`.
fn opt_f64(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() || s == "\\N" {
        None
    } else {
        s.parse().ok()
    }
}

/// One selectable value for one filterable column, with the traffic behind it.
#[derive(Debug, Clone)]
pub struct ClusterFacet {
    /// Column this value belongs to: `connector` | `card_network` | `variant` | `funding` |
    /// `issuer_country` | `currency` | `ic_category` | `verdict`.
    pub dim: String,
    pub value: String,
    pub txns: u64,
}

/// Distinct values for every filterable column — what the UI offers as per-column autosuggestions.
/// Ordered so the highest-traffic values come first within each dimension.
pub async fn cluster_facets(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
    scope: ClusterScope<'_>,
) -> Result<Vec<ClusterFacet>, IngestError> {
    let sql = CLUSTER_FACETS_SQL
        .replace("{snapshot_filter}", &build_snapshot_filter(&scope))
        .replace("__DB__", &cfg.database);
    let mut params: Vec<(&str, &str)> = vec![("param_merchant_id", merchant_id)];
    if let Some(c) = scope.connector {
        params.push(("param_connector", c));
    }
    if let Some(a) = scope.account {
        params.push(("param_account", a));
    }
    if let Some(d) = scope.report_date {
        params.push(("param_report_date", d));
    }
    if let Some(i) = scope.ingestion_id {
        params.push(("param_ingestion_id", i));
    }
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&params)
        .body(sql);
    if !cfg.user.is_empty() {
        req = req.basic_auth(&cfg.user, cfg.password.as_ref().map(|p| p.peek().clone()));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(IngestError::Storage(format!(
            "clickhouse cluster-facets query failed ({status}): {text}"
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;

    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 3 {
            continue;
        }
        out.push(ClusterFacet {
            dim: f[0].trim().to_string(),
            value: f[1].trim().to_string(),
            txns: f[2].trim().parse().unwrap_or(0),
        });
    }
    Ok(out)
}

/// The merchant's capped/tiered clusters and their recovered per-segment rates, narrowed by `scope`
/// exactly like [`top_clusters`]. Empty when nothing has been segmented (the common case — most
/// clusters are GOOD and carry no segment rows).
pub async fn segmented_clusters(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
    scope: ClusterScope<'_>,
) -> Result<Vec<SegmentedCluster>, IngestError> {
    let sql = SEGMENTS_SQL
        .replace("{snapshot_filter}", &build_seg_snapshot_filter(&scope))
        .replace("__DB__", &cfg.database);
    let mut params: Vec<(&str, &str)> = vec![("param_merchant_id", merchant_id)];
    if let Some(c) = scope.connector {
        params.push(("param_connector", c));
    }
    if let Some(a) = scope.account {
        params.push(("param_account", a));
    }
    if let Some(d) = scope.report_date {
        params.push(("param_report_date", d));
    }
    if let Some(i) = scope.ingestion_id {
        params.push(("param_ingestion_id", i));
    }
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&params)
        .body(sql);
    if !cfg.user.is_empty() {
        req = req.basic_auth(&cfg.user, cfg.password.as_ref().map(|p| p.peek().clone()));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(IngestError::Storage(format!(
            "clickhouse segments query failed ({status}): {text}"
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;

    // Rows arrive grouped+ordered by the seven key fields, then seg_idx. Fold contiguous rows of the
    // same cluster into one SegmentedCluster.
    let mut out: Vec<SegmentedCluster> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 16 {
            continue;
        }
        let seg = ClusterSegment {
            seg_idx: f[7].trim().parse().unwrap_or(0),
            lo: f[8].trim().parse().unwrap_or(0.0),
            hi: f[9].trim().parse().unwrap_or(0.0),
            pct_bps: opt_f64(f[10]),
            fixed: opt_f64(f[11]),
            bps_rmse: opt_f64(f[12]),
            n: f[13].trim().parse().unwrap_or(0),
            gross_sum: f[14].trim().parse().unwrap_or(0.0),
            verdict: f[15].trim().to_string(),
        };
        let matches_last = out.last().is_some_and(|c| {
            c.connector == f[0].trim().to_lowercase()
                && c.card_network == f[1].trim()
                && c.variant == f[2].trim()
                && c.funding == f[3].trim()
                && c.issuer_country == f[4].trim()
                && c.currency == f[5].trim()
                && c.ic_category == f[6].trim()
        });
        if matches_last {
            let c = out.last_mut().expect("checked above");
            c.n += seg.n;
            c.gross_sum += seg.gross_sum;
            c.segments.push(seg);
        } else {
            out.push(SegmentedCluster {
                connector: f[0].trim().to_lowercase(),
                card_network: f[1].trim().to_string(),
                variant: f[2].trim().to_string(),
                funding: f[3].trim().to_string(),
                issuer_country: f[4].trim().to_string(),
                currency: f[5].trim().to_string(),
                ic_category: f[6].trim().to_string(),
                n: seg.n,
                gross_sum: seg.gross_sum,
                segments: vec![seg],
            });
        }
    }
    Ok(out)
}

/// The model-derived blended fee for each of a merchant's connectors (keyed by lowercase connector
/// name). Connectors with no GOOD clusters are simply absent.
pub async fn by_connector(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
) -> Result<HashMap<String, ConnectorBlend>, IngestError> {
    let sql = BLEND_SQL.replace("__DB__", &cfg.database);
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&[("param_merchant_id", merchant_id)])
        .body(sql);
    if !cfg.user.is_empty() {
        req = req.basic_auth(&cfg.user, cfg.password.as_ref().map(|p| p.peek().clone()));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(IngestError::Storage(format!(
            "clickhouse blend query failed ({status}): {text}"
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;

    let mut out = HashMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            continue;
        }
        let connector = f[0].trim().to_lowercase();
        let pct_bps: f64 = f[1].trim().parse().unwrap_or(0.0);
        let fixed: f64 = f[2].trim().parse().unwrap_or(0.0);
        let good_gross: f64 = f[3].trim().parse().unwrap_or(0.0);
        let account = f
            .get(4)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        if connector.is_empty() {
            continue;
        }
        out.insert(
            connector,
            ConnectorBlend {
                pct_bps,
                fixed,
                good_gross,
                account,
            },
        );
    }
    Ok(out)
}
