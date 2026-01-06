use super::ast::{Comparison, ComparisonType, IfStatement, Rule, ValueType};
use super::errors::{format_validation_error, EuclidErrors, ValidationErrorDetails};
use super::types::StaticRoutingAlgorithm;
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

pub fn validate_routing_rule_with_details(
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
                validate_rule_with_details(rule, &config, &mut validation_errors);
            }

            if validation_errors.is_empty() {
                crate::logger::debug!("Routing rule validation passed successfully");
                Ok(ValidationResult::success())
            } else {
                // Log each validation error with details
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

/// Original validation function - maintained for backward compatibility
pub fn validate_routing_rule(
    rule: &RoutingRule,
    config: &Option<TomlConfig>,
) -> Result<(), ContainerError<EuclidErrors>> {
    let result = validate_routing_rule_with_details(rule, config)?;

    if result.is_valid {
        Ok(())
    } else {
        Err(EuclidErrors::InvalidRequest(format!(
            "Routing rule validation failed: {}",
            result.to_error_message()
        ))
        .into())
    }
}

/// Returns validation errors as a list of structured error details
pub fn get_validation_errors(
    rule: &RoutingRule,
    config: &Option<TomlConfig>,
) -> Result<Vec<ValidationErrorDetails>, ContainerError<EuclidErrors>> {
    let result = validate_routing_rule_with_details(rule, config)?;
    Ok(result.errors)
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

/// Validates a rule and collects structured validation error details
fn validate_rule_with_details(
    rule: &Rule,
    config: &TomlConfig,
    errors: &mut Vec<ValidationErrorDetails>,
) {
    for (i, statement) in rule.statements.iter().enumerate() {
        let context = format!("Rule '{}'", rule.name);
        validate_statement_with_details(statement, config, errors, &context);
    }
}

/// Validates a statement and collects structured validation error details
fn validate_statement_with_details(
    statement: &IfStatement,
    config: &TomlConfig,
    errors: &mut Vec<ValidationErrorDetails>,
    context: &str,
) {
    for condition in &statement.condition {
        validate_condition_with_details(condition, config, errors, context);
    }

    // Recursively validate nested statements
    if let Some(nested) = &statement.nested {
        for (i, nested_stmt) in nested.iter().enumerate() {
            let nested_context = format!("{} -> Nested {}", context, i + 1);
            validate_statement_with_details(nested_stmt, config, errors, &nested_context);
        }
    }
}

/// Validates a condition and collects structured validation error details
fn validate_condition_with_details(
    condition: &Comparison,
    config: &TomlConfig,
    errors: &mut Vec<ValidationErrorDetails>,
    context: &str,
) {
    let key_exists = config.keys.keys.contains_key(&condition.lhs);
    if !key_exists {
        errors.push(ValidationErrorDetails::new(
            &condition.lhs,
            "unknown_key",
            format_validation_error(
                context,
                &condition.lhs,
                "unknown_key",
                "a defined key",
                "undefined key",
            ),
        ));
        return;
    }

    let key_config = &config.keys.keys[&condition.lhs];

    // Validate comparison type
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
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "invalid_comparison",
                format_validation_error(
                    context,
                    &condition.lhs,
                    "invalid_comparison",
                    "Equal or NotEqual",
                    &format!("{:?}", condition.comparison),
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
                    "{}: Comparison type '{:?}' may not be appropriate for key '{}' of type '{}'",
                    context, condition.comparison, condition.lhs, key_config.data_type
                ),
            ));
        }
        _ => {}
    }

    // Validate value type and constraints
    match (key_config.data_type.as_str(), &condition.value) {
        ("enum", ValueType::EnumVariant(value)) => {
            if !is_valid_enum_value(config, &condition.lhs, value) {
                let valid_values = parse_enum_values(key_config);
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "invalid_enum_value",
                    format_validation_error(
                        context,
                        &condition.lhs,
                        "invalid_enum_value",
                        &format!("one of {:?}", valid_values),
                        value,
                    ),
                    format!("one of {:?}", valid_values),
                    value.to_string(),
                ));
            }
        }
        ("enum", ValueType::EnumVariantArray(arr)) => {
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
                    format_validation_error(
                        context,
                        &condition.lhs,
                        "invalid_enum_values",
                        &format!("values from {:?}", valid_values),
                        &format!("{:?}", invalid),
                    ),
                    format!("values from {:?}", valid_values),
                    format!("{:?}", invalid),
                ));
            }
        }
        ("enum", _) => {
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "type_mismatch",
                format_validation_error(
                    context,
                    &condition.lhs,
                    "type_mismatch",
                    "enum variant",
                    &format!("{:?}", condition.value.get_type()),
                ),
                "enum variant",
                format!("{:?}", condition.value.get_type()),
            ));
        }

        ("integer", ValueType::Number(n)) => {
            // Validate numeric value against constraints
            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    if let Err(e) = validate_numeric_range(&condition.lhs, *n as i64, &rules) {
                        if let Some((min, max)) = rules.numeric_range {
                            errors.push(ValidationErrorDetails::with_expected_actual(
                                &condition.lhs,
                                "value_out_of_range",
                                format!("{}: {}", context, e),
                                format!("value between {} and {}", min, max),
                                n.to_string(),
                            ));
                        }
                    }
                }
            }
        }
        ("integer", ValueType::NumberArray(arr)) => {
            if !matches!(
                condition.comparison,
                ComparisonType::Equal | ComparisonType::NotEqual
            ) {
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "invalid_comparison",
                    format!(
                        "{}: Only '==' or '!=' allowed with number arrays for key '{}'",
                        context, condition.lhs
                    ),
                    "Equal or NotEqual",
                    format!("{:?}", condition.comparison),
                ));
            }

            // Validate each number in array against constraints
            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    for (i, n) in arr.iter().enumerate() {
                        if let Err(e) = validate_numeric_range(&condition.lhs, *n as i64, &rules) {
                            if let Some((min, max)) = rules.numeric_range {
                                errors.push(ValidationErrorDetails::with_expected_actual(
                                    &condition.lhs,
                                    "value_out_of_range",
                                    format!("{}: Element {}: {}", context, i + 1, e),
                                    format!("value between {} and {}", min, max),
                                    n.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        ("integer", ValueType::NumberComparisonArray(_)) => {
            if condition.comparison != ComparisonType::Equal {
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "invalid_comparison",
                    format!(
                        "{}: Only '==' allowed with number comparison arrays for key '{}'",
                        context, condition.lhs
                    ),
                    "Equal",
                    format!("{:?}", condition.comparison),
                ));
            }
        }
        ("integer", _) => {
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "type_mismatch",
                format_validation_error(
                    context,
                    &condition.lhs,
                    "type_mismatch",
                    "number",
                    &format!("{:?}", condition.value.get_type()),
                ),
                "number",
                format!("{:?}", condition.value.get_type()),
            ));
        }

        ("udf", ValueType::MetadataVariant(m)) => {
            // Validate metadata value against constraints
            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    if let Err(e) = validate_string_value(&condition.lhs, &m.value, &rules) {
                        let expected = build_expected_constraint_string(&rules);
                        errors.push(ValidationErrorDetails::with_expected_actual(
                            &condition.lhs,
                            "length_invalid",
                            format!("{}: {}", context, e),
                            expected,
                            format!("\"{}\" ({} chars)", m.value, m.value.len()),
                        ));
                    }
                }
            }
        }
        ("udf", _) => {
            errors.push(ValidationErrorDetails::with_expected_actual(
                &condition.lhs,
                "type_mismatch",
                format_validation_error(
                    context,
                    &condition.lhs,
                    "type_mismatch",
                    "metadata variant",
                    &format!("{:?}", condition.value.get_type()),
                ),
                "metadata variant",
                format!("{:?}", condition.value.get_type()),
            ));
        }

        ("str_value", ValueType::StrValue(s)) => {
            // Validate string value against constraints
            if key_config.has_validation_constraints() {
                if let Ok(rules) = build_validation_rules(key_config) {
                    if let Err(e) = validate_string_value(&condition.lhs, s, &rules) {
                        let expected = build_expected_constraint_string(&rules);
                        errors.push(ValidationErrorDetails::with_expected_actual(
                            &condition.lhs,
                            "length_invalid",
                            format!("{}: {}", context, e),
                            expected,
                            format!("\"{}\" ({} chars)", s, s.len()),
                        ));
                    }
                }
            }
        }

        _ => {
            if condition.value.get_type().to_string() != key_config.data_type {
                errors.push(ValidationErrorDetails::with_expected_actual(
                    &condition.lhs,
                    "type_mismatch",
                    format_validation_error(
                        context,
                        &condition.lhs,
                        "type_mismatch",
                        &key_config.data_type,
                        &condition.value.get_type().to_string(),
                    ),
                    key_config.data_type.clone(),
                    condition.value.get_type().to_string(),
                ));
            }
        }
    }
}

/// Builds a human-readable string describing the expected constraints
fn build_expected_constraint_string(rules: &FieldValidationRules) -> String {
    let mut parts = Vec::new();

    if let Some(exact) = rules.exact_length {
        parts.push(format!("exactly {} characters", exact));
    } else if let Some((min, max)) = rules.length_range {
        parts.push(format!("{}-{} characters", min, max));
    }

    if let Some((min, max)) = rules.numeric_range {
        parts.push(format!("value between {} and {}", min, max));
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

/// validates the comparison operators for different subtle value types present
/// by throwing required errors for comparisons that can't be performed for a certain value type
/// for example
/// can't have greater/less than operations on enum types
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
        ("enum", ValueType::EnumVariantArray(arr)) => {
            let invalid: Vec<_> = arr
                .iter()
                .filter(|v| !is_valid_enum_value(config, &condition.lhs, *v))
                .cloned()
                .collect();
            if !invalid.is_empty() {
                let valid_values = parse_enum_values(key_config);
                errors.push(format!(
                    "{}: Invalid enum values {:?} for key '{}'. Valid values are: {:?}",
                    context, invalid, condition.lhs, valid_values
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
        // array of literals – only == / != make sense
        ("integer", ValueType::NumberArray(_)) => {
            if !matches!(
                condition.comparison,
                ComparisonType::Equal | ComparisonType::NotEqual
            ) {
                errors.push(format!(
                    "{context}: Only '==' or '!=' allowed with number arrays for key '{}'",
                    condition.lhs
                ));
            }
        }
        // comparison array – interpreter supports **only `==`**
        ("integer", ValueType::NumberComparisonArray(_)) => {
            if condition.comparison != ComparisonType::Equal {
                errors.push(format!(
                    "{context}: Only '==' allowed with number comparison arrays for key '{}'",
                    condition.lhs
                ));
            }
        }

        ("integer", _) => {
            errors.push(format!(
                "{}: Key '{}' is of type 'integer' but value is not a number",
                context, condition.lhs
            ));
        }

        ("udf", ValueType::MetadataVariant(m)) => {
            // Metadata value is valid for udf type
            // Validate the metadata value against constraints
            if key_config.has_validation_constraints() {
                match build_validation_rules(key_config) {
                    Ok(rules) => {
                        if let Err(e) = validate_string_value(&condition.lhs, &m.value, &rules) {
                            errors.push(format!("{}: {}", context, e));
                        }
                    }
                    Err(e) => {
                        errors.push(format!(
                            "{}: Failed to build validation rules: {}",
                            context, e
                        ));
                    }
                }
            }
        }
        ("udf", _) => {
            errors.push(format!(
                "{}: Key '{}' is of type 'udf' but value is not a metadata variant",
                context, condition.lhs
            ));
        }
        ("str_value", ValueType::StrValue(s)) => {
            // Validate string value against constraints
            if key_config.has_validation_constraints() {
                match build_validation_rules(key_config) {
                    Ok(rules) => {
                        if let Err(e) = validate_string_value(&condition.lhs, s, &rules) {
                            errors.push(format!("{}: {}", context, e));
                        }
                    }
                    Err(e) => {
                        errors.push(format!(
                            "{}: Failed to build validation rules: {}",
                            context, e
                        ));
                    }
                }
            }
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

    // Additional value-level validation for integer types
    if key_config.data_type == "integer" && key_config.has_validation_constraints() {
        match build_validation_rules(key_config) {
            Ok(rules) => {
                match &condition.value {
                    ValueType::Number(n) => {
                        // Cast u64 to i64 for validation (safe for typical positive values)
                        if let Err(e) = validate_numeric_range(&condition.lhs, *n as i64, &rules) {
                            errors.push(format!("{}: {}", context, e));
                        }
                    }
                    ValueType::NumberArray(arr) => {
                        for (i, n) in arr.iter().enumerate() {
                            // Cast u64 to i64 for validation
                            if let Err(e) =
                                validate_numeric_range(&condition.lhs, *n as i64, &rules)
                            {
                                errors.push(format!("{}: Element {}: {}", context, i + 1, e));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                errors.push(format!(
                    "{}: Failed to build validation rules: {}",
                    context, e
                ));
            }
        }
    }
}

pub fn validate_numeric_range(
    field: &str,
    value: i64,
    rules: &FieldValidationRules,
) -> Result<(), String> {
    if let Some((min, max)) = rules.numeric_range {
        if value < min {
            return Err(format!(
                "Invalid field '{}': value {} is below minimum {}",
                field, value, min
            ));
        }
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
            field,
            expected_length,
            value,
            actual_length
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
    if let Some(exact) = rules.exact_length {
        validate_exact_length(field, value, exact)?;
    } else if let Some((min, max)) = rules.length_range {
        validate_string_length(field, value, Some(min), Some(max))?;
    }

    validate_regex_pattern(field, value, &rules.regex_pattern)?;

    Ok(())
}
