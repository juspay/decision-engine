//! Helpers every connector's report source shares, so a connector module holds only what is specific
//! to it: its report columns, notification shape and download auth.

use std::sync::OnceLock;
use std::time::Duration;

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
