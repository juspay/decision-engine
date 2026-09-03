use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;
use serde::Deserialize;

use crate::app::get_tenant_app_state;
use crate::logger;
use crate::types::card::card_info::CardInfo;
use crate::types::card::card_type::to_card_type;
use crate::types::card::isin::to_isin;

/// Subset of the Hyperswitch `GET /api/cards/{bin}` response (upstream `CardInfoResponse`)
/// that we consume for BIN enrichment.
#[derive(Debug, Deserialize)]
struct CardInfoResponse {
    #[serde(rename = "card_iin")]
    card_bin: String,
    /// Card network / switch provider, e.g. "Mastercard".
    #[serde(default)]
    card_network: Option<String>,
    /// Funding type, e.g. "DEBIT" / "CREDIT".
    #[serde(default)]
    card_type: Option<String>,
    /// Card sub-type / product, e.g. "DEBIT STANDARD".
    #[serde(default)]
    card_sub_type: Option<String>,
    /// Card segment, e.g. "Consumer" / "Commercial".
    #[serde(default)]
    card_segment_type: Option<String>,
    /// ISO alpha-2 issuing country code, e.g. "CN".
    #[serde(default)]
    country_code: Option<String>,
    /// Numeric ISO country code, e.g. "528".
    #[serde(default)]
    numeric_country_code: Option<String>,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(30)))
            .build()
            .expect("failed to build card-info reqwest client")
    })
}

/// Response headers worth keeping on failure rows: request correlation (`x-request-id`),
/// throttling (`retry-after`), upstream latency (`x-envoy-upstream-service-time`), and
/// enough envelope info (`content-type`, `server`, `via`, `date`) to tell an origin
/// error from a proxy/LB one.
const USEFUL_RESPONSE_HEADERS: &[&str] = &[
    "content-type",
    "content-length",
    "date",
    "server",
    "via",
    "x-request-id",
    "retry-after",
    "x-envoy-upstream-service-time",
    "cf-ray",
];

const RESPONSE_BODY_SNIPPET_MAX_BYTES: usize = 2048;

/// Reads at most `RESPONSE_BODY_SNIPPET_MAX_BYTES` of the response body, chunk by chunk,
/// so an arbitrarily large upstream error page never gets buffered whole.
async fn read_body_snippet(mut response: reqwest::Response) -> Option<String> {
    let mut buf: Vec<u8> = Vec::new();
    while buf.len() < RESPONSE_BODY_SNIPPET_MAX_BYTES {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let remaining = RESPONSE_BODY_SNIPPET_MAX_BYTES - buf.len();
                buf.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            }
            Ok(None) | Err(_) => break,
        }
    }
    if buf.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&buf).into_owned())
    }
}

fn useful_response_headers(headers: &reqwest::header::HeaderMap) -> serde_json::Value {
    serde_json::Value::Object(
        USEFUL_RESPONSE_HEADERS
            .iter()
            .filter_map(|name| {
                headers
                    .get(*name)
                    .and_then(|value| value.to_str().ok())
                    .map(|value| ((*name).to_string(), serde_json::Value::from(value)))
            })
            .collect(),
    )
}

/// Ships a failed lookup to ClickHouse (via the analytics domain-event queue) so the
/// cards-API dependency can be monitored and alerted on from Grafana. `endpoint` must be
/// the BIN-free base URL: the BIN already rides the typed `card_is_in` column, so the
/// `details` payload stays free of sensitive-data duplication.
#[allow(clippy::too_many_arguments)]
fn record_lookup_failure(
    bin: &str,
    endpoint: &str,
    error_code: String,
    error_message: &str,
    latency_ms: u64,
    status_code: Option<u16>,
    response_headers: Option<serde_json::Value>,
    response_body: Option<String>,
) {
    let details = serde_json::json!({
        "endpoint": endpoint,
        "status_code": status_code,
        "latency_ms": latency_ms,
        "response_headers": response_headers,
        "response_body": response_body,
    });
    crate::analytics::DomainAnalyticsEvent::record_card_info_lookup_failure(
        Some(bin.to_string()),
        error_code,
        error_message.to_string(),
        Some(details.to_string()),
        Some(latency_ms as f64),
    );
}

/// Normalizes a raw card BIN for the cards API, which supports IIN lookups from 6 up to 8
fn normalize_bin(bin: &str) -> String {
    let digits: String = bin.chars().filter(char::is_ascii_digit).collect();
    if digits.len() > 8 {
        digits[..8].to_string()
    } else {
        digits
    }
}

/// Fetches card metadata for a BIN from the Hyperswitch cards API and maps it into the decision-engine [`CardInfo`].
pub async fn get_card_info_by_bin(card_bin: Option<String>) -> Option<CardInfo> {
    logger::debug!("get_card_info_by_bin (cards API) cardBin: {:?}", card_bin);

    let raw_bin = card_bin
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())?;
    let bin = normalize_bin(&raw_bin);
    if bin.len() < 6 {
        logger::warn!(
            tag = "cardInfoApi",
            "card BIN {:?} too short after normalization; skipping lookup",
            raw_bin
        );
        return None;
    }

    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.card_info_service;
    if cfg.api_key.peek().is_empty() {
        logger::warn!(
            tag = "cardInfoApi",
            "card_info_service.api_key not configured; skipping BIN enrichment"
        );
        return None;
    }

    let endpoint = cfg.base_url.trim_end_matches('/');
    let url = format!("{}/{}", endpoint, bin);
    let fut = client()
        .get(&url)
        .header("api-key", cfg.api_key.peek().as_str())
        .header("x-tenant-id", cfg.tenant_id.as_str())
        .send();

    let started = std::time::Instant::now();
    let response = match tokio::time::timeout(Duration::from_millis(cfg.timeout_ms), fut).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            logger::warn!(
                tag = "cardInfoApi",
                "request error for bin {}: {:?}",
                bin,
                e
            );
            let status_code = e.status().map(|status| status.as_u16());
            record_lookup_failure(
                &bin,
                endpoint,
                "REQUEST_ERROR".to_string(),
                // without_url: reqwest embeds the full URL (and thus the BIN) in messages.
                &e.without_url().to_string(),
                started.elapsed().as_millis() as u64,
                status_code,
                None,
                None,
            );
            return None;
        }
        Err(_) => {
            logger::warn!(
                tag = "cardInfoApi",
                "timeout after {}ms for bin {}",
                cfg.timeout_ms,
                bin
            );
            record_lookup_failure(
                &bin,
                endpoint,
                "TIMEOUT".to_string(),
                &format!("no response within {}ms", cfg.timeout_ms),
                cfg.timeout_ms,
                None,
                None,
                None,
            );
            return None;
        }
    };

    let status = response.status();
    let response_headers = useful_response_headers(response.headers());

    if !status.is_success() {
        logger::warn!(tag = "cardInfoApi", "non-2xx for bin {}: {}", bin, status);
        let body_snippet = read_body_snippet(response).await;
        record_lookup_failure(
            &bin,
            endpoint,
            status.as_u16().to_string(),
            "non-2xx response from cards API",
            started.elapsed().as_millis() as u64,
            Some(status.as_u16()),
            Some(response_headers),
            body_snippet,
        );
        return None;
    }

    match response.json::<CardInfoResponse>().await {
        Ok(body) => map_response_to_card_info(body),
        Err(e) => {
            logger::warn!(tag = "cardInfoApi", "parse error for bin {}: {:?}", bin, e);
            record_lookup_failure(
                &bin,
                endpoint,
                "PARSE_ERROR".to_string(),
                &e.without_url().to_string(),
                started.elapsed().as_millis() as u64,
                Some(status.as_u16()),
                Some(response_headers),
                None,
            );
            None
        }
    }
}

fn map_response_to_card_info(res: CardInfoResponse) -> Option<CardInfo> {
    let card_isin = match to_isin(res.card_bin.clone()) {
        Ok(isin) => isin,
        Err(_) => {
            logger::warn!(
                tag = "cardInfoApi",
                "unparsable card_bin {:?} from cards API",
                res.card_bin
            );
            return None;
        }
    };

    // Soft-fail to None on an unrecognized card_type rather than dropping the whole enrichment.
    let card_type = res
        .card_type
        .as_deref()
        .and_then(|ct| to_card_type(ct).ok());

    let card_issuer_country = res.country_code;

    Some(CardInfo {
        card_isin,
        card_switch_provider: res.card_network.unwrap_or_default(),
        card_type,
        card_sub_type: res.card_sub_type,
        card_sub_type_category: res.card_segment_type,
        card_issuer_country,
        country_code: res.numeric_country_code,
        extended_card_type: None,
    })
}
