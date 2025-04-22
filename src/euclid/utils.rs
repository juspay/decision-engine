use super::ast::{Comparison, ComparisonType, IfStatement, Rule, ValueType};
use super::errors::EuclidErrors;
use crate::euclid::types::{KeyConfig, TomlConfig, RoutingRule};
use crate::error::{ApiError, ContainerError};
use std::collections::HashMap;
use uuid::Uuid;

pub fn generate_random_id(prefix: &str) -> String {
    let uuid = Uuid::new_v4();
    format!("{}_{}", prefix, uuid)
}

/// Helper function to parse enum values from a KeyConfig
pub fn parse_enum_values(key_config: &KeyConfig) -> Vec<String> {
    if let Some(values_str) = &key_config.values {
        values_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        Vec::new()
    }
}

/// Helper function to get all enum keys and their possible values from TomlConfig
pub fn get_all_enum_definitions(config: &TomlConfig) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    for (key, key_config) in &config.keys.keys {
        if key_config.data_type == "enum" {
            let values = parse_enum_values(key_config);
            if !values.is_empty() {
                result.insert(key.clone(), values);
            }
        }
    }
    result
}

/// Helper function to check if a value is valid for a given enum key
pub fn is_valid_enum_value(config: &TomlConfig, key: &str, value: &str) -> bool {
    if let Some(key_config) = config.keys.keys.get(key) {
        if key_config.data_type == "enum" {
            let valid_values = parse_enum_values(key_config);
            return valid_values.contains(&value.to_string());
        }
    }
    false
}

/// Helper function to get all defined keys by their data types
pub fn get_keys_by_type(config: &TomlConfig) -> HashMap<String, Vec<String>> {
    let mut result = HashMap::new();
    result.insert("enum".to_string(), Vec::new());
    result.insert("integer".to_string(), Vec::new());
    result.insert("udf".to_string(), Vec::new());
    result.insert("string".to_string(), Vec::new());
    for (key, key_config) in &config.keys.keys {
        if let Some(keys) = result.get_mut(&key_config.data_type) {
            keys.push(key.clone());
        }
    }
    result
}

pub fn validate_routing_rule(
    rule: &RoutingRule,
    config: &Option<TomlConfig>,
) -> Result<(), ContainerError<EuclidErrors>> {
    let config = config
        .clone()
        .ok_or_else(|| error_stack::report!(EuclidErrors::GlobalRoutingConfigsUnavailable))?;

    let mut errors = Vec::new();

    for rule in &rule.algorithm.rules {
        validate_rule(rule, &config, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(EuclidErrors::FailedToValidateRoutingRule.into())
    }
}

fn validate_rule(rule: &Rule, config: &TomlConfig, errors: &mut Vec<String>) {
    for (i, statement) in rule.statements.iter().enumerate() {
        validate_statement(
            statement,
            config,
            errors,
            &format!("Rule '{}' Statement {}", rule.name, i + 1),
        );
    }
}

fn validate_statement(
    statement: &IfStatement,
    config: &TomlConfig,
    errors: &mut Vec<String>,
    context: &str,
) {
    for condition in &statement.condition {
        validate_condition(condition, config, errors, context);
    }
}

fn validate_condition(
    condition: &Comparison,
    config: &TomlConfig,
    errors: &mut Vec<String>,
    context: &str,
) {
    let key_exists = config.keys.keys.contains_key(&condition.lhs);
    if !key_exists {
        errors.push(format!(
            "{}: Unknown key '{}' in condition",
            context, condition.lhs
        ));
        return;
    }
    let key_config = &config.keys.keys[&condition.lhs];

    match (key_config.data_type.as_str(), &condition.comparison) {
        (
            "integer",
            ComparisonType::Equal
            | ComparisonType::NotEqual
            | ComparisonType::LessThan
            | ComparisonType::LessThanEqual
            | ComparisonType::GreaterThan
            | ComparisonType::GreaterThanEqual,
        ) => {}
        ("enum", ComparisonType::Equal | ComparisonType::NotEqual) => {}
        ("enum", _) => {
            errors.push(format!(
                "{}: Invalid comparison type '{:?}' for enum key '{}'",
                context, condition.comparison, condition.lhs
            ));
        }
        (_, comp) if comp != &ComparisonType::Equal && comp != &ComparisonType::NotEqual => {
            errors.push(format!(
                "{}: Comparison type '{:?}' may not be appropriate for key '{}' of type '{}'",
                context, condition.comparison, condition.lhs, key_config.data_type
            ));
        }
        _ => {}
    }

    match (key_config.data_type.as_str(), &condition.value) {
        ("enum", ValueType::EnumVariant(value)) => {
            if !is_valid_enum_value(config, &condition.lhs, value) {
                let valid_values = parse_enum_values(key_config);
                errors.push(format!(
                    "{}: Invalid enum value '{}' for key '{}'. Valid values are: {:?}",
                    context, value, condition.lhs, valid_values
                ));
            }
        }
        ("enum", _) => {
            errors.push(format!(
                "{}: Key '{}' is of type 'enum' but value is not an enum variant",
                context, condition.lhs
            ));
        }
        ("integer", ValueType::Number(_)) => {
            // Number value is valid for integer type
        }
        ("integer", _) => {
            errors.push(format!(
                "{}: Key '{}' is of type 'integer' but value is not a number",
                context, condition.lhs
            ));
        }
        ("udf", ValueType::MetadataVariant(_)) => {
            // Metadata value is valid for udf type
        }
        ("udf", _) => {
            errors.push(format!(
                "{}: Key '{}' is of type 'udf' but value is not a metadata variant",
                context, condition.lhs
            ));
        }
        _ => {
            if condition.value.get_type().to_string() != key_config.data_type {
                errors.push(format!(
                    "{}: Value type mismatch for key '{}': expected '{}' but got '{}'",
                    context,
                    condition.lhs,
                    key_config.data_type,
                    condition.value.get_type()
                ));
            }
        }
    }
}
