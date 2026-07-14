//! ClickHouse sink for aggregated settlement statistics.
//!
//! The app's analytics ClickHouse client is read-only (writes go via Kafka), so for this
//! once-daily batch we bulk-insert directly over HTTP with `FORMAT JSONEachRow`. The column list
//! omits `ingested_at` so its `DEFAULT now()` applies, and dates are plain "YYYY-MM-DD" strings —
//! sidestepping RowBinary/time-serde friction. Individual transactions are never written: the
//! pipeline aggregates a report into per-day sufficient statistics (see [`super::rollup`]) and this
//! sink writes those `cost_daily_stats` buckets. See `scratch/inhouse-cost-architecture.md` §7 and
//! `scratch/settlement-table-removal-worked-example.md`.

use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;
use serde_json::json;

use crate::config::ClickHouseAnalyticsConfig;

use super::rollup::DailyStatRow;
use super::types::IngestError;

const INSERT_TIMEOUT: Duration = Duration::from_secs(60);

/// Columns we provide; `ingested_at` is intentionally omitted so ClickHouse applies its DEFAULT.
const COLUMNS: &str =
    "connector,account,merchant_id,txn_date,ingestion_id,card_network,variant,funding,\
issuer_country,currency,ic_category,channel,band,n,sx,sy,sxx,sxy,syy,su,suu,suy,suuy,syyuu";

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(INSERT_TIMEOUT)
            .build()
            .expect("failed to build clickhouse sink client")
    })
}

/// Bulk-insert one report's aggregated per-day buckets into `cost_daily_stats`, stamped with the
/// ingestion context. Each bucket's `txn_date` is a transaction (booking) day. `ingestion_id` ties
/// every bucket to its `cost_ingestion` job so an ingestion can later be deleted. A day re-delivered
/// by a later report collapses onto the same key — the latest `ingested_at` wins (see the table's
/// `ReplacingMergeTree` in `035_cost_model.sh`). Returns the number of buckets written.
pub async fn insert_daily_stats(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    ingestion_id: &str,
    rows: &[DailyStatRow],
) -> Result<usize, IngestError> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut body = String::with_capacity(rows.len() * 256);
    for r in rows {
        let obj = json!({
            "connector": connector,
            "account": account,
            "merchant_id": merchant_id,
            "txn_date": r.txn_date.to_string(),
            "ingestion_id": ingestion_id,
            "card_network": r.card_network,
            "variant": r.variant,
            "funding": r.funding,
            "issuer_country": r.issuer_country,
            "currency": r.currency,
            "ic_category": r.ic_category,
            "channel": r.channel,
            "band": r.band,
            "n": r.n,
            "sx": r.sx,
            "sy": r.sy,
            "sxx": r.sxx,
            "sxy": r.sxy,
            "syy": r.syy,
            "su": r.su,
            "suu": r.suu,
            "suy": r.suy,
            "suuy": r.suuy,
            "syyuu": r.syyuu,
        });
        body.push_str(
            &serde_json::to_string(&obj).map_err(|e| IngestError::Storage(e.to_string()))?,
        );
        body.push('\n');
    }

    let query = format!(
        "INSERT INTO {}.cost_daily_stats ({COLUMNS}) FORMAT JSONEachRow",
        cfg.database
    );
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&[("query", query.as_str())])
        .body(body);
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
            "clickhouse insert failed ({status}): {text}"
        )));
    }
    Ok(rows.len())
}

/// Delete the daily buckets an ingestion last wrote, identified by its `ingestion_id`, then the
/// caller refits so the served model reflects what remains. Because a day re-delivered by a later
/// report is owned by *that* job (it overwrote this one's bucket), deleting job X removes only the
/// days still attributable to X. Caveat of the per-day model: undoing a report that *superseded* an
/// earlier report's day drops that day until it is re-ingested — the raw-row era could resurrect it,
/// the object-storage replay layer (deferred) will restore that.
pub async fn delete_ingestion_rows(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    ingestion_id: &str,
) -> Result<(), IngestError> {
    let sql = format!(
        "DELETE FROM {}.cost_daily_stats WHERE connector = {{connector:String}} \
         AND account = {{account:String}} AND merchant_id = {{merchant_id:String}} \
         AND ingestion_id = {{ingestion_id:String}}",
        cfg.database
    );
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&[
            ("param_connector", connector),
            ("param_account", account),
            ("param_merchant_id", merchant_id),
            ("param_ingestion_id", ingestion_id),
        ])
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
            "clickhouse delete staging failed ({status}): {text}"
        )));
    }
    Ok(())
}
