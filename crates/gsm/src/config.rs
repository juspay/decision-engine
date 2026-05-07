use std::{collections::HashMap, path::Path};

use crate::{
    error::GsmError,
    interface::GsmLookup,
    types::{GsmDecision, GsmRule},
};

/// Internal struct that maps 1:1 to a CSV row from gateway_status_map.csv.
/// All fields are strings; conversion to typed `GsmRule` happens in `TryFrom`.
#[derive(Debug, serde::Deserialize)]
struct CsvRow {
    connector: String,
    flow: String,
    sub_flow: String,
    code: String,
    message: String,
    status: String,
    router_error: String,
    decision: String,
    created_at: String,
    last_modified: String,
    step_up_possible: String,
    unified_code: String,
    unified_message: String,
    error_category: String,
    clear_pan_possible: String,
    feature_data: String,
    feature: String,
    standardised_code: String,
    description: String,
    user_guidance_message: String,
}

fn none_if_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn parse_bool_string(s: &str) -> bool {
    matches!(s.to_uppercase().as_str(), "TRUE" | "1" | "YES")
}

impl TryFrom<CsvRow> for GsmRule {
    type Error = GsmError;

    fn try_from(row: CsvRow) -> Result<Self, Self::Error> {
        let decision = row
            .decision
            .parse::<GsmDecision>()
            .map_err(GsmError::InvalidDecision)?;

        let step_up_possible = parse_bool_string(&row.step_up_possible);
        let clear_pan_possible = parse_bool_string(&row.clear_pan_possible);
        let feature_data_raw = none_if_empty(row.feature_data);

        // `alternate_network_possible` lives inside the feature_data JSON blob.
        // We avoid a serde_json dependency here by doing a minimal tolerant scan
        // that handles both `"key":true` and `"key": true` (with whitespace).
        // This keeps the crate dependency-light for consumers like hyperswitch-prism.
        let alternate_network_possible = feature_data_raw
            .as_deref()
            .map(|s| {
                // Find the key anywhere in the string, then look for :true or : true
                s.contains("\"alternate_network_possible\":true")
                    || s.contains("\"alternate_network_possible\": true")
            })
            .unwrap_or(false);

        let _ = (row.created_at, row.last_modified); // present in CSV, not needed at runtime

        Ok(GsmRule {
            connector: row.connector,
            flow: row.flow,
            sub_flow: row.sub_flow,
            code: row.code,
            message: row.message,
            status: row.status,
            router_error: none_if_empty(row.router_error),
            decision,
            step_up_possible,
            clear_pan_possible,
            alternate_network_possible,
            feature_data_raw,
            unified_code: none_if_empty(row.unified_code),
            unified_message: none_if_empty(row.unified_message),
            error_category: none_if_empty(row.error_category),
            standardised_code: none_if_empty(row.standardised_code),
            description: none_if_empty(row.description),
            user_guidance_message: none_if_empty(row.user_guidance_message),
            feature: none_if_empty(row.feature),
        })
    }
}

/// In-memory GSM store backed by a `HashMap` keyed on
/// `(connector, flow, sub_flow, code, message)`.
///
/// Build once at startup from a CSV export of `gateway_status_map`.
/// Lookups are O(1); note: each lookup creates a temporary key (5 allocations).
/// Future optimization: consider a custom borrow-aware key type to eliminate these.
pub struct ConfigGsmStore {
    index: HashMap<(String, String, String, String, String), GsmRule>,
}

impl ConfigGsmStore {
    /// Load from a CSV string (e.g. from `include_str!` or an HTTP response body).
    pub fn from_csv_str(content: &str) -> Result<Self, GsmError> {
        let mut reader = csv::Reader::from_reader(content.as_bytes());
        let mut index = HashMap::new();

        for result in reader.deserialize::<CsvRow>() {
            let row = result?;
            let key = (
                row.connector.clone(),
                row.flow.clone(),
                row.sub_flow.clone(),
                row.code.clone(),
                row.message.clone(),
            );
            let rule = GsmRule::try_from(row)?;
            index.insert(key, rule);
        }

        Ok(Self { index })
    }

    /// Load from a CSV file on disk.
    pub fn from_csv_file(path: impl AsRef<Path>) -> Result<Self, GsmError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_csv_str(&content)
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn rules(&self) -> impl Iterator<Item = &GsmRule> {
        self.index.values()
    }
}

impl GsmLookup for ConfigGsmStore {
    fn find_gsm_rule(
        &self,
        connector: &str,
        flow: &str,
        sub_flow: &str,
        code: &str,
        message: &str,
    ) -> Option<&GsmRule> {
        self.index.get(&(
            connector.to_string(),
            flow.to_string(),
            sub_flow.to_string(),
            code.to_string(),
            message.to_string(),
        ))
    }
}
