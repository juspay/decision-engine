//! Chase (J.P. Morgan Payments) `SettlementReportSource`.
//!
//! Port of J.P. Morgan's **Deposit Details** preset report onto the canonical [`SettledFeeRow`].
//! Deposit Details is the only preset report that carries per-transaction *split* interchange and
//! assessment fees together with every cluster dimension the fit keys on (payment method, card
//! usage type, issuer country, settlement currency, interchange qualification code, digital wallet),
//! so it is the one report we ingest for Chase. Everything Chase-specific — the report envelope, the
//! column labels, the fee decomposition, the fee-sign convention — is contained in this file.
//!
//! Fee model. Deposit Details is a **pass-through** report: it itemizes the interchange and
//! assessment (scheme) fees but not Chase's own processor discount/commission (that lives in the
//! Funded Transaction Fee Details report). So, mirroring Braintree's single-report shape:
//!   * `interchange` ← `Total Interchange Amount`
//!   * `scheme_fee`  ← `Total Assessment Amount`
//!   * `markup`      ← `Other Debit Passthrough Fees` (debit pass-through; 0 for credit)
//!   * `commission`  ← 0.0 (Chase's discount rate is not in this report)
//!
//! and `total_fee = interchange + scheme_fee + markup + commission`. The served cost therefore
//! excludes Chase's discount rate — a roughly constant offset that does not affect PSP *ranking*.
//! Backfilling `commission` from Funded Transaction Fee Details (joined on `Merchant Order Number`)
//! is a deliberate follow-up.
//!
//! Fee sign. Chase reports settled fees as **negative** amounts (money deducted from the merchant);
//! the OLS fit regresses a *positive* `total_fee` on a positive `gross`, so every fee column is
//! negated here to a positive cost magnitude. `gross = Transaction Amount in Presentment Currency`
//! is already positive for sales (refunds are filtered out before any fee extraction).
//!
//! Report envelope. A preset report is framed: a UTF-8 BOM, a `BEGIN…` line, an
//! `EntityId=…,ReportTypeName=…` metadata line, then the CSV header + data rows, then an `END…`
//! line. [`EnvelopeReader`] strips that frame line-by-line so [`csv_reader::parse`] sees a plain CSV
//! and memory stays flat for large reports.
//!
//! Notification/download is NOT yet implemented — Chase's report-ready webhook shape, signature
//! scheme, and download transport (SFTP vs. API) still need to be confirmed. Those three methods
//! return a descriptive error until then; the parser (`parse_rows`) is complete and independently
//! testable.

use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use chrono::NaiveDate;
use josekit::jws::JwsHeader;
use josekit::jwt::{self, JwtPayload};
use masking::{PeekInterface, Secret};
use serde::Deserialize;
use uuid::Uuid;

use crate::cost_ingestion::connectors::csv_reader;
use crate::cost_ingestion::source::SettlementReportSource;
use crate::cost_ingestion::types::{
    ConnectorCreds, IngestError, ReadyReport, ReportNotification, SettledFeeRow,
};

/// `Action Type Code Text` value that carries a settled processing fee. Only sales feed the fit;
/// `REFUND` rows are reversals whose signed amounts would distort the gross→fee regression, so they
/// are skipped (mirrors Adyen's record-type and Braintree's transaction-type filter).
const SALE_ACTION: &str = "SALE";

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// The report *type* the J.P. Morgan **Deposit details** preset report is delivered under. Deposit
/// details carries the per-transaction split interchange/assessment fees plus every cluster
/// dimension the fit keys on; its report type is `Submission details`. A merchant configured for
/// cost ingestion runs only Deposit details, so the poller keeps every completed `Submission
/// details` report. Compared with whitespace stripped and case-insensitively, because the reporting
/// API spells it `"Submission details"` while the report envelope spells it `"SubmissionDetails"`.
const SUBMISSION_DETAILS_REPORT_TYPE: &str = "submissiondetails";

/// OAuth2 client-assertion type for the signed-JWT `client_credentials` grant.
const CLIENT_ASSERTION_TYPE: &str = "urn:ietf:params:oauth:client-assertion-type:jwt-bearer";
/// The signed JWT assertion is short-lived — it is exchanged for the access token immediately.
const ASSERTION_TTL: Duration = Duration::from_secs(300);
/// Refresh the cached access token this far before its stated expiry, to avoid racing a 401.
const TOKEN_REFRESH_SKEW: u64 = 300;
/// Cap on pages followed when listing reports — a backstop against a runaway `next` cursor.
const MAX_REPORT_PAGES: usize = 50;

fn default_token_url() -> String {
    "https://idag2.jpmorganchase.com/adfs/oauth2/token".to_string()
}
fn default_reports_url() -> String {
    // Reporting API host (distinct from the report-files host `api.reports-files.jpmorgan.com`).
    // Override for the mock (`api-mock.payments.jpmorgan.com`) or a local stub when testing.
    "https://api.reports.jpmorgan.com/api/v1/reports".to_string()
}

/// Whether a `reportTypeNames` entry is the Submission details type, tolerant of the API's spacing
/// (`"Submission details"`) vs the envelope's (`"SubmissionDetails"`).
fn is_submission_details(name: &str) -> bool {
    name.split_whitespace()
        .collect::<String>()
        .eq_ignore_ascii_case(SUBMISSION_DETAILS_REPORT_TYPE)
}

pub struct ChaseReportSource;

impl ChaseReportSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChaseReportSource {
    fn default() -> Self {
        Self::new()
    }
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .expect("failed to build chase report reqwest client")
    })
}

/// Chase's OAuth + reporting-API access, carried inside the opaque [`ConnectorCreds::download_auth`]
/// as a JSON blob (Chase has no webhook, so `webhook_secret` is unused). `token_url`/`reports_url`
/// default to production but are overridable — point them at `api-mock.payments.jpmorgan.com` to
/// run against J.P. Morgan's mock, which accepts any bearer token.
#[derive(Debug, Clone, Deserialize)]
struct ChaseCreds {
    /// OAuth client id issued by J.P. Morgan. Required for real auth; unused when `access_token` is
    /// set.
    #[serde(default)]
    client_id: String,
    /// OAuth `resource` parameter issued by J.P. Morgan. Unused when `access_token` is set.
    #[serde(default)]
    resource: String,
    /// RSA private key (PEM) that signs the JWT client assertion. Unused when `access_token` is set.
    #[serde(default)]
    private_key_pem: String,
    /// Local/mock testing escape hatch: a pre-supplied bearer token. When set, OAuth signing is
    /// skipped and this token is sent directly — J.P. Morgan's mock and a local stub ignore the
    /// token's validity, so no `client_id`/`resource`/`private_key_pem` is needed. Leave unset for
    /// real J.P. Morgan auth.
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default = "default_token_url")]
    token_url: String,
    #[serde(default = "default_reports_url")]
    reports_url: String,
}

impl ChaseCreds {
    /// Read the JSON credential blob out of `download_auth`.
    fn parse(creds: &ConnectorCreds) -> Result<Self, IngestError> {
        serde_json::from_str(creds.download_auth.peek()).map_err(|e| {
            IngestError::MalformedNotification(format!(
                "chase download_auth must be a JSON credential blob: {e}"
            ))
        })
    }
}

// ── JSON shapes for the reporting API (only the fields we read; camelCase → snake_case). ──

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportsListResponse {
    #[serde(default)]
    summarized_reports: Vec<SummarizedReport>,
    /// Opaque cursor for the next page; echoed back as the `next` request header. Absent on last page.
    next: Option<String>,
    /// True when this is the final page of results.
    #[serde(default)]
    last_page: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SummarizedReport {
    #[serde(default)]
    report_identifier: String,
    #[serde(default)]
    report_type_names: Vec<String>,
    #[serde(default)]
    report_status: String,
    interval_param: Option<IntervalParam>,
    report_details: Option<ReportDetails>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntervalParam {
    reporting_period_start_timestamp: Option<String>,
    reporting_period_end_timestamp: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportDetails {
    url: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: u64,
}

#[async_trait]
impl SettlementReportSource for ChaseReportSource {
    fn connector(&self) -> &'static str {
        "chase"
    }

    fn peek_account(&self, _raw_body: &[u8]) -> Result<String, IngestError> {
        // Chase is pull-based (no report-ready webhook), so the webhook ingress never routes here —
        // discovery happens in the report poller via `poll_ready_reports`.
        Err(not_implemented("peek_account"))
    }

    fn verify_and_parse_notification(
        &self,
        _headers: &HeaderMap,
        _raw_body: &[u8],
        _secret: &Secret<String>,
    ) -> Result<ReportNotification, IngestError> {
        // Pull-based: no inbound webhook to verify. See `chase_poller`.
        Err(not_implemented("verify_and_parse_notification"))
    }

    /// Chase is pull-based: the report poller lists ready reports (below) rather than receiving a
    /// webhook.
    fn is_pull(&self) -> bool {
        true
    }

    /// List the completed Deposit details (Submission details) reports ready to ingest for one
    /// settlement source. Driven by the generic report poller.
    async fn poll_ready_reports(
        &self,
        creds: &ConnectorCreds,
    ) -> Result<Vec<ReadyReport>, IngestError> {
        let chase = ChaseCreds::parse(creds)?;
        list_ready_reports(&chase).await
    }

    /// Fetch a completed report's file. `note.report_ref` is the `reportDetails.url` captured at
    /// poll time (on J.P. Morgan's file host, distinct from the reports API), fetched with a fresh
    /// bearer token.
    async fn download_report(
        &self,
        creds: &ConnectorCreds,
        note: &ReportNotification,
    ) -> Result<Bytes, IngestError> {
        let chase = ChaseCreds::parse(creds)?;
        let token = oauth_token(&chase).await?;
        let resp = http_client()
            .get(&note.report_ref)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| IngestError::Download(format!("chase report download: {e}")))?;
        if !resp.status().is_success() {
            return Err(IngestError::Download(format!(
                "chase report download status {}",
                resp.status()
            )));
        }
        resp.bytes()
            .await
            .map_err(|e| IngestError::Download(e.to_string()))
    }

    fn unwrap_envelope(&self, reader: Box<dyn Read + Send>) -> Box<dyn Read + Send> {
        Box::new(EnvelopeReader::new(reader))
    }

    fn parse_rows(
        &self,
        reader: Box<dyn Read + Send>,
        mapping: &crate::cost_ingestion::mapping::ColumnMapping,
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        // Resolved column indices for one report. Preset-report column order can drift between
        // report versions, so never index positionally — resolve every column by its exact label.
        struct Cols {
            order: usize,
            action: usize,
            method: usize,
            usage: usize,
            issuer: usize,
            currency: usize,
            ic_code: usize,
            amount: usize,
            interchange: usize,
            assessment: usize,
            // Optional columns — absent in some report profiles. Missing ⇒ blank/0.
            passthrough: Option<usize>,
            wallet: Option<usize>,
            submission_date: Option<usize>,
            // A terminal id marks in-person (POS) acceptance; absent ⇒ online (ecom). Drives the
            // channel feature of the interchange-category predictor (as Adyen's terminal id does).
            terminal: Option<usize>,
        }

        // Strip the preset-report BEGIN/metadata/END envelope so the shared CSV driver sees a plain
        // header + data stream.
        let reader = self.unwrap_envelope(reader);

        csv_reader::parse(
            reader,
            mapping,
            |h| {
                Ok(Cols {
                    order: h.require("Merchant Order Number")?,
                    action: h.require("Action Type Code Text")?,
                    method: h.require("Payment Method Code")?,
                    usage: h.require("Card Usage Type")?,
                    issuer: h.require("Country of Issuance")?,
                    currency: h.require("Settlement Currency Code")?,
                    ic_code: h.require("Interchange Qualification Code")?,
                    amount: h.require("Transaction Amount in Presentment Currency")?,
                    interchange: h.require("Total Interchange Amount")?,
                    assessment: h.require("Total Assessment Amount")?,
                    passthrough: h.index("Other Debit Passthrough Fees"),
                    wallet: h.index("Digital Wallet Brand"),
                    submission_date: h.index("Submission Date"),
                    terminal: h.index("Terminal Identifier"),
                })
            },
            |c, row| {
                // Keep only settled sales; skip refunds (and any stray envelope remnant, whose
                // action field is out of range ⇒ "" ⇒ not a sale) before any field extraction.
                if !row.get(c.action).trim().eq_ignore_ascii_case(SALE_ACTION) {
                    return Ok(None);
                }

                // Chase reports settled fees as negative deductions; negate to the positive cost
                // magnitudes the OLS fit expects.
                let interchange = -to_float(row.get(c.interchange));
                let scheme_fee = -to_float(row.get(c.assessment));
                let markup = -to_float(row.get_opt(c.passthrough));
                // Deposit Details carries no processor discount/commission; that fee lives in the
                // Funded Transaction Fee Details report. Left 0 until a join backfills it.
                let commission = 0.0;
                let total_fee = interchange + scheme_fee + markup + commission;
                // `Transaction Amount in Presentment Currency` is the sale value (the fee's
                // calculation base) and is already positive for sales.
                let gross = to_float(row.get(c.amount));

                let network = normalize_network(row.get(c.method));
                // Reconcile funding: the interchange qualification code is authoritative about the
                // fee tier, so it corrects a `Card Usage Type` that some exports stamp uniformly
                // (every row `1`), which would otherwise price credit txns as debit.
                let funding = reconcile_funding(row.get(c.usage), row.get(c.ic_code));
                let wallet = row.get_opt(c.wallet);
                let variant = build_variant(&network, &funding, wallet);
                let txn_date = c.submission_date.and_then(|i| parse_date(row.get(i)));
                // POS when a terminal id is present, else online. Absent column ⇒ unknown ⇒ ecom.
                let channel = match c.terminal {
                    Some(i) if !row.get(i).trim().is_empty() => "pos",
                    _ => "ecom",
                }
                .to_string();

                Ok(Some(SettledFeeRow {
                    txn_ref: row.get(c.order).trim().to_string(),
                    card_network: network,
                    variant,
                    funding,
                    issuer_country: row.get(c.issuer).trim().to_string(),
                    currency: row.get(c.currency).trim().to_string(),
                    ic_category: row.get(c.ic_code).trim().to_string(),
                    txn_date,
                    channel,
                    gross,
                    total_fee,
                    interchange,
                    scheme_fee,
                    markup,
                    commission,
                }))
            },
            on_row,
        )
    }
}

/// The webhook path is unused for Chase (pull-based); these trait methods are never called.
fn not_implemented(method: &str) -> IngestError {
    IngestError::MalformedNotification(format!(
        "chase connector: {method} is unused — chase is pull-based (see the report poller)"
    ))
}

/// `GET /api/v1/reports`, following the `next`/`lastPage` cursor → all completed Submission details
/// instances across pages.
async fn list_ready_reports(creds: &ChaseCreds) -> Result<Vec<ReadyReport>, IngestError> {
    let token = oauth_token(creds).await?;
    let mut out = Vec::new();
    let mut next: Option<String> = None;
    for _ in 0..MAX_REPORT_PAGES {
        let mut req = http_client()
            .get(&creds.reports_url)
            .bearer_auth(&token)
            .header("Accept", "application/json");
        // The cursor is passed as a request header (per the reporting API spec), not a query param.
        if let Some(cursor) = &next {
            req = req.header("next", cursor.as_str());
        }
        let resp = req
            .send()
            .await
            .map_err(|e| IngestError::Download(format!("chase reports request: {e}")))?;
        if !resp.status().is_success() {
            return Err(IngestError::Download(format!(
                "chase reports status {}",
                resp.status()
            )));
        }
        let body = resp
            .bytes()
            .await
            .map_err(|e| IngestError::Download(format!("chase reports body: {e}")))?;
        let page = parse_reports_page(&body)?;
        let last = page.last_page || page.next.is_none();
        next = page.next.clone();
        out.extend(ready_reports(page));
        if last {
            break;
        }
    }
    Ok(out)
}

/// Parse one `GET /reports` page (typed), leaving pagination and filtering to the caller.
fn parse_reports_page(body: &[u8]) -> Result<ReportsListResponse, IngestError> {
    serde_json::from_slice(body)
        .map_err(|e| IngestError::Download(format!("chase reports parse: {e}")))
}

/// Keep only completed Submission details instances that carry a download URL.
fn ready_reports(resp: ReportsListResponse) -> Vec<ReadyReport> {
    resp.summarized_reports
        .into_iter()
        .filter_map(ready_report)
        .collect()
}

/// Buffered convenience over [`parse_reports_page`] + [`ready_reports`] for one page (used by tests).
#[cfg(test)]
fn parse_reports_list(body: &[u8]) -> Result<Vec<ReadyReport>, IngestError> {
    Ok(ready_reports(parse_reports_page(body)?))
}

/// One `summarizedReports` entry → a [`ReadyReport`], or `None` if it isn't a completed Submission
/// details report with a download URL. `reportStatus` is capitalized in the API (`"Completed"`), so
/// compare case-insensitively; `Requested`/`Initiated`/`Errored` reports carry no `reportDetails`
/// and are skipped.
fn ready_report(r: SummarizedReport) -> Option<ReadyReport> {
    if !r.report_status.eq_ignore_ascii_case("completed") {
        return None;
    }
    if !r.report_type_names.iter().any(|n| is_submission_details(n)) {
        return None;
    }
    let report_ref = r
        .report_details
        .and_then(|d| d.url)
        .filter(|u| !u.trim().is_empty())?;
    let (period_start, period_end) = match r.interval_param {
        Some(p) => (
            p.reporting_period_start_timestamp
                .as_deref()
                .and_then(parse_ts_date),
            p.reporting_period_end_timestamp
                .as_deref()
                .and_then(parse_ts_date),
        ),
        None => (None, None),
    };
    Some(ReadyReport {
        report_id: r.report_identifier,
        report_ref,
        period_start,
        period_end,
    })
}

/// Parse the date out of a JPM timestamp cell (`2021-08-20 00:18:18` → `2021-08-20`).
fn parse_ts_date(s: &str) -> Option<NaiveDate> {
    let date = s.split_whitespace().next()?;
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

// ── OAuth2: signed-JWT `client_credentials` grant, with a per-`client_id` token cache. ──

/// Return a valid access token for `creds`, minting a new one only when the cache is empty/expired.
/// Caching respects J.P. Morgan's token-request rate limit (tokens are valid ~8h).
async fn oauth_token(creds: &ChaseCreds) -> Result<String, IngestError> {
    // Local/mock testing: a pre-supplied token bypasses OAuth signing entirely.
    if let Some(tok) = creds.access_token.as_ref().filter(|t| !t.is_empty()) {
        return Ok(tok.clone());
    }
    if let Some(tok) = cached_token(&creds.client_id) {
        return Ok(tok);
    }
    let assertion = build_client_assertion(creds, SystemTime::now(), &Uuid::new_v4().to_string())?;
    let form = build_token_form(creds, &assertion);
    let resp = http_client()
        .post(&creds.token_url)
        .form(&form)
        .send()
        .await
        .map_err(|e| IngestError::Download(format!("chase token request: {e}")))?;
    if !resp.status().is_success() {
        return Err(IngestError::Download(format!(
            "chase token status {}",
            resp.status()
        )));
    }
    let token: TokenResponse = resp
        .json()
        .await
        .map_err(|e| IngestError::Download(format!("chase token parse: {e}")))?;
    store_token(&creds.client_id, &token);
    Ok(token.access_token)
}

fn token_cache() -> &'static Mutex<HashMap<String, (String, Instant)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (String, Instant)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A still-valid cached token for `client_id`, if any. Never holds the lock across an await.
fn cached_token(client_id: &str) -> Option<String> {
    let cache = token_cache().lock().ok()?;
    cache
        .get(client_id)
        .and_then(|(tok, exp)| (Instant::now() < *exp).then(|| tok.clone()))
}

/// Cache a freshly minted token, refreshing a little before its stated expiry. Falls back to a
/// conservative 1h TTL if the server omits `expires_in`.
fn store_token(client_id: &str, token: &TokenResponse) {
    let ttl = token
        .expires_in
        .checked_sub(TOKEN_REFRESH_SKEW)
        .filter(|s| *s > 0)
        .unwrap_or(3600);
    if let Ok(mut cache) = token_cache().lock() {
        cache.insert(
            client_id.to_string(),
            (
                token.access_token.clone(),
                Instant::now() + Duration::from_secs(ttl),
            ),
        );
    }
}

/// Build the RS256-signed JWT client assertion. `now`/`jti` are parameters so this is deterministic
/// to test; production passes `SystemTime::now()` and a fresh UUID.
fn build_client_assertion(
    creds: &ChaseCreds,
    now: SystemTime,
    jti: &str,
) -> Result<String, IngestError> {
    let signer = josekit::jws::RS256
        .signer_from_pem(creds.private_key_pem.as_bytes())
        .map_err(|e| IngestError::Download(format!("chase jwt signer: {e}")))?;
    let mut header = JwsHeader::new();
    header.set_token_type("JWT");
    let mut payload = JwtPayload::new();
    payload.set_issuer(&creds.client_id);
    payload.set_subject(&creds.client_id);
    payload.set_audience(vec![creds.token_url.clone()]);
    payload.set_issued_at(&now);
    payload.set_expires_at(&(now + ASSERTION_TTL));
    payload.set_jwt_id(jti);
    jwt::encode_with_signer(&payload, &header, &signer)
        .map_err(|e| IngestError::Download(format!("chase jwt encode: {e}")))
}

/// The `application/x-www-form-urlencoded` body of the `client_credentials` token request.
fn build_token_form(creds: &ChaseCreds, assertion: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "client_credentials".to_string()),
        ("client_id", creds.client_id.clone()),
        ("client_assertion_type", CLIENT_ASSERTION_TYPE.to_string()),
        ("client_assertion", assertion.to_string()),
        ("resource", creds.resource.clone()),
    ]
}

/// Map a Chase `Payment Method Code` (MOP) onto the lowercased network ids the rest of the pipeline
/// uses (matching Adyen/Braintree: `visa`, `mc`, `amex`, …). Unknown codes pass through lowercased
/// so they still form their own consistent cluster rather than being dropped.
fn normalize_network(code: &str) -> String {
    match code.trim().to_uppercase().as_str() {
        "" => String::new(),
        "VI" => "visa".to_string(),
        "MC" => "mc".to_string(),
        "AX" => "amex".to_string(),
        "DI" => "discover".to_string(),
        "JC" => "jcb".to_string(),
        "DC" => "diners".to_string(),
        other => other.to_lowercase(),
    }
}

/// Map Chase's `Card Usage Type` onto the canonical funding bucket: `1` = signature debit and
/// `2` = PIN debit both fold to `debit`; `3` = credit; anything else is unknown (`""`).
fn funding_from_usage_type(code: &str) -> String {
    match code.trim() {
        "1" | "2" => "debit".to_string(),
        "3" => "credit".to_string(),
        _ => String::new(),
    }
}

/// Funding implied by a known **interchange qualification code**, or `None` when the code isn't
/// classified. The interchange program *is* the fee tier, so it is authoritative about funding —
/// a credit program is a credit transaction whatever the `Card Usage Type` column says. This is a
/// curated lookup of the Visa/Mastercard/Amex/Discover/JCB programs seen in Chase Deposit Details;
/// extend it as new programs appear. Unlisted codes defer to `Card Usage Type`.
fn funding_from_ic_code(ic: &str) -> Option<&'static str> {
    match ic.trim().to_uppercase().as_str() {
        // ── credit programs ──
        // Visa (US consumer/rewards/commercial + EU-capped + cross-border consumer credit)
        "V148" | "V105" | "VBL2" | "VPDM" | "V987" | "VINT" | "V5FI"
        // Mastercard credit
        | "MCRW" | "MX29" | "MM1" | "MIP" | "MIC" | "MHB2" | "MHB4"
        // Amex (OptBlue), Discover, JCB — credit networks
        | "AXSP" | "DIRW" | "D460" | "D361" | "DDEM" | "DCEC" | "DCED" | "DPEC" => Some("credit"),
        // ── debit programs ──
        // Visa regulated/signature/EU debit, Mastercard EU debit
        "VSDD" | "VCEB" | "V2DB" | "MDIN" => Some("debit"),
        _ => None,
    }
}

/// Resolve funding, preferring the authoritative interchange program over `Card Usage Type`. Some
/// Chase exports stamp every row `Card Usage Type = 1` (signature debit), which would otherwise
/// price credit interchange (e.g. Visa `V148` at ~3.3%) as debit; keying off the interchange code
/// corrects that. Falls back to `Card Usage Type` for any code we don't classify.
fn reconcile_funding(usage_code: &str, ic: &str) -> String {
    match funding_from_ic_code(ic) {
        Some(f) => f.to_string(),
        None => funding_from_usage_type(usage_code),
    }
}

/// Synthesize a `variant` cluster key. Chase's Deposit Details carries no scheme *tier* on the card
/// row (the interchange qualification is captured separately as `ic_category`), so for cards we use
/// `{network}{funding}` (e.g. `visacredit`) and for wallets the report's own wallet form
/// (`{network}_applepay`). This matches how serving reconstructs the variant at decide time.
fn build_variant(network: &str, funding: &str, wallet: &str) -> String {
    let w = wallet.trim().to_lowercase();
    if w.contains("apple") {
        return format!("{network}_applepay");
    }
    if w.contains("google") {
        return format!("{network}_googlepay");
    }
    let v = format!("{network}{funding}");
    if v.is_empty() {
        network.to_string()
    } else {
        v
    }
}

/// Parse a money cell; blanks/garbage become `0.0` (mirrors Adyen's/Braintree's `to_float`).
fn to_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// Parse a Chase date cell (`Submission Date`), formatted US `M/D/YYYY` (e.g. `3/3/2025`). A
/// 2-digit-year fallback is kept for safety. Blank/odd values yield `None`.
fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Choose the year format by the last field's width so `%Y` doesn't read `25` as year 0025.
    let fmt = match s.rsplit('/').next().map(str::len) {
        Some(4) => "%m/%d/%Y",
        Some(2) => "%m/%d/%y",
        _ => return None,
    };
    chrono::NaiveDate::parse_from_str(s, fmt).ok()
}

/// Streams a J.P. Morgan preset report with its envelope stripped: a leading UTF-8 BOM, the
/// `BEGIN…` and `EntityId=…,ReportTypeName=…` preamble lines, and the trailing `END…` line are all
/// dropped, leaving a plain `header + data` CSV for [`csv_reader::parse`]. Line-buffered, so a
/// multi-GB report stays flat in memory.
struct EnvelopeReader<R> {
    inner: BufReader<R>,
    line: Vec<u8>, // current kept line (incl. its trailing '\n'), being handed out
    pos: usize,    // bytes of `line` already emitted
    first: bool,   // strip a UTF-8 BOM on the first physical line
    eof: bool,
}

impl<R: Read> EnvelopeReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner: BufReader::new(inner),
            line: Vec::new(),
            pos: 0,
            first: true,
            eof: false,
        }
    }
}

/// A framing line the CSV parser must not see: the `BEGIN`/`END` markers and the `EntityId=…`
/// metadata preamble. Data rows begin with a date and the header with `Merchant Order Number`/etc.,
/// so none of them collide with these prefixes.
fn is_envelope_line(line: &[u8]) -> bool {
    let s = line
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map(|i| &line[i..])
        .unwrap_or(line);
    s.starts_with(b"BEGIN,") || s.starts_with(b"END,") || s.starts_with(b"EntityId=")
}

impl<R: Read> Read for EnvelopeReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        use std::io::BufRead;
        loop {
            // Drain the current kept line first.
            if self.pos < self.line.len() {
                let n = (self.line.len() - self.pos).min(out.len());
                out[..n].copy_from_slice(&self.line[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            if self.eof {
                return Ok(0);
            }
            // Pull the next physical line.
            self.line.clear();
            self.pos = 0;
            if self.inner.read_until(b'\n', &mut self.line)? == 0 {
                self.eof = true;
                return Ok(0);
            }
            if self.first {
                if self.line.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    self.line.drain(0..3);
                }
                self.first = false;
            }
            if is_envelope_line(&self.line) {
                // Discard the frame line so the drain check at the loop top doesn't emit it, then
                // pull the next physical line.
                self.line.clear();
                continue;
            }
            // Kept line — loop back and emit it.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal Deposit Details report: BOM + BEGIN + metadata preamble, header, a sale, a refund,
    /// then the END trailer. Column order is intentionally the real report's order.
    const REPORT: &[u8] = b"\xef\xbb\xbfBEGIN,EntityId=418553,Frequency=adhoc\n\
EntityId=418553,ReportTypeName=SubmissionDetails,Frequency=adhoc,FromDate=2025-03-03,ToDate=2025-03-04\n\
Submission Date,Presentment Currency Code,Settlement Currency Code,Transaction Division External Identifier,Payment Method Code,Merchant Order Number,Account Number,Credit Card Expiration Year Month Number,Transaction Amount in Presentment Currency,Merchant Action Code,Action Type Code Text,Authorization Date,Country of Issuance,Transaction Type Code,Transaction Type Text,Merchant Category Code,Digital Token Method Code,Digital Wallet Brand,Interchange Qualification Code,Interchange Unit Fee,Fee Rate,Total Interchange Amount,Total Assessment Amount,Other Debit Passthrough Fees,Merchant Information Description,Surcharge Amount,Bank Sort Code,Card Usage Type\n\
3/3/2025,USD,USD,418554,VI,SYN0002500,401068XXXXXX9249,26-Jan,60,DP,SALE,3/3/2025,IN,7,ECI Indicator - Channel Encrypted Transaction,4722,0,,VINT,0.20,0.0315,-2.09,-0.084,-0.01,N,,,3\n\
3/3/2025,USD,USD,418554,VI,SYN0002501,476134XXXXXX0050,25-May,-20,RF,REFUND,3/3/2025,US,7,ECI Indicator - Channel Encrypted Transaction,5968,0,-0.0205,0.41,-0.022,0,N,,,1\n\
END,EntityId=418553,Frequency=adhoc\n";

    #[test]
    fn parses_sales_and_skips_refunds_and_envelope() {
        let rows = ChaseReportSource::new().parse_report(REPORT).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "only the sale row survives (refund + envelope skipped)"
        );
        let r = &rows[0];
        assert_eq!(r.txn_ref, "SYN0002500");
        assert_eq!(r.card_network, "visa", "VI -> visa");
        assert_eq!(r.funding, "credit", "Card Usage Type 3 -> credit");
        assert_eq!(r.variant, "visacredit");
        assert_eq!(r.issuer_country, "IN");
        assert_eq!(r.currency, "USD");
        assert_eq!(r.ic_category, "VINT");
        assert_eq!(r.channel, "ecom", "no Terminal Identifier column -> ecom");
        // Fees are negated to positive cost magnitudes.
        assert!((r.interchange - 2.09).abs() < 1e-9);
        assert!((r.scheme_fee - 0.084).abs() < 1e-9);
        assert!(
            (r.markup - 0.01).abs() < 1e-9,
            "Other Debit Passthrough Fees negated"
        );
        assert_eq!(r.commission, 0.0);
        assert!((r.total_fee - (2.09 + 0.084 + 0.01)).abs() < 1e-9);
        assert!(
            (r.gross - 60.0).abs() < 1e-9,
            "Transaction Amount is gross as-is"
        );
        assert_eq!(r.txn_date, chrono::NaiveDate::from_ymd_opt(2025, 3, 3));
    }

    #[test]
    fn debit_and_wallet_variants() {
        // signature debit (usage 1) with an Apple Pay wallet.
        let csv = b"BEGIN,EntityId=1,Frequency=adhoc\n\
Merchant Order Number,Payment Method Code,Card Usage Type,Country of Issuance,Settlement Currency Code,Interchange Qualification Code,Action Type Code Text,Transaction Amount in Presentment Currency,Total Interchange Amount,Total Assessment Amount,Digital Wallet Brand\n\
o1,MC,1,US,USD,MCEB,SALE,40,-0.30,-0.05,APPLE\n\
END,EntityId=1,Frequency=adhoc\n";
        let rows = ChaseReportSource::new().parse_report(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].card_network, "mc");
        assert_eq!(rows[0].funding, "debit", "usage 1 = signature debit");
        assert_eq!(rows[0].variant, "mc_applepay", "wallet takes precedence");
        assert_eq!(
            rows[0].markup, 0.0,
            "absent Other Debit Passthrough Fees -> 0"
        );
    }

    #[test]
    fn funding_reconciled_from_interchange_code() {
        // A Visa credit interchange program (V148, ~3.3%) mislabeled `Card Usage Type = 1` (signature
        // debit) — the export contamination. The interchange code wins, so it prices as credit.
        let csv = b"Merchant Order Number,Payment Method Code,Card Usage Type,Country of Issuance,Settlement Currency Code,Interchange Qualification Code,Action Type Code Text,Transaction Amount in Presentment Currency,Total Interchange Amount,Total Assessment Amount\n\
o1,VI,1,US,USD,V148,SALE,100,-3.15,-0.14\n\
o2,VI,1,US,USD,VSDD,SALE,100,-0.05,-0.13\n";
        let rows = ChaseReportSource::new().parse_report(csv).unwrap();
        assert_eq!(rows.len(), 2);
        // V148 is a credit program → corrected to credit despite usage=1.
        assert_eq!(
            rows[0].funding, "credit",
            "V148 credit program overrides usage=1"
        );
        assert_eq!(rows[0].variant, "visacredit");
        // VSDD is a genuine (regulated) debit program → stays debit, consistent with usage=1.
        assert_eq!(rows[1].funding, "debit", "VSDD debit program");
        assert_eq!(rows[1].variant, "visadebit");
    }

    #[test]
    fn unknown_method_code_forms_its_own_cluster() {
        let csv = b"Merchant Order Number,Payment Method Code,Card Usage Type,Country of Issuance,Settlement Currency Code,Interchange Qualification Code,Action Type Code Text,Transaction Amount in Presentment Currency,Total Interchange Amount,Total Assessment Amount\n\
o2,ED,3,GB,GBP,EDBT,SALE,100,-1.00,-0.20\n";
        let rows = ChaseReportSource::new().parse_report(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].card_network, "ed", "unknown MOP code lowercased");
        assert_eq!(rows[0].variant, "edcredit");
    }

    #[test]
    fn missing_required_column_errors() {
        let csv = b"Merchant Order Number,Action Type Code Text\no1,SALE\n";
        let err = ChaseReportSource::new().parse_report(csv).unwrap_err();
        let IngestError::MissingColumns {
            missing, required, ..
        } = err
        else {
            panic!("expected MissingColumns, got {err:?}");
        };
        // Every miss is reported at once, not just the first one resolved.
        assert_eq!(missing.len(), required.len() - 2, "all but the two present");
        assert!(missing.contains(&"Payment Method Code".to_string()));
        assert!(missing.contains(&"Total Interchange Amount".to_string()));
        assert!(!missing.contains(&"Merchant Order Number".to_string()));
    }

    #[test]
    fn helpers() {
        assert_eq!(normalize_network("VI"), "visa");
        assert_eq!(normalize_network("mc"), "mc");
        assert_eq!(normalize_network("AX"), "amex");
        assert_eq!(normalize_network(""), "");
        assert_eq!(funding_from_usage_type("1"), "debit");
        assert_eq!(funding_from_usage_type("2"), "debit");
        assert_eq!(funding_from_usage_type("3"), "credit");
        assert_eq!(funding_from_usage_type(""), "");
        // Reconciliation: interchange code wins over usage type when it's a known program.
        assert_eq!(
            reconcile_funding("1", "V148"),
            "credit",
            "credit program beats usage=1"
        );
        assert_eq!(
            reconcile_funding("3", "VSDD"),
            "debit",
            "debit program beats usage=3"
        );
        assert_eq!(
            reconcile_funding("1", "MCEB"),
            "debit",
            "unlisted code -> usage type"
        );
        assert_eq!(
            reconcile_funding("3", "UNKNOWN"),
            "credit",
            "unlisted code -> usage type"
        );
        assert_eq!(reconcile_funding("", ""), "", "no signal -> empty");
        assert_eq!(
            parse_date("3/3/2025"),
            chrono::NaiveDate::from_ymd_opt(2025, 3, 3)
        );
        assert_eq!(
            parse_date("12/21/2024"),
            chrono::NaiveDate::from_ymd_opt(2024, 12, 21)
        );
        assert_eq!(parse_date(""), None);
    }

    /// Shaped after J.P. Morgan's live `GET /api/v1/reports`: a `Requested` report (no
    /// `reportDetails`), the completed **Submission details** report we want (Deposit details' type,
    /// spelled with a space by the API), and a completed report of another type. Only the middle one
    /// survives.
    const REPORTS_LIST: &[u8] = br#"{
      "summarizedReports": [
        { "reportIdentifier": "req-1", "reportTypeNames": ["Submission details"],
          "reportStatus": "Requested",
          "intervalParam": { "reportingPeriodStartTimestamp": "2021-08-22 00:18:18" } },
        { "reportIdentifier": "dep-1", "reportTypeNames": ["Submission details"],
          "reportStatus": "Completed",
          "intervalParam": { "reportingPeriodStartTimestamp": "2021-08-20 00:18:18",
                             "reportingPeriodEndTimestamp": "2021-08-21 00:19:18" },
          "reportDetails": { "reportFileName": "dd.2021-08-20",
                             "url": "https://api.reports-files.jpmorgan.com/api/v1/report-files/dep-1" } },
        { "reportIdentifier": "set-1", "reportTypeNames": ["Settlement Summary", "Settlement Details"],
          "reportStatus": "Completed",
          "reportDetails": { "url": "https://api.reports-files.jpmorgan.com/api/v1/report-files/set-1" } }
      ],
      "lastPage": true
    }"#;

    #[test]
    fn parse_reports_list_keeps_only_completed_submission_details() {
        let ready = parse_reports_list(REPORTS_LIST).unwrap();
        assert_eq!(
            ready.len(),
            1,
            "only the completed Submission details report survives"
        );
        let r = &ready[0];
        assert_eq!(r.report_id, "dep-1");
        assert_eq!(
            r.report_ref,
            "https://api.reports-files.jpmorgan.com/api/v1/report-files/dep-1"
        );
        assert_eq!(r.period_start, chrono::NaiveDate::from_ymd_opt(2021, 8, 20));
        assert_eq!(r.period_end, chrono::NaiveDate::from_ymd_opt(2021, 8, 21));
    }

    #[test]
    fn submission_details_type_matches_both_spellings() {
        assert!(is_submission_details("Submission details"));
        assert!(is_submission_details("SubmissionDetails"));
        assert!(is_submission_details("submission details"));
        assert!(!is_submission_details("Settlement Details"));
        assert!(!is_submission_details("Transaction Details"));
    }

    #[test]
    fn parse_reports_list_tolerates_empty_and_missing_fields() {
        assert!(parse_reports_list(br#"{"summarizedReports": []}"#)
            .unwrap()
            .is_empty());
        assert!(parse_reports_list(br#"{}"#).unwrap().is_empty());
        assert!(parse_reports_list(b"not json").is_err());
    }

    #[test]
    fn creds_parse_applies_url_defaults() {
        let creds = ConnectorCreds {
            webhook_secret: Secret::new(String::new()),
            download_auth: Secret::new(
                r#"{"client_id":"c","resource":"r","private_key_pem":"pem"}"#.to_string(),
            ),
        };
        let chase = ChaseCreds::parse(&creds).unwrap();
        assert_eq!(chase.client_id, "c");
        assert_eq!(chase.token_url, default_token_url());
        assert_eq!(chase.reports_url, default_reports_url());
        // A bad blob is a clear, typed error.
        let bad = ConnectorCreds {
            webhook_secret: Secret::new(String::new()),
            download_auth: Secret::new("nope".to_string()),
        };
        assert!(matches!(
            ChaseCreds::parse(&bad),
            Err(IngestError::MalformedNotification(_))
        ));
    }

    #[test]
    fn token_form_has_the_five_client_credentials_params() {
        let creds = ChaseCreds {
            client_id: "cid".to_string(),
            resource: "res".to_string(),
            private_key_pem: String::new(),
            access_token: None,
            token_url: default_token_url(),
            reports_url: default_reports_url(),
        };
        let form = build_token_form(&creds, "ASSERTION");
        assert_eq!(form.len(), 5);
        let get = |k: &str| form.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("grant_type"), Some("client_credentials"));
        assert_eq!(get("client_id"), Some("cid"));
        assert_eq!(get("client_assertion_type"), Some(CLIENT_ASSERTION_TYPE));
        assert_eq!(get("client_assertion"), Some("ASSERTION"));
        assert_eq!(get("resource"), Some("res"));
    }
}
