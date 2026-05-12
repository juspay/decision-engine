//! GSM — Gateway Status Map
//!
//! Maps connector-specific error codes to routing decisions (`retry` / `do_default`)
//! and unified error information (`unified_code`, `unified_message`, etc.).
//!
//! # Quick start
//!
//! ```rust,no_run
//! use gsm::{ConfigGsmStore, GsmLookup};
//!
//! let store = ConfigGsmStore::from_csv_file("config/gsm.csv").unwrap();
//!
//! // O(1) lookup — safe to call on every payment decision.
//! if let Some(rule) = store.find_gsm_rule("adyen", "Payment", "Authorize", "2", "Refused") {
//!     println!("decision: {}", rule.decision);
//!     println!("unified_code: {:?}", rule.unified_code);
//! }
//! ```
//!
//! # Static store with runtime source selection (requires `loader` feature)
//!
//! ```rust,ignore
//! // Called once at startup — source driven by [gsm] in your TOML config.
//! gsm::init(&config.gsm).await;
//!
//! // Thereafter, zero-cost sync lookups from anywhere.
//! let info = gsm::lookup(&error_info);
//! ```
//!
//! # Implementing your own backing store
//!
//! Implement [`GsmLookup`] on any type that can resolve a 5-tuple key to a [`GsmRule`].

pub mod config;
pub mod error;
pub mod interface;
pub mod lookup;
pub mod source;
pub mod types;

#[cfg(feature = "loader")]
pub mod loader;

pub use config::ConfigGsmStore;
pub use error::GsmError;
pub use interface::GsmLookup;
pub use lookup::{get_gsm_rule, lookup_from_error_info};
pub use source::{GsmConfig, GsmSourceKind};
pub use types::{GsmDecision, GsmErrorInfo, GsmInfo, GsmOptionRow, GsmRule};

#[cfg(feature = "loader")]
pub use loader::{get_store, init, lookup, options};

#[cfg(test)]
mod tests {
    use super::*;

    static SAMPLE_CSV: &str = include_str!("../data/gsm.csv");

    #[test]
    fn parses_bundled_csv_without_error() {
        let store = ConfigGsmStore::from_csv_str(SAMPLE_CSV).expect("CSV should parse");
        assert!(!store.is_empty(), "store must contain at least one rule");
    }

    #[test]
    fn finds_known_adyen_authorize_rule() {
        let store = ConfigGsmStore::from_csv_str(SAMPLE_CSV).expect("CSV should parse");

        let rule = store.find_gsm_rule("adyen", "Payment", "Authorize", "2", "Refused");
        assert!(
            rule.is_some(),
            "rule for adyen/Payment/Authorize/2/Refused must exist"
        );

        let rule = rule.unwrap();
        assert_eq!(rule.decision, GsmDecision::Retry);
        assert_eq!(rule.unified_code.as_deref(), Some("UE_3000"));
        assert!(!rule.step_up_possible);
        assert!(!rule.clear_pan_possible);
        assert!(!rule.alternate_network_possible);
    }

    #[test]
    fn returns_none_for_unknown_rule() {
        let store = ConfigGsmStore::from_csv_str(SAMPLE_CSV).expect("CSV should parse");
        let rule = store.find_gsm_rule("unknown_connector", "Payment", "Authorize", "999", "err");
        assert!(rule.is_none());
    }

    #[test]
    fn get_gsm_rule_falls_back_to_connector_code() {
        let store = ConfigGsmStore::from_csv_str(SAMPLE_CSV).expect("CSV should parse");

        // No issuer code — should fall back to connector error code path
        let rule = get_gsm_rule(
            &store,
            "adyen",
            "Payment",
            "Authorize",
            Some("2"),
            Some("Refused"),
            None,
            None,
        );
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().decision, GsmDecision::Retry);
    }

    #[test]
    fn get_gsm_rule_issuer_lookup_misses_then_falls_back() {
        let store = ConfigGsmStore::from_csv_str(SAMPLE_CSV).expect("CSV should parse");

        // Use an issuer code that has no matching rule so the lookup falls back to connector code.
        let rule = get_gsm_rule(
            &store,
            "adyen",
            "Payment",
            "Authorize",
            Some("2"),
            Some("Refused"),
            Some("9999"), // issuer code with no matching rule
            Some("UnknownNetwork"),
        );
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().decision, GsmDecision::Retry);
    }
}
