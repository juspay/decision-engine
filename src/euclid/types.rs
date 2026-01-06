use super::ast::ConnectorInfo;
use crate::euclid::ast::{Output, Program, ValueType};
#[cfg(feature = "mysql")]
use crate::storage::schema;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg;
use diesel::prelude::AsChangeset;
use diesel::Identifiable;
use diesel::Insertable;
use diesel::{Queryable, Selectable};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, ops::Deref};
use time::PrimitiveDateTime;

pub type Metadata = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, strum::Display, PartialEq)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DataType {
    Number,
    EnumVariant,
    MetadataValue,
    StrValue,
    GlobalRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    pub rule_id: Option<String>,
    pub name: String,
    pub description: String,
    pub created_by: String,
    pub algorithm: StaticRoutingAlgorithm,
    #[serde(default)]
    pub algorithm_for: AlgorithmType,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, strum::Display)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum StaticRoutingAlgorithm {
    Single(Box<ConnectorInfo>),
    Priority(Vec<ConnectorInfo>),
    VolumeSplit(Vec<super::ast::VolumeSplit<ConnectorInfo>>),
    Advanced(Program),
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AlgorithmType {
    #[default]
    Payment,
    Payout,
    ThreeDsAuthentication,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct RoutingRequest {
    pub created_by: String,
    pub fallback_output: Option<Vec<ConnectorInfo>>,
    pub parameters: HashMap<String, Option<ValueType>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendOutput {
    pub rule_name: Option<String>,
    pub output: Output,
    pub evaluated_output: Vec<ConnectorInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutingDictionaryRecord {
    pub rule_id: String,
    pub name: String,
    pub algorithm_for: String,
    pub created_at: time::PrimitiveDateTime,
    pub modified_at: time::PrimitiveDateTime,
}

impl RoutingDictionaryRecord {
    pub fn new(
        rule_id: String,
        name: String,
        algorithm_for: String,
        created_at: time::PrimitiveDateTime,
        modified_at: time::PrimitiveDateTime,
    ) -> Self {
        Self {
            rule_id,
            name,
            algorithm_for,
            created_at,
            modified_at,
        }
    }
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SrDimensionConfig {
    pub merchant_id: String,
    pub paymentInfo: SrDimensionInfo,
}
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SrDimensionInfo {
    pub udfs: Vec<i32>,
    pub fields: Option<Vec<String>>,
}
pub const ELIGIBLE_DIMENSIONS: [&str; 5] = [
    "currency",
    "country",
    "auth_type",
    "card_is_in",
    "card_network",
];
#[derive(Debug, serde::Serialize)]
pub struct RoutingEvaluateResponse {
    pub status: String,
    pub output: serde_json::Value,
    pub evaluated_output: Vec<ConnectorInfo>,
    pub eligible_connectors: Vec<ConnectorInfo>,
}

// #[derive(AsChangeset, Debug, Clone, Identifiable, Insertable, Queryable, Selectable)]
#[derive(
    AsChangeset,
    Insertable,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    Identifiable,
    Queryable,
    Selectable,
)]
#[cfg_attr(feature = "mysql", diesel(table_name = schema::routing_algorithm))]
#[cfg_attr(feature = "postgres", diesel(table_name = schema_pg::routing_algorithm))]
pub struct RoutingAlgorithm {
    pub id: String,
    pub created_by: String,
    pub name: String,
    pub description: String,
    // #[cfg(feature = "mysql")]
    pub algorithm_data: String,
    pub algorithm_for: String,
    // #[cfg(feature = "postgres")]
    // pub algorithm_data: serde_json::Value,
    #[cfg(feature = "postgres")]
    pub metadata: Option<serde_json::Value>,
    #[cfg(feature = "mysql")]
    pub metadata: Option<String>,
    pub created_at: PrimitiveDateTime,
    pub modified_at: PrimitiveDateTime,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct JsonifiedRoutingAlgorithm {
    pub id: String,
    pub created_by: String,
    pub name: String,
    pub description: String,
    pub algorithm_data: serde_json::Value,
    pub algorithm_for: String,
    pub created_at: PrimitiveDateTime,
    pub modified_at: PrimitiveDateTime,
}

impl From<RoutingAlgorithm> for JsonifiedRoutingAlgorithm {
    fn from(ra: RoutingAlgorithm) -> Self {
        let algorithm_data: serde_json::Value =
            serde_json::from_str(&ra.algorithm_data).unwrap_or_else(|_| serde_json::Value::Null);

        JsonifiedRoutingAlgorithm {
            id: ra.id,
            created_by: ra.created_by,
            name: ra.name,
            description: ra.description,
            algorithm_data,
            algorithm_for: ra.algorithm_for,
            created_at: ra.created_at,
            modified_at: ra.modified_at,
        }
    }
}

#[derive(
    AsChangeset, Insertable, Debug, serde::Serialize, serde::Deserialize, Identifiable, Queryable,
)]
#[cfg_attr(feature = "mysql", diesel(table_name = schema::routing_algorithm_mapper))]
#[cfg_attr(feature = "postgres", diesel(table_name = schema_pg::routing_algorithm_mapper))]
#[diesel(primary_key(id))]
pub struct RoutingAlgorithmMapper {
    pub id: i32,
    pub created_by: String,
    pub routing_algorithm_id: String,
    pub algorithm_for: String,
}

#[derive(Insertable, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "mysql", diesel(table_name = schema::routing_algorithm_mapper))]
#[cfg_attr(feature = "postgres", diesel(table_name = schema_pg::routing_algorithm_mapper))]
pub struct RoutingAlgorithmMapperNew {
    pub created_by: String,
    pub routing_algorithm_id: String,
    pub algorithm_for: String,
}

impl RoutingAlgorithmMapperNew {
    pub fn new(created_by: String, routing_algorithm_id: String, algorithm_for: String) -> Self {
        Self {
            created_by,
            routing_algorithm_id,
            algorithm_for,
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ActivateRoutingConfigRequest {
    pub created_by: String,
    pub routing_algorithm_id: String,
}

#[derive(AsChangeset, Debug, serde::Serialize, serde::Deserialize, Queryable, Selectable)]
#[cfg_attr(feature = "mysql", diesel(table_name = schema::routing_algorithm_mapper))]
#[cfg_attr(feature = "postgres", diesel(table_name = schema_pg::routing_algorithm_mapper))]
pub struct RoutingAlgorithmMapperUpdate {
    pub routing_algorithm_id: String,
    pub algorithm_for: String,
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum InterpreterErrorType {
    #[error("Invalid key received '{0}'")]
    InvalidKey(String),
    #[error("Invalid Comparison")]
    InvalidComparison,
    #[error("Invalid Output '{0}'")]
    OutputEvaluationFailed(String),
}

#[derive(Debug, Clone, Serialize, thiserror::Error)]
pub struct InterpreterError {
    pub error_type: InterpreterErrorType,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        InterpreterErrorType::fmt(&self.error_type, f)
    }
}

#[derive(Debug)]
pub struct Context(HashMap<String, Option<ValueType>>);
impl Context {
    pub fn new(parameters: HashMap<String, Option<ValueType>>) -> Self {
        Self(parameters)
    }
}
impl Deref for Context {
    type Target = HashMap<String, Option<ValueType>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ValidationConstraints {
    #[serde(default)]
    pub min: Option<i64>,
    #[serde(default)]
    pub max: Option<i64>,
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub exact_length: Option<usize>,
    #[serde(default)]
    pub regex: Option<String>,
}

/// Represents a key configuration in the TOML file
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyConfig {
    #[serde(rename = "type")]
    pub data_type: String,
    #[serde(default)]
    pub values: Option<String>,
    #[serde(default)]
    pub min: Option<i64>,
    #[serde(default)]
    pub max: Option<i64>,
    #[serde(default)]
    pub min_length: Option<usize>,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub exact_length: Option<usize>,
    #[serde(default)]
    pub regex: Option<String>,
}

impl KeyConfig {
    pub fn get_validation_constraints(&self) -> ValidationConstraints {
        ValidationConstraints {
            min: self.min,
            max: self.max,
            min_length: self.min_length,
            max_length: self.max_length,
            exact_length: self.exact_length,
            regex: self.regex.clone(),
        }
    }

    pub fn has_validation_constraints(&self) -> bool {
        self.min.is_some()
            || self.max.is_some()
            || self.min_length.is_some()
            || self.max_length.is_some()
            || self.exact_length.is_some()
            || self.regex.is_some()
    }

    pub fn build_validation_rules(&self) -> Result<FieldValidationRules, String> {
        let regex_pattern = match &self.regex {
            Some(pattern) => Some(
                regex::Regex::new(pattern)
                    .map_err(|e| format!("Invalid regex pattern '{}': {}", pattern, e))?,
            ),
            None => None,
        };

        Ok(FieldValidationRules {
            numeric_range: match (self.min, self.max) {
                (Some(min), Some(max)) => Some((min, max)),
                (Some(min), None) => Some((min, i64::MAX)),
                (None, Some(max)) => Some((i64::MIN, max)),
                (None, None) => None,
            },
            length_range: match (self.min_length, self.max_length) {
                (Some(min), Some(max)) => Some((min, max)),
                (Some(min), None) => Some((min, usize::MAX)),
                (None, Some(max)) => Some((0, max)),
                (None, None) => None,
            },
            exact_length: self.exact_length,
            regex_pattern,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FieldValidationRules {
    pub numeric_range: Option<(i64, i64)>,
    pub length_range: Option<(usize, usize)>,
    pub exact_length: Option<usize>,
    pub regex_pattern: Option<regex::Regex>,
}

impl FieldValidationRules {
    pub fn validate_numeric(&self, field: &str, value: i64) -> Result<(), String> {
        if let Some((min, max)) = self.numeric_range {
            if value < min {
                return Err(format!(
                    "value {} is below minimum {} for field '{}'",
                    value, min, field
                ));
            }
            if value > max {
                return Err(format!(
                    "value {} exceeds maximum {} for field '{}'",
                    value, max, field
                ));
            }
        }
        Ok(())
    }

    pub fn validate_string(&self, field: &str, value: &str) -> Result<(), String> {
        let len = value.len();

        if let Some(exact) = self.exact_length {
            if len != exact {
                return Err(format!(
                    "expected exactly {} characters, got {} for field '{}'",
                    exact, len, field
                ));
            }
        }

        if let Some((min, max)) = self.length_range {
            if len < min {
                return Err(format!(
                    "length {} is below minimum {} for field '{}'",
                    len, min, field
                ));
            }
            if len > max {
                return Err(format!(
                    "length {} exceeds maximum {} for field '{}'",
                    len, max, field
                ));
            }
        }

        if let Some(ref pattern) = self.regex_pattern {
            if !pattern.is_match(value) {
                return Err(format!(
                    "value '{}' does not match required pattern for field '{}'",
                    value, field
                ));
            }
        }

        Ok(())
    }

    pub fn has_rules(&self) -> bool {
        self.numeric_range.is_some()
            || self.length_range.is_some()
            || self.exact_length.is_some()
            || self.regex_pattern.is_some()
    }
}

impl Default for FieldValidationRules {
    fn default() -> Self {
        Self {
            numeric_range: None,
            length_range: None,
            exact_length: None,
            regex_pattern: None,
        }
    }
}

/// Structure for the [keys] section in the TOML
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeysConfig {
    #[serde(flatten)]
    pub keys: HashMap<String, KeyConfig>,
}

/// Structure for the [default] section in the TOML
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DefaultConfig {
    pub output: Vec<String>,
}

/// The complete TOML configuration structure
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlConfig {
    pub keys: KeysConfig,
    pub default: DefaultConfig,
    #[serde(default)]
    pub constraint_graph: crate::euclid::cgraph::ConstraintGraph,
}

impl Default for TomlConfig {
    fn default() -> Self {
        Self {
            keys: KeysConfig::default(),
            default: DefaultConfig::default(),
            constraint_graph: crate::euclid::cgraph::ConstraintGraph::default(),
        }
    }
}

impl Default for KeysConfig {
    fn default() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self { output: Vec::new() }
    }
}
