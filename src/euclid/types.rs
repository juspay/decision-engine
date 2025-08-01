use super::ast::ConnectorInfo;
use super::utils::generate_random_id;
use crate::decider::network_decider;
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
    pub fields: Vec<String>,
}
pub const ELIGIBLE_DIMENSIONS: [&str; 5] = [
    "paymentInfo.currency",
    "paymentInfo.country",
    "paymentInfo.auth_type",
    "paymentInfo.card_is_in",
    "paymentInfo.card_network",
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

/// Represents a key configuration in the TOML file
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct KeyConfig {
    #[serde(rename = "type")]
    pub data_type: String,
    #[serde(default)]
    pub values: Option<String>,
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
