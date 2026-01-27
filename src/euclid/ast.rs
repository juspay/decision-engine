use super::types::{DataType, Metadata};
use serde::{Deserialize, Serialize};

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

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
    /// Represents a global reference, which is a reference to a global variable
    GlobalRef(String),
    /// Represents an array of numbers. This is basically used for
    /// "one of the given numbers" operations
    /// eg: payment.method.amount = (1, 2, 3)
    NumberArray(Vec<u64>),
    /// Similar to NumberArray but for enum variants
    /// eg: payment.method.cardtype = (debit, credit)
    EnumVariantArray(Vec<String>),
    /// Like a number array but can include comparisons. Useful for
    /// conditions like "500 < amount < 1000"
    /// eg: payment.amount = (> 500, < 1000)
    NumberComparisonArray(Vec<NumberComparison>),
}

impl ValueType {
    pub fn is_metadata(&self) -> bool {
        matches!(self, Self::MetadataVariant(_))
    }

    pub fn get_type(&self) -> DataType {
        match self {
            Self::Number(_) => DataType::Number,
            Self::StrValue(_) => DataType::StrValue,
            Self::MetadataVariant(_) => DataType::MetadataValue,
            Self::EnumVariant(_) => DataType::EnumVariant,
            Self::GlobalRef(_) => DataType::GlobalRef,
            Self::NumberComparisonArray(_) => DataType::Number,
            Self::NumberArray(_) => DataType::Number,
            Self::EnumVariantArray(_) => DataType::EnumVariant,
        }
    }
}

/// Represents a number comparison for "NumberComparisonArrayValue"
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct VolumeSplit<T> {
    pub split: u8,
    pub output: T,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Output {
    Single(ConnectorInfo),
    Priority(Vec<ConnectorInfo>),
    VolumeSplit(Vec<VolumeSplit<ConnectorInfo>>),
    VolumeSplitPriority(Vec<VolumeSplit<Vec<ConnectorInfo>>>),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ConnectorInfo {
    pub gateway_name: String,
    pub gateway_id: Option<String>,
}

impl fmt::Display for ConnectorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.gateway_id {
            Some(id) => write!(f, "{} ({})", self.gateway_name, id),
            None => write!(f, "{}", self.gateway_name),
        }
    }
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
