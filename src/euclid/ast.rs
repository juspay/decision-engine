use super::types::{DataType, Metadata};
use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetadataValue {
    pub key: String,
    pub value: String,
}

/// Represents a value in the DSL
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ValueType {
    /// Represents a number literal
    Number(u64),
    /// Represents an enum variant
    EnumVariant(String),
    /// Represents a Metadata variant
    MetadataVariant(MetadataValue),
    /// Represents a arbitrary String value
    StrValue(String),
    GlobalRef(String),
}

impl ValueType {
    pub fn get_type(&self) -> DataType {
        match self {
            Self::Number(_) => DataType::Number,
            Self::StrValue(_) => DataType::StrValue,
            Self::MetadataVariant(_) => DataType::MetadataValue,
            Self::EnumVariant(_) => DataType::EnumVariant,
            Self::GlobalRef(_) => DataType::GlobalRef,
        }
    }
}

/// Represents a number comparison for "NumberComparisonArrayValue"
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NumberComparison {
    pub comparison_type: ComparisonType,
    pub number: u64,
}

/// Conditional comparison type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonType {
    Equal,
    NotEqual,
    LessThan,
    LessThanEqual,
    GreaterThan,
    GreaterThanEqual,
}

/// Represents a single comparison condition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Comparison {
    /// The left hand side which will always be a domain input identifier like "payment.method.cardtype"
    pub lhs: String,
    /// The comparison operator
    pub comparison: ComparisonType,
    /// The value to compare against
    pub value: ValueType,
    /// Additional metadata that the Static Analyzer and Backend does not touch.
    /// This can be used to store useful information for the frontend and is required for communication
    /// between the static analyzer and the frontend.
    // #[schema(value_type=HashMap<String, serde_json::Value>)]
    pub metadata: Metadata,
}

/// Represents all the conditions of an IF statement
/// eg:
///
/// ```text
/// payment.method = card & payment.method.cardtype = debit & payment.method.network = diners
/// ```
pub type IfCondition = Vec<Comparison>;

/// Represents an IF statement with conditions and optional nested IF statements
///
/// ```text
/// payment.method = card {
///     payment.method.cardtype = (credit, debit) {
///         payment.method.network = (amex, rupay, diners)
///     }
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IfStatement {
    // #[schema(value_type=Vec<Comparison>)]
    pub condition: IfCondition,
    pub nested: Option<Vec<IfStatement>>,
}

/// Represents a rule
///
/// ```text
/// rule_name: [stripe, adyen, checkout]
/// {
///     payment.method = card {
///         payment.method.cardtype = (credit, debit) {
///             payment.method.network = (amex, rupay, diners)
///         }
///
///         payment.method.cardtype = credit
///     }
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
// #[aliases(RuleConnectorSelection = Rule<ConnectorSelection>)]
pub struct Rule {
    pub name: String,
    #[serde(alias = "routingType")]
    pub routing_type: RoutingType,
    #[serde(alias = "routingOutput")]
    pub output: Output,
    pub statements: Vec<IfStatement>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingType {
    Priority,
    VolumeSplit,
    VolumeSplitPriority,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct VolumeSplit<T> {
    pub split: u8,
    pub output: T,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    Priority(Vec<ConnectorInfo>),
    VolumeSplit(Vec<VolumeSplit<ConnectorInfo>>),
    VolumeSplitPriority(Vec<VolumeSplit<Vec<ConnectorInfo>>>),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConnectorInfo {
    pub gateway_name: String,
    pub gateway_id: Option<String>,
}

pub type Globals = HashMap<String, HashSet<ValueType>>;

/// The program, having a default connector selection and
/// a bunch of rules. Also can hold arbitrary metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
// #[aliases(ProgramConnectorSelection = Program<ConnectorSelection>)]
pub struct Program {
    pub globals: Globals,
    pub default_selection: Output,
    // #[schema(value_type=RuleConnectorSelection)]
    pub rules: Vec<Rule>,
    // #[schema(value_type=HashMap<String, serde_json::Value>)]
    pub metadata: Option<Metadata>,
}
