/// Routing decision derived from a GSM rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GsmDecision {
    Retry,
    #[default]
    DoDefault,
}

impl std::fmt::Display for GsmDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retry => write!(f, "retry"),
            Self::DoDefault => write!(f, "do_default"),
        }
    }
}

impl std::str::FromStr for GsmDecision {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "retry" => Ok(Self::Retry),
            "do_default" => Ok(Self::DoDefault),
            other => Err(other.to_string()),
        }
    }
}

/// A single GSM rule mapping a connector error to a routing decision and unified error info.
///
/// The lookup key is the 5-tuple: (connector, flow, sub_flow, code, message).
#[derive(Debug, Clone)]
pub struct GsmRule {
    pub connector: String,
    pub flow: String,
    pub sub_flow: String,
    pub code: String,
    pub message: String,

    /// Normalised status string e.g. "failure", "success".
    pub status: String,

    /// Optional router-level error classification.
    pub router_error: Option<String>,

    /// Whether to retry or fall back to default handling.
    pub decision: GsmDecision,

    // Retry feature flags
    pub step_up_possible: bool,
    pub clear_pan_possible: bool,
    /// Extracted from the `feature_data` JSON column when present; defaults to `false`.
    /// Callers needing the raw blob can inspect `feature_data_raw`.
    pub alternate_network_possible: bool,
    /// Raw JSON string from the `feature_data` column, if present.
    pub feature_data_raw: Option<String>,

    // Unified error fields — used by hyperswitch-prism and error display
    pub unified_code: Option<String>,
    pub unified_message: Option<String>,
    pub error_category: Option<String>,
    pub standardised_code: Option<String>,
    pub description: Option<String>,
    pub user_guidance_message: Option<String>,

    pub feature: Option<String>,
}

/// Error context from a failed payment attempt, used as input for a GSM lookup.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GsmErrorInfo {
    pub connector: String,
    #[serde(default)]
    pub flow: String,
    #[serde(default)]
    pub sub_flow: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub issuer_error_code: Option<String>,
    pub card_network: Option<String>,
}

/// Result of a GSM lookup — routing decision and unified error details.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GsmInfo {
    /// `"retry"` or `"do_default"`.
    pub decision: String,
    pub step_up_possible: bool,
    pub clear_pan_possible: bool,
    pub alternate_network_possible: bool,
    pub unified_code: Option<String>,
    pub unified_message: Option<String>,
    pub error_category: Option<String>,
    pub standardised_code: Option<String>,
    pub description: Option<String>,
    pub user_guidance_message: Option<String>,
}

/// One row in the `/gsm/options` response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GsmOptionRow {
    pub connector: String,
    pub flow: String,
    pub sub_flow: String,
    pub error_code: String,
    pub error_message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unified_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unified_message: Option<String>,
    pub decision: String,
}
