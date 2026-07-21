//! Manual settlement-report upload: `POST /merchant-account/:id/connectors/:connector/report`.
//!
//! Lets a merchant upload a report file directly (no webhook / no download) — useful before
//! webhooks are wired, for backfills, or for testing. The request body is streamed to a temp file
//! (capped, never buffered in memory), a `cost_ingestion` row is created, and processing runs in a
//! background task that ticks progress and records the outcome. The handler returns the job id
//! immediately so the dashboard can poll progress — a multi-GB report no longer hangs the request.
//!
//! Manual and webhook ingestions share the same `cost_ingestion` table and pipeline, so history and
//! progress are unified. (Single-node assumption: the background task reads the temp file on the
//! same host that received the upload.)

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::Json;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::cost_ingestion::{mapping, pipeline, sink, store};
use crate::logger;
use crate::storage::types::CostIngestion;

/// Upper bound on a single uploaded report. Monthly reports run a few GB; this leaves headroom
/// while still rejecting a runaway/accidental upload before it fills the disk.
pub const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct UploadParams {
    /// Connector-side account (e.g. Adyen `merchantAccountCode`) the report belongs to.
    pub account: String,
}

/// Returned immediately (202): the created job's id, which the dashboard polls for progress.
#[derive(Debug, Serialize)]
pub struct UploadAccepted {
    pub id: String,
    pub status: String,
}

/// Max ingestions returned by the history listing.
const HISTORY_LIMIT: i64 = 50;

/// One ingestion for the dashboard's history + progress view (dates/timestamps as ISO strings,
/// currency/country lists split back into arrays).
#[derive(Debug, Serialize)]
pub struct IngestionDto {
    pub id: String,
    pub connector: String,
    pub account: String,
    pub source: String,
    pub status: String,
    pub staged_rows: i64,
    pub report_date: Option<String>,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub currency_count: i32,
    pub currencies: Vec<String>,
    pub country_count: i32,
    pub countries: Vec<String>,
    pub total_gross: f64,
    pub total_clusters: i64,
    pub good_clusters: i64,
    pub last_error: Option<String>,
    pub created_at: String,
}

impl From<CostIngestion> for IngestionDto {
    fn from(r: CostIngestion) -> Self {
        Self {
            id: r.id,
            connector: r.connector,
            account: r.account,
            source: r.source,
            status: r.status,
            staged_rows: r.staged_rows,
            report_date: r.report_date.map(date_str),
            period_start: r.period_start.map(date_str),
            period_end: r.period_end.map(date_str),
            currency_count: r.currency_count,
            currencies: split_list(r.currencies),
            country_count: r.country_count,
            countries: split_list(r.countries),
            total_gross: r.total_gross,
            total_clusters: r.total_clusters,
            good_clusters: r.good_clusters,
            last_error: r.last_error,
            created_at: datetime_str(r.created_at),
        }
    }
}

/// `GET /merchant-account/:id/cost-ingestions` — recent ingestions (history + in-flight), newest
/// first. The dashboard polls this to show history and live progress of processing jobs.
pub async fn list_ingestions(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<IngestionDto>>, (StatusCode, String)> {
    let rows = store::list_for_merchant(&merchant_id, HISTORY_LIMIT)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    Ok(Json(rows.into_iter().map(IngestionDto::from).collect()))
}

/// `DELETE /merchant-account/:id/cost-ingestions/:ingestion_id` — undo an ingestion. Removes the
/// transactions it contributed (by `ingestion_id`), **refits** the affected `(connector, account)`
/// so the served model reflects what remains, and deletes the history row. In-progress jobs can't
/// be deleted.
pub async fn delete_ingestion(
    Path((merchant_id, ingestion_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let row = store::get_for_merchant(&merchant_id, &ingestion_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?
        .ok_or((StatusCode::NOT_FOUND, "ingestion not found".to_string()))?;

    if row.status == "processing" {
        return Err((
            StatusCode::CONFLICT,
            "cannot delete an in-progress ingestion".to_string(),
        ));
    }

    let clickhouse = crate::app::APP_STATE
        .get()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "app state not initialized".to_string(),
        ))?
        .global_config
        .analytics
        .clickhouse
        .clone();

    // Remove this ingestion's staged transactions, then refit from what remains so the served model
    // and coverage reflect the deletion. The fit stamps today as the new snapshot.
    sink::delete_ingestion_rows(
        &clickhouse,
        &row.connector,
        &row.account,
        &merchant_id,
        &ingestion_id,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;

    let report_date = crate::utils::date_time::now().date().to_string();
    if let Err(e) = crate::cost_ingestion::fit::fit_snapshot(
        &clickhouse,
        &row.connector,
        &row.account,
        &merchant_id,
        &report_date,
    )
    .await
    {
        logger::warn!(tag = "report_upload", "refit after delete failed: {:?}", e);
    }

    store::delete(&merchant_id, &ingestion_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;

    // Revert the served models immediately rather than waiting for the periodic refresh. Runs
    // unconditionally — including when the refit above was empty (last data deleted), in which case
    // the fit has purged the stale snapshot and this rebuilds this merchant's cache entry without it
    // (dropping it), so the router stops using models with no supporting data right away instead of
    // after the 300s tick. Per-merchant: other merchants' cached models are left untouched.
    if let Err(e) =
        crate::cost_ingestion::serving::refresh_merchant(&clickhouse, &merchant_id).await
    {
        logger::warn!(
            tag = "report_upload",
            "serving refresh after delete failed: {}",
            e
        );
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `GET /merchant-account/:id/cost-price-changes` — fee-regime changes detected by diffing each
/// cluster's two most recent fits. Surfaces when a connector's pricing moved and which part (% or
/// flat fee) changed.
pub async fn list_price_changes(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<crate::cost_ingestion::detect::PriceChange>>, (StatusCode, String)> {
    let clickhouse = crate::app::APP_STATE
        .get()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "app state not initialized".to_string(),
        ))?
        .global_config
        .analytics
        .clickhouse
        .clone();
    let changes = crate::cost_ingestion::detect::price_changes(&clickhouse, &merchant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    Ok(Json(changes))
}

fn split_list(s: Option<String>) -> Vec<String> {
    s.map(|v| {
        v.split(',')
            .filter(|x| !x.is_empty())
            .map(String::from)
            .collect()
    })
    .unwrap_or_default()
}

fn date_str(d: time::Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
}

fn datetime_str(dt: time::PrimitiveDateTime) -> String {
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        date_str(dt.date()),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

fn temp_report_path() -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "de-settlement-report-{}-{}.csv",
        std::process::id(),
        seq
    ))
}

pub async fn upload_report(
    Path((merchant_id, connector)): Path<(String, String)>,
    Query(params): Query<UploadParams>,
    body: Body, // must be the last extractor (consumes the request body)
) -> Result<(StatusCode, Json<UploadAccepted>), (StatusCode, String)> {
    let path = temp_report_path();

    // Stream the body to the temp file, capping total size. On any failure here we remove the
    // partial file and bail — the background task only takes over once the file is complete.
    if let Err(err) = stream_to_file(&path, body).await {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(err);
    }

    // ClickHouse config lives on the global config (not the per-tenant config).
    let clickhouse = match crate::app::APP_STATE.get() {
        Some(state) => state.global_config.analytics.clickhouse.clone(),
        None => {
            let _ = tokio::fs::remove_file(&path).await;
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "app state not initialized".to_string(),
            ));
        }
    };

    // Create the ingestion row (status=processing) up front so the dashboard can poll it.
    let path_str = path.to_string_lossy().to_string();
    let job_id = store::create_manual(&merchant_id, &connector, &params.account, &path_str)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not create ingestion job: {e:?}"),
            )
        })?;

    // Process in the background: the request returns now; the merchant watches progress via polling.
    tokio::spawn(process_upload(
        job_id.clone(),
        path,
        clickhouse,
        connector,
        params.account,
        merchant_id,
    ));

    Ok((
        StatusCode::ACCEPTED,
        Json(UploadAccepted {
            id: job_id,
            status: "processing".to_string(),
        }),
    ))
}

/// `GET /cost-ingestion/connectors` — every connector whose settlement report can be ingested.
///
/// Read straight off the [`ConnectorRegistry`](crate::cost_ingestion::ConnectorRegistry), which is
/// the same registry the parsers, the preflight and the mapper resolve through. The dashboard's
/// connector picker is driven from this rather than its own copy of the list, so registering a
/// connector is one line in `with_builtins` and it appears in the UI — a hardcoded frontend list
/// would silently make new connectors unmappable, which is precisely the coupling the registry
/// exists to avoid.
pub async fn list_ingest_connectors() -> Json<Vec<ConnectorInfo>> {
    let registry = crate::cost_ingestion::source::ConnectorRegistry::with_builtins();
    Json(
        registry
            .connectors()
            .into_iter()
            .map(|id| ConnectorInfo {
                id: id.to_string(),
                // Whether reports arrive by polling the connector's API rather than a pushed
                // webhook. Surfaced so the dashboard can explain how a source gets its reports
                // without keeping its own per-connector table of that fact.
                pull: crate::cost_ingestion::creds::is_pull_connector(id),
            })
            .collect(),
    )
}

/// A connector that supports settlement-report ingestion.
#[derive(Debug, Serialize)]
pub struct ConnectorInfo {
    pub id: String,
    pub pull: bool,
}

/// `POST /merchant-account/:id/connectors/:connector/report/validate-headers` — check a report's
/// header row against `connector` **before** the merchant uploads the file.
///
/// The body is just the leading bytes of their file (the dashboard sends
/// `file.slice(0, HEADER_SAMPLE_BYTES)`), so this is a few-KB request answered synchronously in
/// milliseconds. Without it the only way to learn a file's headers are wrong is to upload all of it,
/// wait for the background parse, and read `last_error` off the history row — once per missing
/// column, since resolution used to stop at the first.
///
/// A file that cannot be parsed is **200 with `ok: false`**, not an error status: "your header is
/// missing these columns" is a successful answer to the question asked. Only an unknown connector
/// (404) or an unreadable sample (400) is an error.
pub async fn validate_report_headers(
    Path((merchant_id, connector)): Path<(String, String)>,
    Query(params): Query<PreflightParams>,
    body: Body,
) -> Result<Json<crate::cost_ingestion::preflight::PreflightReport>, (StatusCode, String)> {
    use crate::cost_ingestion::preflight;

    let sample = read_capped(body, preflight::HEADER_SAMPLE_BYTES).await?;
    if sample.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty header sample".to_string()));
    }

    // Validate against the source's saved mapping when there is an account to look one up for, so a
    // merchant who mapped this source last month sees their next file pass immediately.
    let mapping = match params.account.as_deref() {
        Some(account) => mapping::load(&merchant_id, &connector, account)
            .await
            .map_err(|e| storage_error("load column mapping", e))?,
        None => Default::default(),
    };

    let registry = crate::cost_ingestion::source::ConnectorRegistry::with_builtins();
    preflight::check(&registry, &connector, &sample, &mapping)
        .map(Json)
        .map_err(map_preflight_error)
}

#[derive(Debug, Deserialize)]
pub struct PreflightParams {
    /// Optional: when given, the source's saved column mapping is applied to the check.
    pub account: Option<String>,
}

/// Body of a mapping preview/save: the candidate mapping plus the file sample it was written
/// against. The sample travels with the request because a mapping is only meaningful relative to a
/// specific file's headers, and the server holds no copy of the merchant's file at this stage.
#[derive(Debug, Deserialize)]
pub struct MappingRequest {
    /// `expected label -> merchant's label`.
    pub columns: std::collections::HashMap<String, String>,
    /// The leading bytes of the merchant's report, as text.
    #[serde(default)]
    pub sample: String,
    /// Whether `sample` is the head of a larger file rather than the whole of a small one. Stated by
    /// the caller (which knows the file's real size) rather than inferred from the sample's length,
    /// which text decoding can shift off the cap. Defaults to `false`: treating a partial sample as
    /// whole merely reinstates the old behaviour, whereas treating a whole file as partial would
    /// silently discard genuinely fee-free rows.
    #[serde(default)]
    pub truncated: bool,
}

/// `POST /merchant-account/:id/connectors/:connector/report/preview` — parse the first rows of a
/// sample under a **candidate** mapping and return what they become.
///
/// Takes no `account`: the mapping being previewed is in the request body, not yet stored against a
/// settlement source, so there is nothing to look one up for.
///
/// This is the safety net for the mapping UI, and the reason mapping is safe to offer at all.
/// Well-formedness is checkable (`ColumnMapping::validate`) but *correctness* is not: pointing
/// `Commission (SC)` at an all-in fee column validates fine, fits fine, grades `GOOD`, and serves a
/// wrong price to the router. Returning the derived `gross` / `total_fee` / effective rate is what
/// lets a human catch that, so the dashboard renders this before it will save.
pub async fn preview_column_mapping(
    Path((_merchant_id, connector)): Path<(String, String)>,
    Json(req): Json<MappingRequest>,
) -> Result<Json<crate::cost_ingestion::preflight::PreviewReport>, (StatusCode, String)> {
    use crate::cost_ingestion::preflight;

    let registry = crate::cost_ingestion::source::ConnectorRegistry::with_builtins();
    let candidate = mapping::ColumnMapping::from_pairs(req.columns);
    preflight::preview(
        &registry,
        &connector,
        req.sample.as_bytes(),
        &candidate,
        req.truncated,
    )
    .map(Json)
    .map_err(map_preflight_error)
}

/// `GET /merchant-account/:id/connectors/:connector/report/column-mapping?account=` — the saved
/// mapping for a settlement source, or an empty one.
pub async fn get_column_mapping(
    Path((merchant_id, connector)): Path<(String, String)>,
    Query(params): Query<UploadParams>,
) -> Result<Json<mapping::ColumnMapping>, (StatusCode, String)> {
    mapping::load(&merchant_id, &connector, &params.account)
        .await
        .map(Json)
        .map_err(|e| storage_error("load column mapping", e))
}

/// `PUT /merchant-account/:id/connectors/:connector/report/column-mapping?account=` — save a
/// mapping so every future upload, webhook, and poll for this source applies it automatically.
/// Persistence is most of the value: it turns a recurring monthly chore into a one-time one.
///
/// Rejects a mapping that is not well-formed against the connector's schema and the file's headers
/// (unknown column, absent target, two columns sharing a source). It deliberately does **not**
/// reject an implausible one — that judgement belongs to the merchant looking at
/// [`preview_column_mapping`], since an unusual-but-real report is indistinguishable from a wrong
/// mapping from here.
pub async fn set_column_mapping(
    Path((merchant_id, connector)): Path<(String, String)>,
    Query(params): Query<UploadParams>,
    Json(req): Json<MappingRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    use crate::cost_ingestion::preflight;

    let candidate = mapping::ColumnMapping::from_pairs(req.columns);

    // Well-formedness is checked against the connector's own schema and the sample's real headers,
    // both discovered from the connector rather than hardcoded here.
    if !candidate.is_empty() {
        if req.sample.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "a file sample is required to validate a mapping".to_string(),
            ));
        }
        let registry = crate::cost_ingestion::source::ConnectorRegistry::with_builtins();
        let report = preflight::check(
            &registry,
            &connector,
            req.sample.as_bytes(),
            mapping::ColumnMapping::none(),
        )
        .map_err(map_preflight_error)?;
        let known: Vec<String> = report
            .required
            .iter()
            .chain(report.optional.iter())
            .cloned()
            .collect();
        candidate
            .validate(&known, &report.found)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("{e}")))?;
    }

    mapping::save(&merchant_id, &connector, &params.account, &candidate)
        .await
        .map_err(|e| storage_error("save column mapping", e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /merchant-account/:id/connectors/:connector/report/column-mapping?account=` — clear a
/// source's mapping so its reports parse with the connector's own labels again.
pub async fn delete_column_mapping(
    Path((merchant_id, connector)): Path<(String, String)>,
    Query(params): Query<UploadParams>,
) -> Result<StatusCode, (StatusCode, String)> {
    mapping::delete(&merchant_id, &connector, &params.account)
        .await
        .map_err(|e| storage_error("clear column mapping", e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Turn a storage failure into a 500 that says what failed without handing the caller our internals.
///
/// The `Debug` form of an `IngestError` can carry driver text, connection strings and query
/// fragments; useful in a log, not something to return to a dashboard client. The detail is logged
/// server-side under `op` so it stays diagnosable, and the response body is a stable phrase.
///
/// Deliberately not applied to the 4xx paths: a rejected column mapping ("'Payable (SC)' is mapped
/// to 'X', which is not a column in this file") is the merchant's own input described back to them,
/// and it is the entire value of that response.
fn storage_error(op: &str, e: crate::cost_ingestion::IngestError) -> (StatusCode, String) {
    logger::error!(tag = "report_upload", "{} failed: {:?}", op, e);
    (StatusCode::INTERNAL_SERVER_ERROR, format!("could not {op}"))
}

fn map_preflight_error(e: crate::cost_ingestion::IngestError) -> (StatusCode, String) {
    match e {
        crate::cost_ingestion::IngestError::UnknownConnector(c) => (
            StatusCode::NOT_FOUND,
            format!("connector {c} does not support report ingestion"),
        ),
        other => (StatusCode::BAD_REQUEST, format!("{other}")),
    }
}

/// Read at most `cap` bytes of a body into memory, stopping early once the cap is reached. Used only
/// for the header sample — the report upload path itself never buffers a body.
async fn read_capped(body: Body, cap: usize) -> Result<Vec<u8>, (StatusCode, String)> {
    let mut buf = Vec::new();
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| (StatusCode::BAD_REQUEST, format!("read error: {e}")))?;
        let take = cap.saturating_sub(buf.len()).min(chunk.len());
        buf.extend_from_slice(&chunk[..take]);
        if buf.len() >= cap {
            break;
        }
    }
    Ok(buf)
}

/// `POST /merchant-account/:id/connectors/:connector/report/sample` — run a curated **sample**
/// report for `connector` through the normal pipeline, so a merchant without a report file of their
/// own can still exercise the ingest → fit → coverage flow. The sample CSV is fetched from S3 at
/// `<sample_bucket>/<connector>_report.csv` and filed under `acc_<connector>`. If the connector is
/// not one that supports manual/report ingestion, or no `sample_bucket` is configured, the endpoint
/// returns 404. Like the manual upload, it returns the job id immediately (202) and processes in
/// the background — the dashboard polls the same history/progress.
pub async fn run_sample_report(
    Path((merchant_id, connector)): Path<(String, String)>,
) -> Result<(StatusCode, Json<UploadAccepted>), (StatusCode, String)> {
    // Only connectors that support report ingestion can have samples.
    let registry = crate::cost_ingestion::source::ConnectorRegistry::with_builtins();
    let _ = registry.get(&connector).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            format!("connector {connector} does not support sample reports"),
        )
    })?;

    // Sample config + ClickHouse both live on the global config.
    let state = crate::app::APP_STATE.get().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "app state not initialized".to_string(),
    ))?;
    let cost_cfg = &state.global_config.cost_ingestion;
    let bucket = Some(cost_cfg.aws_bucket.clone())
        .filter(|b| !b.is_empty())
        .ok_or((
            StatusCode::NOT_FOUND,
            "no sample bucket configured".to_string(),
        ))?;
    let region = cost_cfg.aws_region.clone();

    let account = format!("acc_{connector}");
    let key = format!("{connector}_report.csv");
    let report_ref = format!("s3://{bucket}/{key}");
    let clickhouse = state.global_config.analytics.clickhouse.clone();

    // Create the ingestion row (status=processing) up front so the dashboard can poll it. The row
    // records the S3 location as its ref, mirroring how a manual upload records its temp path.
    let job_id = store::create_sample(&merchant_id, &connector, &account, &report_ref)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("could not create ingestion job: {e:?}"),
            )
        })?;

    let path = temp_report_path();
    tokio::spawn(process_sample(
        job_id.clone(),
        path,
        clickhouse,
        connector,
        account,
        merchant_id,
        bucket,
        key,
        region,
    ));

    Ok((
        StatusCode::ACCEPTED,
        Json(UploadAccepted {
            id: job_id,
            status: "processing".to_string(),
        }),
    ))
}

/// Background task for a sample run: download the sample CSV from S3 to a temp file, run it through
/// the same parse → stage → fit pipeline as an upload, and record the outcome. Progress ticks
/// against `job_id` as batches stage.
async fn process_sample(
    job_id: String,
    path: PathBuf,
    clickhouse: crate::config::ClickHouseAnalyticsConfig,
    connector: String,
    account: String,
    merchant_id: String,
    bucket: String,
    key: String,
    region: Option<String>,
) {
    let download_result = fetch_sample_from_s3(&bucket, &key, region.as_deref(), &path).await;
    let result = match download_result {
        Ok(()) => {
            run_ingest(
                &job_id,
                &path,
                &clickhouse,
                &connector,
                &account,
                &merchant_id,
            )
            .await
        }
        Err(e) => Err(e),
    };

    finish_ingest(&job_id, result, &clickhouse, &merchant_id).await;

    // Always remove the temp file, success or failure. Ignore "not found" — that just means the
    // download failed before the file was created.
    if let Err(e) = tokio::fs::remove_file(&path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            logger::warn!(
                tag = "report_upload",
                "sample temp cleanup {:?} failed: {}",
                path,
                e
            );
        }
    }
}

/// Download a sample report from S3 into `path`. Streams chunks so multi-GB samples don't need to
/// be held in memory. Credentials come from the default AWS credential chain in the runtime
/// environment (IAM role, env vars, profile, etc.).
async fn fetch_sample_from_s3(
    bucket: &str,
    key: &str,
    region: Option<&str>,
    path: &PathBuf,
) -> Result<(), crate::cost_ingestion::IngestError> {
    use crate::cost_ingestion::IngestError;
    use aws_config::BehaviorVersion;

    let mut loader = aws_config::defaults(BehaviorVersion::latest());
    if let Some(region) = region {
        loader = loader.region(aws_config::Region::new(region.to_string()));
    }
    let sdk_config = loader.load().await;
    let client = aws_sdk_s3::Client::new(&sdk_config);

    let mut output = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| {
            IngestError::Download(format!("s3 get_object s3://{bucket}/{key} failed: {e}"))
        })?;

    let mut file = tokio::fs::File::create(path).await.map_err(|e| {
        IngestError::Download(format!("could not open temp file {path:?} for sample: {e}"))
    })?;

    let mut total: usize = 0;
    while let Some(chunk) = output.body.next().await {
        let chunk =
            chunk.map_err(|e| IngestError::Download(format!("s3 body stream error: {e}")))?;
        total = total.saturating_add(chunk.len());
        if total > MAX_UPLOAD_BYTES {
            return Err(IngestError::Download(format!(
                "sample exceeds {MAX_UPLOAD_BYTES} byte limit"
            )));
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| IngestError::Download(format!("temp file write failed: {e}")))?;
    }
    file.flush()
        .await
        .map_err(|e| IngestError::Download(format!("temp flush failed: {e}")))?;

    if total == 0 {
        return Err(IngestError::Download("empty sample object".to_string()));
    }
    Ok(())
}

/// Stream a request body to `path`, enforcing `MAX_UPLOAD_BYTES`. Never holds the whole body in RAM.
async fn stream_to_file(path: &PathBuf, body: Body) -> Result<(), (StatusCode, String)> {
    let mut file = tokio::fs::File::create(path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not open temp file: {e}"),
        )
    })?;

    let mut total: usize = 0;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| (StatusCode::BAD_REQUEST, format!("upload read error: {e}")))?;
        total = total.saturating_add(chunk.len());
        if total > MAX_UPLOAD_BYTES {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("report exceeds {MAX_UPLOAD_BYTES} byte limit"),
            ));
        }
        file.write_all(&chunk).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("temp write failed: {e}"),
            )
        })?;
    }
    file.flush().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("temp flush failed: {e}"),
        )
    })?;

    if total == 0 {
        return Err((StatusCode::BAD_REQUEST, "empty report body".to_string()));
    }
    Ok(())
}

/// Background task: run the same parse → stage → fit pipeline the worker uses, record the outcome,
/// and clean up the temp file. Progress ticks against `job_id` as batches stage.
async fn process_upload(
    job_id: String,
    path: PathBuf,
    clickhouse: crate::config::ClickHouseAnalyticsConfig,
    connector: String,
    account: String,
    merchant_id: String,
) {
    let result = run_ingest(
        &job_id,
        &path,
        &clickhouse,
        &connector,
        &account,
        &merchant_id,
    )
    .await;

    finish_ingest(&job_id, result, &clickhouse, &merchant_id).await;

    // Always remove the temp file, success or failure.
    if let Err(e) = tokio::fs::remove_file(&path).await {
        logger::warn!(
            tag = "report_upload",
            "temp cleanup {:?} failed: {}",
            path,
            e
        );
    }
}

/// Record the outcome of an ingest run: on success mark the job completed and refresh this
/// merchant's served models immediately (so a just-ingested cluster doesn't fall back for ~5 min);
/// on failure mark it failed with the error. Shared by the manual upload and sample flows.
async fn finish_ingest(
    job_id: &str,
    result: Result<pipeline::IngestOutcome, crate::cost_ingestion::IngestError>,
    clickhouse: &crate::config::ClickHouseAnalyticsConfig,
    merchant_id: &str,
) {
    match result {
        Ok(outcome) => {
            if let Err(e) = store::mark_completed(job_id, &outcome.to_completion()).await {
                logger::warn!(
                    tag = "report_upload",
                    "mark_completed {} failed: {:?}",
                    job_id,
                    e
                );
            }
            // Serve the freshly-fitted models immediately, rather than waiting for the periodic
            // serving refresh (otherwise a just-ingested cluster keeps falling back for ~5 min).
            // Per-merchant: only this merchant's models are rebuilt, keeping the upload off the
            // ~2s global-refresh path (the periodic ticker still does the full rebuild).
            if let Err(e) =
                crate::cost_ingestion::serving::refresh_merchant(clickhouse, merchant_id).await
            {
                logger::warn!(
                    tag = "report_upload",
                    "serving refresh after ingest failed: {}",
                    e
                );
            }
        }
        Err(e) => {
            let msg = format!("{e:?}");
            logger::warn!(tag = "report_upload", "ingest {} failed: {}", job_id, msg);
            if let Err(e2) = store::mark_failed(job_id, &msg).await {
                logger::warn!(
                    tag = "report_upload",
                    "mark_failed {} failed: {:?}",
                    job_id,
                    e2
                );
            }
        }
    }
}

/// Open the staged file as a blocking reader and run the ingest pipeline.
async fn run_ingest(
    job_id: &str,
    path: &PathBuf,
    clickhouse: &crate::config::ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
) -> Result<pipeline::IngestOutcome, crate::cost_ingestion::IngestError> {
    use crate::cost_ingestion::IngestError;
    let std_file = std::fs::File::open(path)
        .map_err(|e| IngestError::Storage(format!("could not reopen temp file: {e}")))?;
    let reader = Box::new(std::io::BufReader::new(std_file));

    pipeline::ingest_report_reader(
        clickhouse,
        connector,
        account,
        merchant_id,
        reader,
        Some(job_id),
    )
    .await
}
