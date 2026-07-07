//! ClickHouse sink for staged settlement rows.
//!
//! The app's analytics ClickHouse client is read-only (writes go via Kafka), so for this
//! once-daily batch we bulk-insert directly over HTTP with `FORMAT JSONEachRow`. The column list
//! omits `ingested_at` so its `DEFAULT now()` applies, and dates are plain "YYYY-MM-DD" strings —
//! sidestepping RowBinary/time-serde friction. See `scratch/inhouse-cost-architecture.md` §7.

use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;
use serde_json::json;

use crate::config::ClickHouseAnalyticsConfig;

use super::types::{IngestError, SettledFeeRow};

const INSERT_TIMEOUT: Duration = Duration::from_secs(60);

/// Columns we provide; `ingested_at` is intentionally omitted so ClickHouse applies its DEFAULT.
const COLUMNS: &str = "connector,account,merchant_id,txn_date,ingestion_id,txn_ref,card_network,variant,\
funding,issuer_country,currency,ic_category,channel,gross,total_fee,interchange,scheme_fee,markup,commission";

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(INSERT_TIMEOUT)
            .build()
            .expect("failed to build clickhouse sink client")
    })
}

/// Bulk-insert the normalized rows for one report into `settlement_txn_fees`, stamped with the
/// ingestion context. Each row's `txn_date` is its own transaction (booking) date; rows whose
/// report carries no date fall back to `fallback_date` (the ingestion date). `ingestion_id` ties
/// every row to its `cost_ingestion` job so an ingestion can later be deleted precisely. Returns
/// the number of rows written.
pub async fn insert_settled_rows(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    fallback_date: &str,
    ingestion_id: i64,
    rows: &[SettledFeeRow],
) -> Result<usize, IngestError> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut body = String::with_capacity(rows.len() * 256);
    for r in rows {
        // Per-transaction date drives the fit window; blank-date rows fall back to the ingest date.
        let txn_date = r
            .txn_date
            .map(|d| d.to_string())
            .unwrap_or_else(|| fallback_date.to_string());
        let obj = json!({
            "connector": connector,
            "account": account,
            "merchant_id": merchant_id,
            "txn_date": txn_date,
            "ingestion_id": ingestion_id,
            "txn_ref": r.txn_ref,
            "card_network": r.card_network,
            "variant": r.variant,
            "funding": r.funding,
            "issuer_country": r.issuer_country,
            "currency": r.currency,
            "ic_category": r.ic_category,
            "channel": r.channel,
            "gross": r.gross,
            "total_fee": r.total_fee,
            "interchange": r.interchange,
            "scheme_fee": r.scheme_fee,
            "markup": r.markup,
            "commission": r.commission,
        });
        body.push_str(&serde_json::to_string(&obj).map_err(|e| IngestError::Storage(e.to_string()))?);
        body.push('\n');
    }

    let query = format!(
        "INSERT INTO {}.settlement_txn_fees ({COLUMNS}) FORMAT JSONEachRow",
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

/// Delete the staged transactions an ingestion contributed, identified by its `ingestion_id`. Since
/// staging dedups by `txn_ref` keeping the latest `ingested_at`, a transaction later re-provided by
/// another ingestion is owned by *that* job's id — so deleting job X only removes the transactions
/// still attributable to X. The caller then refits so the served model reflects what remains.
pub async fn delete_ingestion_rows(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    ingestion_id: i64,
) -> Result<(), IngestError> {
    let sql = format!(
        "DELETE FROM {}.settlement_txn_fees WHERE connector = {{connector:String}} \
         AND account = {{account:String}} AND merchant_id = {{merchant_id:String}} \
         AND ingestion_id = {{ingestion_id:Int64}}",
        cfg.database
    );
    let id = ingestion_id.to_string();
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&[
            ("param_connector", connector),
            ("param_account", account),
            ("param_merchant_id", merchant_id),
            ("param_ingestion_id", id.as_str()),
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
