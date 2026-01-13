use super::ast::{Comparison, ComparisonType, IfStatement, Rule, ValueType};
use super::errors::{EuclidErrors, ValidationErrorDetails};
use super::types::{KeyDataType, StaticRoutingAlgorithm};
use crate::error::ContainerError;
use crate::euclid::types::{FieldValidationRules, KeyConfig, RoutingRule, TomlConfig};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationErrorDetails>,
    pub error_summary: Option<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
            error_summary: None,
        }
    }

    pub fn failure(errors: Vec<ValidationErrorDetails>) -> Self {
        let summary = if errors.is_empty() {
            None
        } else {
            Some(
                errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        };
        Self {
            is_valid: false,
            errors,
            error_summary: summary,
        }
    }

    pub fn to_error_message(&self) -> String {
        self.error_summary
            .clone()
            .unwrap_or_else(|| "Validation failed".to_string())
    }
}

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
        if key_config.data_type == KeyDataType::Enum {
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
        if key_config.data_type == KeyDataType::Enum {
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
    result.insert("str_value".to_string(), Vec::new());
    for (key, key_config) in &config.keys.keys {
        let type_str = key_config.data_type.as_str().to_string();
        if let Some(keys) = result.get_mut(&type_str) {
            keys.push(key.clone());
        }
    }
    result
}

pub fn validate_routing_rule(
    rule: &RoutingRule,
    config: &Option<TomlConfig>,
) -> Result<ValidationResult, ContainerError<EuclidErrors>> {
    let config = config
        .clone()
        .ok_or_else(|| error_stack::report!(EuclidErrors::GlobalRoutingConfigsUnavailable))?;

    match &rule.algorithm {
        StaticRoutingAlgorithm::Single(_)
        | StaticRoutingAlgorithm::Priority(_)
        | StaticRoutingAlgorithm::VolumeSplit(_) => {
            crate::logger::debug!("Validation skipped for non-advanced algorithm types");
            Ok(ValidationResult::success())
        }
        StaticRoutingAlgorithm::Advanced(program) => {
            let mut validation_errors: Vec<ValidationErrorDetails> = Vec::new();

            for rule in &program.rules {
                validate_rule(rule, &config, &mut validation_errors);
            }

            if validation_errors.is_empty() {
                crate::logger::debug!("Routing rule validation passed successfully");
                Ok(ValidationResult::success())
            } else {
                for error in &validation_errors {
                    crate::logger::warn!(
                        field = %error.field,
                        error_type = %error.error_type,
                        message = %error.message,
                        expected = ?error.expected,
                        actual = ?error.actual,
                        "Field validation error"
                    );
                }

                let result = ValidationResult::failure(validation_errors);
                crate::logger::error!(
                    error_count = result.errors.len(),
                    summary = ?result.error_summary,
                    "Routing rule validation failed"
                );
                Ok(result)
            }
        }
    }
}

fn validate_rule(rule: &Rule, config: &TomlConfig, errors: &mut Vec<ValidationErrorDetails>) {
    for statement in &rule.statements {
        validate_statement(statement, config, errors);
    }
}

fn validate_statement(
    statement: &IfStatement,
    config: &TomlConfig,
    errors: &mut Vec<ValidationErrorDetails>,
) {
    for condition in &statement.condition {
        validate_condition(condition, config, errors);
    }

    if let Some(nested) = &statement.nested {
        for nested_stmt in nested {
            validate_statement(nested_stmt, config, errors);
        }
    }
}

fn validate_condition(
    condition: &Comparison,
    config: &TomlConfig,
    errors: &mut Vec<ValidationErrorDetails>,
) {
    let key_exists = config.keys.keys.contains_key(&condition.lhs);
    if !key_exists {
        errors.push(ValidationErrorDetails::new(
            &condition.lhs,
            "unknown_key",
            format!(
                "Invalid field '{}' (unknown_key) - expected a defined key, got undefined key",
                &condition.lhs
            ),
        ));
        return;
    }

    let key_config = &config.keys.keys[&condition.lhs];

    match (&key_config.data_type, &condition.comparison) {
        (
            KeyDataType::Integer,
            ComparisonType::Equal
            | ComparisonType::NotEqual
            | ComparisonType::LessThan
            | ComparisonType::LessThanEqual
            | ComparisonType::GreaterThan
            | ComparisonType::GreaterThanEqual,
        ) => {}
        (KeyDataType::Enum, ComparisonType::Equal | ComparisonType::NotEqual) => {}
        (KeyDataType::Enum, _) => {
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "invalid_comparison",
                format!(
                    "Invalid field '{}' (invalid_comparison) - expected Equal or NotEqual, got {:?}",
                    &condition.lhs, condition.comparison
                ),
                "Equal or NotEqual",
                format!("{:?}", condition.comparison),
            ));
        }
        (_, comp) if comp != &ComparisonType::Equal && comp != &ComparisonType::NotEqual => {
            errors.push(ValidationErrorDetails::new(
                &condition.lhs,
                "comparison_warning",
                format!(
                    "Comparison type '{:?}' may not be appropriate for key '{}' of type '{:?}'",
                    condition.comparison, condition.lhs, key_config.data_type
                ),
            ));
        }
        _ => {}
    }

    match (&key_config.data_type, &condition.value) {
        (KeyDataType::Enum, ValueType::EnumVariant(value)) => {
            if !is_valid_enum_value(config, &condition.lhs, value) {
                let valid_values = parse_enum_values(key_config);
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "invalid_enum_value",
                    format!(
                        "Invalid field '{}' (invalid_enum_value) - expected one of {:?}, got '{}'",
                        &condition.lhs, valid_values, value
                    ),
                    format!("one of {:?}", valid_values),
                    value.to_string(),
                ));
            }
        }
        (KeyDataType::Enum, ValueType::EnumVariantArray(arr)) => {
            let invalid: Vec<_> = arr
                .iter()
                .filter(|v| !is_valid_enum_value(config, &condition.lhs, *v))
                .cloned()
                .collect();
            if !invalid.is_empty() {
                let valid_values = parse_enum_values(key_config);
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "invalid_enum_values",
                    format!(
                        "Invalid field '{}' (invalid_enum_values) - expected values from {:?}, got {:?}",
                        &condition.lhs, valid_values, invalid
                    ),
                    format!("values from {:?}", valid_values),
                    format!("{:?}", invalid),
                ));
            }
        }
        (KeyDataType::Enum, _) => {
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "type_mismatch",
                format!(
                    "Invalid field '{}' (type_mismatch) - expected enum variant, got {:?}",
                    &condition.lhs,
                    condition.value.get_type()
                ),
                "enum variant",
                format!("{:?}", condition.value.get_type()),
            ));
        }

        (KeyDataType::Integer, ValueType::Number(n)) => {
            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    if let Err(e) = validate_numeric_range(&condition.lhs, *n as i64, &rules) {
                        let mut expected_parts = Vec::new();
                        if let Some(min) = rules.numeric_min {
                            expected_parts.push(format!("min: {}", min));
                        }
                        if let Some(max) = rules.numeric_max {
                            expected_parts.push(format!("max: {}", max));
                        }
                        let expected = if expected_parts.is_empty() {
                            "valid value".to_string()
                        } else {
                            expected_parts.join(", ")
                        };
                        errors.push(ValidationErrorDetails::with_expected_actual(
                            &condition.lhs,
                            "value_out_of_range",
                            e,
                            expected,
                            n.to_string(),
                        ));
                    }
                }
            }
        }
        (KeyDataType::Integer, ValueType::NumberArray(arr)) => {
            if !matches!(
                condition.comparison,
                ComparisonType::Equal | ComparisonType::NotEqual
            ) {
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "invalid_comparison",
                    format!(
                        "Only '==' or '!=' allowed with number arrays for key '{}'",
                        condition.lhs
                    ),
                    "Equal or NotEqual",
                    format!("{:?}", condition.comparison),
                ));
            }

            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    for (i, n) in arr.iter().enumerate() {
                        if let Err(e) = validate_numeric_range(&condition.lhs, *n as i64, &rules) {
                            let mut expected_parts = Vec::new();
                            if let Some(min) = rules.numeric_min {
                                expected_parts.push(format!("min: {}", min));
                            }
                            if let Some(max) = rules.numeric_max {
                                expected_parts.push(format!("max: {}", max));
                            }
                            let expected = if expected_parts.is_empty() {
                                "valid value".to_string()
                            } else {
                                expected_parts.join(", ")
                            };
                            errors.push(ValidationErrorDetails::with_expected_actual(
                                &condition.lhs,
                                "value_out_of_range",
                                format!("Element {}: {}", i + 1, e),
                                expected,
                                n.to_string(),
                            ));
                        }
                    }
                }
            }
        }
        (KeyDataType::Integer, ValueType::NumberComparisonArray(_)) => {
            if condition.comparison != ComparisonType::Equal {
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "invalid_comparison",
                    format!(
                        "Only '==' allowed with number comparison arrays for key '{}'",
                        condition.lhs
                    ),
                    "Equal",
                    format!("{:?}", condition.comparison),
                ));
            }
        }
        (KeyDataType::Integer, _) => {
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "type_mismatch",
                format!(
                    "Invalid field '{}' (type_mismatch) - expected number, got {:?}",
                    &condition.lhs,
                    condition.value.get_type()
                ),
                "number",
                format!("{:?}", condition.value.get_type()),
            ));
        }

        (KeyDataType::Udf, ValueType::MetadataVariant(m)) => {
            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    if let Err(e) = validate_string_value(&condition.lhs, &m.value, &rules) {
                        let expected = build_expected_constraint_string(&rules);
                        errors.push(ValidationErrorDetails::with_expected_actual(
                            &condition.lhs,
                            "length_invalid",
                            e,
                            expected,
                            format!("\"{}\" ({} chars)", m.value, m.value.len()),
                        ));
                    }
                }
            }
        }
        (KeyDataType::Udf, _) => {
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "type_mismatch",
                format!(
                    "Invalid field '{}' (type_mismatch) - expected metadata variant, got {:?}",
                    &condition.lhs,
                    condition.value.get_type()
                ),
                "metadata variant",
                format!("{:?}", condition.value.get_type()),
            ));
        }

        (KeyDataType::StrValue, ValueType::StrValue(s)) => {
            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    if let Err(e) = validate_string_value(&condition.lhs, s, &rules) {
                        let expected = build_expected_constraint_string(&rules);
                        errors.push(ValidationErrorDetails::with_expected_actual(
                            &condition.lhs,
                            "length_invalid",
                            e,
                            expected,
                            format!("\"{}\" ({} chars)", s, s.len()),
                        ));
                    }
                }
            }
        }

        _ => {
            if condition.value.get_type().to_string() != key_config.data_type.as_str() {
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "type_mismatch",
                    format!(
                        "Invalid field '{}' (type_mismatch) - expected {}, got {}",
                        &condition.lhs,
                        key_config.data_type.as_str(),
                        condition.value.get_type()
                    ),
                    key_config.data_type.as_str().to_string(),
                    condition.value.get_type().to_string(),
                ));
            }
        }
    }
}

fn build_expected_constraint_string(rules: &FieldValidationRules) -> String {
    let mut parts = Vec::new();

    if let Some(exact) = rules.exact_length {
        parts.push(format!("exactly {} characters", exact));
    } else {
        let mut length_parts = Vec::new();
        if let Some(min) = rules.length_min {
            length_parts.push(format!("min: {}", min));
        }
        if let Some(max) = rules.length_max {
            length_parts.push(format!("max: {}", max));
        }
        if !length_parts.is_empty() {
            parts.push(format!("{} characters", length_parts.join(", ")));
        }
    }

    let mut numeric_parts = Vec::new();
    if let Some(min) = rules.numeric_min {
        numeric_parts.push(format!("min: {}", min));
    }
    if let Some(max) = rules.numeric_max {
        numeric_parts.push(format!("max: {}", max));
    }
    if !numeric_parts.is_empty() {
        parts.push(format!("value {}", numeric_parts.join(", ")));
    }

    if rules.regex_pattern.is_some() {
        parts.push("matching pattern".to_string());
    }

    if parts.is_empty() {
        "valid value".to_string()
    } else {
        parts.join(", ")
    }
}

pub fn validate_numeric_range(
    field: &str,
    value: i64,
    rules: &FieldValidationRules,
) -> Result<(), String> {
    if let Some(min) = rules.numeric_min {
        if value < min {
            return Err(format!(
                "Invalid field '{}': value {} is below minimum {}",
                field, value, min
            ));
        }
    }
    if let Some(max) = rules.numeric_max {
        if value > max {
            return Err(format!(
                "Invalid field '{}': value {} exceeds maximum {}",
                field, value, max
            ));
        }
    }
    Ok(())
}

pub fn validate_string_length(
    field: &str,
    value: &str,
    min_length: Option<usize>,
    max_length: Option<usize>,
) -> Result<(), String> {
    let len = value.len();

    if let Some(min) = min_length {
        if len < min {
            return Err(format!(
                "Invalid field '{}': length {} is below minimum {}",
                field, len, min
            ));
        }
    }

    if let Some(max) = max_length {
        if len > max {
            return Err(format!(
                "Invalid field '{}': length {} exceeds maximum {}",
                field, len, max
            ));
        }
    }

    Ok(())
}

pub fn validate_exact_length(
    field: &str,
    value: &str,
    expected_length: usize,
) -> Result<(), String> {
    let actual_length = value.len();
    if actual_length != expected_length {
        return Err(format!(
            "Invalid field '{}': expected {} characters, got {} ({} characters)",
            field, expected_length, value, actual_length
        ));
    }
    Ok(())
}

pub fn validate_regex_pattern(
    field: &str,
    value: &str,
    pattern: &Option<regex::Regex>,
) -> Result<(), String> {
    if let Some(ref regex) = pattern {
        if !regex.is_match(value) {
            return Err(format!(
                "Invalid field '{}': value does not match required pattern",
                field
            ));
        }
    }
    Ok(())
}

pub fn build_validation_rules(key_config: &KeyConfig) -> Result<FieldValidationRules, String> {
    key_config.build_validation_rules()
}

pub fn validate_string_value(
    field: &str,
    value: &str,
    rules: &FieldValidationRules,
) -> Result<(), String> {
    let mut errors = Vec::new();

    if let Some(exact) = rules.exact_length {
        if let Err(e) = validate_exact_length(field, value, exact) {
            errors.push(e);
        }
    } else if rules.length_min.is_some() || rules.length_max.is_some() {
        if let Err(e) = validate_string_length(field, value, rules.length_min, rules.length_max) {
            errors.push(e);
        }
    }

    if let Err(e) = validate_regex_pattern(field, value, &rules.regex_pattern) {
        errors.push(e);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
