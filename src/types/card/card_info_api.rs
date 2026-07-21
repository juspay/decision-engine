use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;
use serde::Deserialize;

use crate::app::get_tenant_app_state;
use crate::logger;
use crate::types::card::card_info::CardInfo;
use crate::types::card::card_type::to_card_type;
use crate::types::card::isin::to_isin;
use crate::types::country::country_name::country_name_to_iso2_code;

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
    /// Issuing country name, e.g. "NETHERLANDS".
    #[serde(default)]
    card_issuing_country: Option<String>,
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

    let url = format!("{}/{}", cfg.base_url.trim_end_matches('/'), bin);
    let fut = client()
        .get(&url)
        .header("api-key", cfg.api_key.peek().as_str())
        .send();

    let response = match tokio::time::timeout(Duration::from_millis(cfg.timeout_ms), fut).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            logger::warn!(
                tag = "cardInfoApi",
                "request error for bin {}: {:?}",
                bin,
                e
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
            return None;
        }
    };

    if !response.status().is_success() {
        logger::warn!(
            tag = "cardInfoApi",
            "non-2xx for bin {}: {}",
            bin,
            response.status()
        );
        return None;
    }

    match response.json::<CardInfoResponse>().await {
        Ok(body) => map_response_to_card_info(body),
        Err(e) => {
            logger::warn!(tag = "cardInfoApi", "parse error for bin {}: {:?}", bin, e);
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

    // The API returns a free-text country *name* ("NETHERLANDS"); normalize it to its ISO
    // alpha-2 code ("NL") via CountryISO2 so downstream region bucketing (issuer_region) works.
    // An unrecognized name yields None rather than storing a non-code string.
    let card_issuer_country = res.card_issuing_country.as_deref().and_then(|name| {
        let code = country_name_to_iso2_code(name);
        if code.is_none() {
            logger::warn!(
                tag = "cardInfoApi",
                "unrecognized card_issuing_country {:?}; dropping",
                name
            );
        }
        code
    });

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
