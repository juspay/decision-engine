//! Access to the unified `cost_ingestion` table — work queue, live progress, and history in one.
//!
//! Two entry points create rows: the webhook route enqueues a `pending` job the worker claims, and
//! a manual upload inserts a `processing` row it runs itself. Both then tick `staged_rows` for
//! progress and, on completion, record the ingested report's shape (period, currencies, countries,
//! volume, fit outcome). Connector-generic — every row carries its `connector`. See
//! `scratch/inhouse-cost-architecture.md` §7.

use async_bb8_diesel::AsyncRunQueryDsl;
use chrono::Datelike;
use diesel::associations::HasTable;
use diesel::*;

#[cfg(feature = "mysql")]
use crate::storage::schema::cost_ingestion::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::cost_ingestion::dsl;

use crate::app::get_tenant_app_state;
use crate::generics;
use crate::storage::types::{
    CostIngestion, CostIngestionNew, CostIngestionOutcomeUpdate, CostIngestionProgressUpdate,
    CostIngestionStatusUpdate,
};
use crate::storage::utils::generate_uuid;

use super::types::IngestError;

/// The full shape of a finished ingestion, recorded for history when a job completes.
pub struct Completion {
    pub staged_rows: i64,
    pub report_date: Option<chrono::NaiveDate>,
    pub period_start: Option<chrono::NaiveDate>,
    pub period_end: Option<chrono::NaiveDate>,
    /// Distinct settlement currencies seen in the report (sorted).
    pub currencies: Vec<String>,
    /// Distinct issuer countries seen in the report (sorted).
    pub countries: Vec<String>,
    pub total_gross: f64,
    pub total_clusters: i64,
    pub good_clusters: i64,
}

/// Enqueue a report discovered automatically — either pushed by a connector webhook (`source =
/// "webhook"`) or found by polling a connector's API (`source = "poll"`). **Idempotent** on
/// `(connector, notification_id)`: a re-delivered/re-listed report is a no-op. Returns `true` when a
/// new job was created.
pub async fn enqueue_pending(
    connector: &str,
    account: &str,
    merchant_id: &str,
    notification_id: &str,
    report_ref: &str,
    source: &str,
) -> Result<bool, IngestError> {
    let app_state = get_tenant_app_state().await;

    // The UNIQUE (connector, notification_id) constraint is the real guard; this check keeps a
    // duplicate delivery from erroring in the common case.
    let existing = generics::generic_find_one_optional::<
        <CostIngestion as HasTable>::Table,
        _,
        CostIngestion,
    >(
        &app_state.db,
        dsl::connector
            .eq(connector.to_string())
            .and(dsl::notification_id.eq(Some(notification_id.to_string()))),
    )
    .await
    .map_err(|e| IngestError::Storage(e.to_string()))?;

    if existing.is_some() {
        return Ok(false);
    }

    let new = CostIngestionNew {
        id: generate_uuid(),
        merchant_id: merchant_id.to_string(),
        connector: connector.to_string(),
        account: account.to_string(),
        source: source.to_string(),
        notification_id: Some(notification_id.to_string()),
        report_ref: report_ref.to_string(),
        status: "pending".to_string(),
    };
    generics::generic_insert::<<CostIngestion as HasTable>::Table, _>(&app_state.db, new)
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    Ok(true)
}

/// Create a `processing` row for a manual upload and return its id, so the caller's background task
/// can report progress and record the outcome against it. The id is generated client-side (UUIDv7),
/// so it's known up front — no read-back of the freshly-inserted row is needed.
pub async fn create_manual(
    merchant_id: &str,
    connector: &str,
    account: &str,
    report_ref: &str,
) -> Result<String, IngestError> {
    let app_state = get_tenant_app_state().await;
    let id = generate_uuid();
    let new = CostIngestionNew {
        id: id.clone(),
        merchant_id: merchant_id.to_string(),
        connector: connector.to_string(),
        account: account.to_string(),
        source: "manual".to_string(),
        notification_id: None,
        report_ref: report_ref.to_string(),
        status: "processing".to_string(),
    };
    generics::generic_insert::<<CostIngestion as HasTable>::Table, _>(&app_state.db, new)
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    Ok(id)
}

/// Create a `processing` row for a **sample** run (the "Use a sample file" flow) and return its id.
/// Identical to [`create_manual`] but tagged `source = "sample"` so history/coverage can label it a
/// demo run; it still shares the pipeline, progress ticking, and undo path (`delete_ingestion`).
pub async fn create_sample(
    merchant_id: &str,
    connector: &str,
    account: &str,
    report_ref: &str,
) -> Result<String, IngestError> {
    let app_state = get_tenant_app_state().await;
    let id = generate_uuid();
    let new = CostIngestionNew {
        id: id.clone(),
        merchant_id: merchant_id.to_string(),
        connector: connector.to_string(),
        account: account.to_string(),
        source: "sample".to_string(),
        notification_id: None,
        report_ref: report_ref.to_string(),
        status: "processing".to_string(),
    };
    generics::generic_insert::<<CostIngestion as HasTable>::Table, _>(&app_state.db, new)
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    Ok(id)
}

/// Claim up to `limit` pending jobs by compare-and-swapping each `pending → processing`. The CAS
/// (`WHERE id = ? AND status = 'pending'` affecting exactly one row) makes this safe with multiple
/// workers without `FOR UPDATE SKIP LOCKED`: a row another worker already flipped updates zero rows
/// here and is skipped.
pub async fn claim_pending(limit: usize) -> Result<Vec<CostIngestion>, IngestError> {
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| IngestError::Storage("db connection".to_string()))?;

    // Oldest-first, capped in SQL — a large backlog stays cheap and jobs are claimed in order.
    let pending: Vec<CostIngestion> = dsl::cost_ingestion
        .filter(dsl::status.eq("pending".to_string()))
        .order(dsl::created_at.asc())
        .limit(limit as i64)
        .get_results_async(&*conn)
        .await
        .map_err(|e| IngestError::Storage(format!("{e:?}")))?;

    let now = crate::utils::date_time::now();
    let mut claimed = Vec::new();
    for row in pending.into_iter() {
        let conn = app_state
            .db
            .get_conn()
            .await
            .map_err(|_| IngestError::Storage("db connection".to_string()))?;
        let won = generics::generic_update_if_present::<<CostIngestion as HasTable>::Table, _, _>(
            &conn,
            dsl::id
                .eq(row.id.clone())
                .and(dsl::status.eq("pending".to_string())),
            CostIngestionStatusUpdate {
                status: "processing".to_string(),
                last_error: None,
                updated_at: now,
            },
        )
        .await
        .map_err(|e| IngestError::Storage(format!("{e:?}")))?;
        if won == 1 {
            claimed.push(row);
        }
    }
    Ok(claimed)
}

/// Bump the staged-row counter the dashboard polls for progress.
pub async fn update_progress(id: &str, staged_rows: i64) -> Result<(), IngestError> {
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| IngestError::Storage("db connection".to_string()))?;
    generics::generic_update_if_present::<<CostIngestion as HasTable>::Table, _, _>(
        &conn,
        dsl::id.eq(id.to_string()),
        CostIngestionProgressUpdate {
            staged_rows,
            updated_at: crate::utils::date_time::now(),
        },
    )
    .await
    .map_err(|e| IngestError::Storage(format!("{e:?}")))?;
    Ok(())
}

/// Mark a job `completed` and record its full outcome for history.
pub async fn mark_completed(id: &str, c: &Completion) -> Result<(), IngestError> {
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| IngestError::Storage("db connection".to_string()))?;
    let update = CostIngestionOutcomeUpdate {
        status: "completed".to_string(),
        staged_rows: c.staged_rows,
        report_date: c.report_date.and_then(to_time_date),
        period_start: c.period_start.and_then(to_time_date),
        period_end: c.period_end.and_then(to_time_date),
        currency_count: c.currencies.len() as i32,
        currencies: Some(c.currencies.join(",")),
        country_count: c.countries.len() as i32,
        countries: Some(c.countries.join(",")),
        total_gross: c.total_gross,
        total_clusters: c.total_clusters,
        good_clusters: c.good_clusters,
        updated_at: crate::utils::date_time::now(),
    };
    generics::generic_update_if_present::<<CostIngestion as HasTable>::Table, _, _>(
        &conn,
        dsl::id.eq(id.to_string()),
        update,
    )
    .await
    .map_err(|e| IngestError::Storage(format!("{e:?}")))?;
    Ok(())
}

/// Mark a job `failed`, recording the error for operators / the dashboard.
pub async fn mark_failed(id: &str, error: &str) -> Result<(), IngestError> {
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| IngestError::Storage("db connection".to_string()))?;
    generics::generic_update_if_present::<<CostIngestion as HasTable>::Table, _, _>(
        &conn,
        dsl::id.eq(id.to_string()),
        CostIngestionStatusUpdate {
            status: "failed".to_string(),
            last_error: Some(error.to_string()),
            updated_at: crate::utils::date_time::now(),
        },
    )
    .await
    .map_err(|e| IngestError::Storage(format!("{e:?}")))?;
    Ok(())
}

/// A merchant's ingestion history (and any in-flight jobs), newest first.
pub async fn list_for_merchant(
    merchant_id: &str,
    limit: i64,
) -> Result<Vec<CostIngestion>, IngestError> {
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| IngestError::Storage("db connection".to_string()))?;
    let rows: Vec<CostIngestion> = dsl::cost_ingestion
        .filter(dsl::merchant_id.eq(merchant_id.to_string()))
        .order(dsl::created_at.desc())
        .limit(limit)
        .get_results_async(&*conn)
        .await
        .map_err(|e| IngestError::Storage(format!("{e:?}")))?;
    Ok(rows)
}

/// A single ingestion by id, scoped to the merchant (used for progress polling).
pub async fn get_for_merchant(
    merchant_id: &str,
    id: &str,
) -> Result<Option<CostIngestion>, IngestError> {
    let app_state = get_tenant_app_state().await;
    generics::generic_find_one_optional::<<CostIngestion as HasTable>::Table, _, CostIngestion>(
        &app_state.db,
        dsl::id
            .eq(id.to_string())
            .and(dsl::merchant_id.eq(merchant_id.to_string())),
    )
    .await
    .map_err(|e| IngestError::Storage(e.to_string()))
}

/// Hard-delete an ingestion's history row, scoped to the merchant. Returns `true` if a row was
/// removed. The caller deletes the ClickHouse data separately (see `sink::delete_snapshot`).
pub async fn delete(merchant_id: &str, id: &str) -> Result<bool, IngestError> {
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| IngestError::Storage("db connection".to_string()))?;
    let n = generics::generic_delete::<<CostIngestion as HasTable>::Table, _>(
        &conn,
        dsl::id
            .eq(id.to_string())
            .and(dsl::merchant_id.eq(merchant_id.to_string())),
    )
    .await
    .map_err(|e| IngestError::Storage(format!("{e:?}")))?;
    Ok(n > 0)
}

/// `chrono::NaiveDate` → `time::Date` (the DB date type). Uses the day-of-year, valid for any date.
fn to_time_date(d: chrono::NaiveDate) -> Option<time::Date> {
    time::Date::from_ordinal_date(d.year(), d.ordinal() as u16).ok()
}
