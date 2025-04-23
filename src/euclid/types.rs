use crate::euclid::ast::{Output, Program, ValueType};
use diesel::prelude::AsChangeset;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, ops::Deref};

pub type Metadata = HashMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, strum::Display)]
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
    pub name: String,
    pub algorithm: Program,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingRequest {
    pub routing_id: String,
    pub parameters: HashMap<String, Option<ValueType>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendOutput {
    pub rule_name: Option<String>,
    pub output: Output,
    pub evaluated_output: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutingDictionaryRecord {
    pub rule_id: String,
    pub name: String,
    pub created_at: time::PrimitiveDateTime,
    pub modified_at: time::PrimitiveDateTime,
}

impl RoutingDictionaryRecord {
    pub fn new(
        rule_id: String,
        name: String,
        created_at: time::PrimitiveDateTime,
        modified_at: time::PrimitiveDateTime,
    ) -> Self {
        Self {
            rule_id,
            name,
            created_at,
            modified_at,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct RoutingEvaluateResponse {
    pub status: String,
    pub output: serde_json::Value,
    pub evaluated_output: Vec<String>,
    pub eligible_connectors: Vec<String>,
}

use crate::storage::schema::routing_algorithm_mapper;
#[derive(AsChangeset, Debug, serde::Serialize, serde::Deserialize)]
#[diesel(table_name = routing_algorithm_mapper)]
pub struct ActivateRoutingRule {
    pub created_by: String,
    pub routing_algorithm_id: String,
}

#[derive(AsChangeset, Debug, serde::Serialize, serde::Deserialize)]
#[diesel(table_name = routing_algorithm_mapper)]
pub struct RoutingAlgorithmMapperUpdate {
    pub routing_algorithm_id: String,
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
