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
//! // Load from a CSV exported from hyperswitch's gateway_status_map table.
//! let store = ConfigGsmStore::from_csv_file("config/gsm.csv").unwrap();
//!
//! // O(1) lookup — safe to call on every payment decision.
//! if let Some(rule) = store.find_gsm_rule("adyen", "Payment", "Authorize", "2", "Refused") {
//!     println!("decision: {}", rule.decision);
//!     println!("unified_code: {:?}", rule.unified_code);
//! }
//! ```
//!
//! # Using with hyperswitch-prism (embed rules at compile time)
//!
//! ```rust,ignore
//! static GSM_CSV: &str = include_str!("../data/gsm.csv");
//! let store = ConfigGsmStore::from_csv_str(GSM_CSV).unwrap();
//! ```
//!
//! # Implementing your own backing store
//!
//! Implement [`GsmLookup`] on any type that can resolve a 5-tuple key to a [`GsmRule`].
//! hyperswitch can implement it on its async `Store` wrapper for future integration.

pub mod config;
pub mod error;
pub mod interface;
pub mod lookup;
pub mod types;

pub use config::ConfigGsmStore;
pub use error::GsmError;
pub use interface::GsmLookup;
pub use lookup::{get_gsm_rule, lookup_from_error_info};
pub use types::{GsmDecision, GsmErrorInfo, GsmInfo, GsmOptionRow, GsmRule};

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

        // Issuer lookup will miss (no issuer rules in sample CSV), falls back to connector code
        let rule = get_gsm_rule(
            &store,
            "adyen",
            "Payment",
            "Authorize",
            Some("2"),
            Some("Refused"),
            Some("51"),   // issuer code
            Some("Visa"), // card network
        );
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().decision, GsmDecision::Retry);
    }
}
