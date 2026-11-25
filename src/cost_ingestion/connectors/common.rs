//! Helpers every connector's report source shares, so a connector module holds only what is specific
//! to it: its report columns, notification shape and download auth.

use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use serde::de::{self, DeserializeOwned, IgnoredAny};

use crate::cost_ingestion::source::MAX_NOTIFICATION_BYTES;
use crate::cost_ingestion::IngestError;

/// Settlement reports run to several GB; a download gets this long before it is abandoned.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// The HTTP client report downloads and reporting-API calls go through, built once and shared.
pub(super) fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .expect("failed to build the report download client")
    })
}

/// Like [`http_client`], but never follows a redirect: for downloads that carry credentials in a
/// header, which a redirect would forward to wherever it points.
pub(super) fn http_client_without_redirects() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build the report download client")
    })
}

/// A numeric report cell; blank or unparsable reads as `0`.
pub(super) fn to_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// A US `M/D/YYYY` date cell, accepting a 2-digit year (`2/2/20`). Blank or odd values yield
/// `None`.
pub(super) fn parse_us_date(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Choose the year format by the last field's width — `%Y` would otherwise greedily read a
    // 2-digit `20` as year 0020.
    let fmt = match s.rsplit('/').next().map(str::len) {
        Some(4) => "%m/%d/%Y",
        Some(2) => "%m/%d/%y",
        _ => return None,
    };
    chrono::NaiveDate::parse_from_str(s, fmt).ok()
}

/// The error for a report-source method a connector does not support yet.
pub(super) fn not_implemented(connector: &str, method: &str) -> IngestError {
    IngestError::MalformedNotification(format!(
        "{connector} connector: {method} not yet implemented (webhook/download shape TBD)"
    ))
}

/// Reject a webhook body over [`MAX_NOTIFICATION_BYTES`] before anything reads it.
pub(super) fn check_notification_size(raw_body: &[u8]) -> Result<(), IngestError> {
    if raw_body.len() > MAX_NOTIFICATION_BYTES {
        return Err(IngestError::MalformedNotification(format!(
            "webhook body is {} bytes, over the {MAX_NOTIFICATION_BYTES}-byte limit",
            raw_body.len()
        )));
    }
    Ok(())
}

/// Parse a webhook body, which is not yet verified, as JSON into `T`. `T` names only the fields the
/// connector reads, so every other part of the payload is skipped rather than kept in memory.
pub(super) fn parse_notification_json<T: DeserializeOwned>(
    raw_body: &[u8],
) -> Result<T, IngestError> {
    check_notification_size(raw_body)?;
    let malformed =
        |e: serde_json::Error| IngestError::MalformedNotification(format!("webhook body: {e}"));
    let mut de = serde_json::Deserializer::from_slice(raw_body);
    let parsed = T::deserialize(&mut de).map_err(malformed)?;
    de.end().map_err(malformed)?;
    Ok(parsed)
}

/// `deserialize_with` for an optional string field: a string is kept, and any other JSON value
/// (number, object, `null`, …) reads as absent instead of failing the whole notification.
pub(super) fn lenient_string<'de, D: de::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    deserializer.deserialize_any(LenientString {
        keep_numbers: false,
    })
}

/// Like [`lenient_string`], but a JSON number is kept too, in its JSON text form (`1000`).
pub(super) fn string_or_number<'de, D: de::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    deserializer.deserialize_any(LenientString { keep_numbers: true })
}

struct LenientString {
    keep_numbers: bool,
}

impl<'de> de::Visitor<'de> for LenientString {
    type Value = Option<String>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Some(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(Some(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(self.keep_numbers.then(|| v.to_string()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(self.keep_numbers.then(|| v.to_string()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        Ok(self
            .keep_numbers
            .then(|| serde_json::Number::from_f64(v).map(|n| n.to_string()))
            .flatten())
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: de::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_dates_read_two_and_four_digit_years() {
        let date = chrono::NaiveDate::from_ymd_opt(2025, 3, 3);
        assert_eq!(parse_us_date("3/3/2025"), date);
        assert_eq!(parse_us_date(" 3/3/25 "), date);
        assert_eq!(parse_us_date(""), None);
        assert_eq!(parse_us_date("2025-03-03"), None);
    }

    #[test]
    fn numeric_cells_default_to_zero() {
        assert_eq!(to_float(" 1.5 "), 1.5);
        assert_eq!(to_float(""), 0.0);
        assert_eq!(to_float("n/a"), 0.0);
    }
}
