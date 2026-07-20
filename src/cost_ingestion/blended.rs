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
    pub interchange_bps: String,
    pub segment_idx: u16,
    pub amount_lo: f64,
    pub amount_hi: f64,
    pub pct_bps: f64,
    pub fixed: f64,
    pub grade_bps: f64,
    pub pct_ci95_bps: f64,
    pub crossover_amount: f64,
    pub prop_bps: f64,
    pub fix_abs: f64,
    pub fix_bps: f64,
    pub below_gross_frac: f64,
    pub fan_frac: f64,
    pub fan_money_bps: f64,
    /// Transaction count (so a small-ticket/high-txn segment stays visible next to GMV).
    pub n: u64,
    /// Settled GMV — the ranking weight.
    pub gross_sum: f64,
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
}

// Highest-GMV GOOD segments, ranked by settled money (gross_sum). Rolled up across `variant` (the
// card program/tier) — within a connector + interchange category variants price the same, so keeping
// them split only produces confusing duplicate rows. Fees are volume-weighted; n/gross summed.
// `{snapshot_filter}` is either an exact (connector, account, report_date) match (scoped) or the
// "latest snapshot per connector" subquery.
// One row per fitted segment (grouped including `variant`, so card products stay distinct). Aggregate
// outputs are aliased to distinct names (`blended_*`, `txns`, `total_gross`) so they don't shadow the
// `gross_sum` column in WHERE — reusing a column name as an aggregate alias trips ClickHouse's
// ILLEGAL_AGGREGATION check. Parsing below is positional, so the names are free.
const TOP_CLUSTERS_SQL: &str = r#"
SELECT
    connector, card_network, variant, funding, issuer_country, currency, ic_category,
    interchange_bps, segment_idx, amount_lo, amount_hi,
    sum(pct_bps * gross_sum) / sum(gross_sum) AS blended_pct_bps,
    sum(fixed * gross_sum)   / sum(gross_sum) AS blended_fixed,
    sum(grade_bps * gross_sum) / sum(gross_sum) AS blended_grade_bps,
    max(pct_ci95_bps)        AS pct_ci95_bps,
    max(crossover_amount)    AS crossover_amount,
    sum(prop_bps * gross_sum) / sum(gross_sum) AS blended_prop_bps,
    sum(fix_abs * gross_sum) / sum(gross_sum) AS blended_fix_abs,
    sum(fix_bps * gross_sum) / sum(gross_sum) AS blended_fix_bps,
    sum(below_gross_frac * gross_sum) / sum(gross_sum) AS blended_below_gross_frac,
    max(fan_frac)            AS fan_frac,
    max(fan_money_bps)       AS fan_money_bps,
    sum(n)                   AS txns,
    sum(gross_sum)           AS total_gross
FROM __DB__.cost_fee_model FINAL
WHERE verdict = 'GOOD' AND gross_sum > 0 AND merchant_id = {merchant_id:String}{snapshot_filter}
GROUP BY connector, card_network, variant, funding, issuer_country, currency, ic_category,
         interchange_bps, segment_idx, amount_lo, amount_hi
ORDER BY total_gross DESC
LIMIT {limit:UInt32}
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
    filter
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| super::ch_http::client(TIMEOUT))
}

/// The merchant's highest-GMV GOOD segments ranked by settled volume, narrowed by `scope` (empty =
/// merchant-wide; connector/account = that connector's latest snapshot; + report_date = one exact
/// ingested snapshot).
pub async fn top_clusters(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
    limit: u32,
    scope: ClusterScope<'_>,
) -> Result<Vec<TopCluster>, IngestError> {
    let sql = TOP_CLUSTERS_SQL
        .replace("{snapshot_filter}", &build_snapshot_filter(&scope))
        .replace("__DB__", &cfg.database);
    let limit_s = limit.to_string();
    let mut params: Vec<(&str, &str)> = vec![
        ("param_merchant_id", merchant_id),
        ("param_limit", &limit_s),
    ];
    // Bind only the params the filter actually references.
    if let Some(c) = scope.connector {
        params.push(("param_connector", c));
    }
    if let Some(a) = scope.account {
        params.push(("param_account", a));
    }
    if let Some(d) = scope.report_date {
        params.push(("param_report_date", d));
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
        if f.len() < 24 {
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
            interchange_bps: f[7].trim().to_string(),
            segment_idx: f[8].trim().parse().unwrap_or(0),
            amount_lo: f[9].trim().parse().unwrap_or(0.0),
            amount_hi: f[10].trim().parse().unwrap_or(0.0),
            pct_bps: f[11].trim().parse().unwrap_or(0.0),
            fixed: f[12].trim().parse().unwrap_or(0.0),
            grade_bps: f[13].trim().parse().unwrap_or(0.0),
            pct_ci95_bps: f[14].trim().parse().unwrap_or(0.0),
            crossover_amount: f[15].trim().parse().unwrap_or(0.0),
            prop_bps: f[16].trim().parse().unwrap_or(0.0),
            fix_abs: f[17].trim().parse().unwrap_or(0.0),
            fix_bps: f[18].trim().parse().unwrap_or(0.0),
            below_gross_frac: f[19].trim().parse().unwrap_or(0.0),
            fan_frac: f[20].trim().parse().unwrap_or(0.0),
            fan_money_bps: f[21].trim().parse().unwrap_or(0.0),
            n: f[22].trim().parse().unwrap_or(0),
            gross_sum: f[23].trim().parse().unwrap_or(0.0),
        });
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
