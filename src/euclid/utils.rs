use super::ast::{
    Comparison, ComparisonType, ConnectorInfo, IfStatement, Output, Rule, ValueType, VolumeSplit,
};
use super::errors::{EuclidErrors, ValidationErrorDetails};
use super::interpreter::RoutingError;
use super::types::{AlgorithmType, BackendOutput, KeyDataType, StaticRoutingAlgorithm};
use crate::error::ContainerError;
use crate::euclid::types::{FieldValidationRules, KeyConfig, RoutingRule, TomlConfig};
use rand::distributions::WeightedIndex;
use rand::prelude::*;
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

/// Coerces condition values to the representation their key's configured data type expects.
///
/// Hyperswitch's rule migration serializes numeric-looking literals as `number` values even for
/// keys this service declares as `str_value` (`card_bin`, `extended_card_bin`), so those rules
/// would be rejected by [`validate_routing_rule`] and — if stored as-is — could never match at
/// evaluation time, since the interpreter only compares `str_value` against `str_value`. Any
/// length/regex constraints on the key still apply to the coerced string during validation.
pub fn normalize_rule_value_types(algorithm: &mut StaticRoutingAlgorithm, config: &TomlConfig) {
    let StaticRoutingAlgorithm::Advanced(program) = algorithm else {
        return;
    };
    for rule in &mut program.rules {
        for statement in &mut rule.statements {
            normalize_statement_value_types(statement, config);
        }
    }
}

fn normalize_statement_value_types(statement: &mut IfStatement, config: &TomlConfig) {
    for condition in &mut statement.condition {
        normalize_condition_value_type(condition, config);
    }
    if let Some(nested) = &mut statement.nested {
        for nested_statement in nested {
            normalize_statement_value_types(nested_statement, config);
        }
    }
}

fn normalize_condition_value_type(condition: &mut Comparison, config: &TomlConfig) {
    let is_str_value_key = config
        .keys
        .keys
        .get(&condition.lhs)
        .is_some_and(|key_config| key_config.data_type == KeyDataType::StrValue);
    if !is_str_value_key {
        return;
    }

    if let ValueType::Number(number) = &condition.value {
        condition.value = ValueType::StrValue(number.to_string());
    }
}

pub fn validate_routing_rule(
    rule: &RoutingRule,
    config: &Option<TomlConfig>,
) -> Result<ValidationResult, ContainerError<EuclidErrors>> {
    let config = config
        .clone()
        .ok_or_else(|| error_stack::report!(EuclidErrors::GlobalRoutingConfigsUnavailable))?;

    // The volume_commitment activation slot and the volume_contract payload imply each other:
    // a contract document in the payment slot would reach /routing/evaluate, and a routing
    // algorithm in the volume_commitment slot would be dead weight.
    let is_volume_contract = matches!(rule.algorithm, StaticRoutingAlgorithm::VolumeContract(_));
    if is_volume_contract != matches!(rule.algorithm_for, AlgorithmType::VolumeCommitment) {
        return Ok(ValidationResult::failure(vec![
            ValidationErrorDetails::new(
                "algorithm_for",
                "invalid_value",
                if is_volume_contract {
                    "a volume_contract algorithm requires algorithm_for: volume_commitment"
                } else {
                    "algorithm_for: volume_commitment requires a volume_contract algorithm"
                },
            ),
        ]));
    }

    match &rule.algorithm {
        StaticRoutingAlgorithm::Single(_)
        | StaticRoutingAlgorithm::Priority(_)
        | StaticRoutingAlgorithm::VolumeSplit(_)
        | StaticRoutingAlgorithm::AbTest(_) => Ok(ValidationResult::success()),
        StaticRoutingAlgorithm::VolumeContract(contract_config) => {
            let validation_errors =
                crate::euclid::volume_contract::validate_volume_contract_config(contract_config);
            if validation_errors.is_empty() {
                Ok(ValidationResult::success())
            } else {
                for error in &validation_errors {
                    crate::logger::warn!(
                        field = %error.field,
                        error_type = %error.error_type,
                        message = %error.message,
                        "Volume contract validation error"
                    );
                }
                Ok(ValidationResult::failure(validation_errors))
            }
        }
        StaticRoutingAlgorithm::Advanced(program) => {
            let mut validation_errors: Vec<ValidationErrorDetails> = Vec::new();

            for rule in &program.rules {
                validate_rule(rule, &config, &mut validation_errors);
            }

            if validation_errors.is_empty() {
                Ok(ValidationResult::success())
            } else {
                for error in &validation_errors {
                    crate::logger::warn!(
                        field = %error.field,
                        error_type = %error.error_type,
                        message = %error.message,
                        "Field validation error"
                    );
                }

                let result = ValidationResult::failure(validation_errors);
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

/// validates the comparison operators for different subtle value types present
/// by throwing required errors for comparisons that can't be performed for a certain value type
/// for example
/// can't have greater/less than operations on enum types
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
            format!("Invalid key '{}': Unknown key in condition", condition.lhs),
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
            errors.push(ValidationErrorDetails::new(
                &condition.lhs,
                "invalid_comparison",
                format!(
                    "Invalid comparison type '{}': expected Equal or NotEqual, got {:?}",
                    condition.lhs, condition.comparison
                ),
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
                errors.push(ValidationErrorDetails::new(
                    &condition.lhs,
                    "invalid_enum_value",
                    format!(
                        "Invalid enum value '{}': expected one of {:?}, got '{}'",
                        condition.lhs, valid_values, value
                    ),
                ));
            }
        }
        (KeyDataType::Enum, ValueType::EnumVariantArray(arr)) => {
            let invalid: Vec<_> = arr
                .iter()
                .filter(|v| !is_valid_enum_value(config, &condition.lhs, v))
                .cloned()
                .collect();
            if !invalid.is_empty() {
                let valid_values = parse_enum_values(key_config);
                errors.push(ValidationErrorDetails::new(
                    &condition.lhs,
                    "invalid_enum_values",
                    format!(
                        "Invalid enum values '{}': expected values from {:?}, got {:?}",
                        condition.lhs, valid_values, invalid
                    ),
                ));
            }
        }
        (KeyDataType::Enum, _) => {
            errors.push(ValidationErrorDetails::new(
                &condition.lhs,
                "type_mismatch",
                format!(
                    "Invalid enum variant '{}': expected enum variant, got {:?}",
                    condition.lhs,
                    condition.value.get_type()
                ),
            ));
        }

        (KeyDataType::Integer, ValueType::Number(n)) => {
            if key_config.has_validation_constraints() {
                if let Ok(rules) = key_config.build_validation_rules() {
                    if let Err(e) = validate_numeric_range(&condition.lhs, *n as i64, &rules) {
                        let mut expected_parts = Vec::new();
                        if let Some(min) = rules.min_value {
                            expected_parts.push(format!("min: {}", min));
                        }
                        if let Some(max) = rules.max_value {
                            expected_parts.push(format!("max: {}", max));
                        }
                        errors.push(ValidationErrorDetails::new(
                            &condition.lhs,
                            "value_out_of_range",
                            e,
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
                errors.push(ValidationErrorDetails::new(
                    &condition.lhs,
                    "invalid_comparison",
                    format!(
                        "Only '==' or '!=' allowed with number arrays for key '{}'",
                        condition.lhs
                    ),
                ));
            }

            if key_config.has_validation_constraints() {
                if let Ok(rules) = key_config.build_validation_rules() {
                    for (i, n) in arr.iter().enumerate() {
                        if let Err(e) = validate_numeric_range(&condition.lhs, *n as i64, &rules) {
                            let mut expected_parts = Vec::new();
                            if let Some(min) = rules.min_value {
                                expected_parts.push(format!("min: {}", min));
                            }
                            if let Some(max) = rules.max_value {
                                expected_parts.push(format!("max: {}", max));
                            }
                            errors.push(ValidationErrorDetails::new(
                                &condition.lhs,
                                "value_out_of_range",
                                format!("Element {}: {}", i + 1, e),
                            ));
                        }
                    }
                }
            }
        }
        (KeyDataType::Integer, ValueType::NumberComparisonArray(_)) => {
            if condition.comparison != ComparisonType::Equal {
                errors.push(ValidationErrorDetails::new(
                    &condition.lhs,
                    "invalid_comparison",
                    format!(
                        "Only '==' allowed with number comparison arrays for key '{}'",
                        condition.lhs
                    ),
                ));
            }
        }
        (KeyDataType::Integer, _) => {
            errors.push(ValidationErrorDetails::new(
                &condition.lhs,
                "type_mismatch",
                format!(
                    "Invalid key '{}': expected number, got {:?}",
                    condition.lhs,
                    condition.value.get_type()
                ),
            ));
        }

        (KeyDataType::Udf, ValueType::MetadataVariant(m)) => {
            if key_config.has_validation_constraints() {
                if let Ok(rules) = key_config.build_validation_rules() {
                    if let Err(e) = validate_string_value(&condition.lhs, &m.value, &rules) {
                        errors.push(ValidationErrorDetails::new(
                            &condition.lhs,
                            "length_invalid",
                            e,
                        ));
                    }
                }
            }
        }
        (KeyDataType::Udf, _) => {
            errors.push(ValidationErrorDetails::new(
                &condition.lhs,
                "type_mismatch",
                format!(
                    "Invalid key '{}': expected metadata variant, got {:?}",
                    condition.lhs,
                    condition.value.get_type()
                ),
            ));
        }

        (KeyDataType::StrValue, ValueType::StrValue(s)) => {
            if key_config.has_validation_constraints() {
                if let Ok(rules) = key_config.build_validation_rules() {
                    if let Err(e) = validate_string_value(&condition.lhs, s, &rules) {
                        errors.push(ValidationErrorDetails::new(
                            &condition.lhs,
                            "length_invalid",
                            e,
                        ));
                    }
                }
            }
        }

        _ => {
            if condition.value.get_type().to_string() != key_config.data_type.as_str() {
                errors.push(ValidationErrorDetails::new(
                    &condition.lhs,
                    "type_mismatch",
                    format!(
                        "Invalid key '{}': expected {}, got {}",
                        condition.lhs,
                        key_config.data_type.as_str(),
                        condition.value.get_type()
                    ),
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
    if let Some(min) = rules.min_value {
        if value < min {
            return Err(format!(
                "Invalid field '{}': value {} is below minimum {}",
                field, value, min
            ));
        }
    }
    if let Some(max) = rules.max_value {
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
            "Invalid field '{}': expected {} characters, got {} characters",
            field, expected_length, actual_length
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
    } else if rules.min_length.is_some() || rules.max_length.is_some() {
        if let Err(e) = validate_string_length(field, value, rules.min_length, rules.max_length) {
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

pub fn sample_split_winner_first<T>(
    mut splits: Vec<VolumeSplit<T>>,
) -> Result<Vec<T>, RoutingError> {
    let weights: Vec<u8> = splits.iter().map(|sp| sp.split).collect();
    let weighted_index =
        WeightedIndex::new(weights).map_err(|_| RoutingError::VolumeSplitFailed)?;
    let mut rng = rand::thread_rng();
    let idx = weighted_index.sample(&mut rng);

    if idx >= splits.len() {
        return Err(RoutingError::VolumeSplitFailed);
    }
    let winner = splits.remove(idx);
    splits.insert(0, winner);

    Ok(splits.into_iter().map(|split| split.output).collect())
}

pub fn apply_default_fallback(ir: &mut BackendOutput, fallback_output: Option<&[ConnectorInfo]>) {
    if ir.rule_name.is_none() {
        if let Some(fallback) = fallback_output.filter(|connectors| !connectors.is_empty()) {
            crate::logger::debug!("Default fallback triggered: Overriding with fallback connector");

            ir.rule_name = Some("default_fallback".to_string());
            ir.output = Output::Priority(fallback.to_vec());
            ir.evaluated_output = fallback.to_vec();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::euclid::types::KeysConfig;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn key_config(data_type: KeyDataType) -> KeyConfig {
        KeyConfig {
            data_type,
            values: None,
            min_value: None,
            max_value: None,
            min_length: None,
            max_length: None,
            exact_length: None,
            regex: None,
        }
    }

    fn routing_config() -> TomlConfig {
        let mut keys = BTreeMap::new();
        keys.insert(
            "card_bin".to_string(),
            KeyConfig {
                exact_length: Some(6),
                regex: Some("^[0-9]{6}$".to_string()),
                ..key_config(KeyDataType::StrValue)
            },
        );
        keys.insert(
            "extended_card_bin".to_string(),
            KeyConfig {
                exact_length: Some(8),
                regex: Some("^[0-9]{8}$".to_string()),
                ..key_config(KeyDataType::StrValue)
            },
        );
        keys.insert("amount".to_string(), key_config(KeyDataType::Integer));
        TomlConfig {
            keys: KeysConfig { keys },
        }
    }

    /// A rule in the shape Hyperswitch's migration sends: BIN conditions carrying `number`
    /// literals for keys this service declares as `str_value`.
    fn migrated_rule(card_bin_value: serde_json::Value) -> RoutingRule {
        serde_json::from_value(json!({
            "name": "migrated rule",
            "created_by": "merchant_1",
            "algorithm": {
                "type": "advanced",
                "data": {
                    "globals": {},
                    "default_selection": {
                        "priority": [{"gateway_name": "stripe", "gateway_id": null}]
                    },
                    "metadata": null,
                    "rules": [{
                        "name": "rule_1",
                        "routing_type": "priority",
                        "output": {"priority": [{"gateway_name": "adyen", "gateway_id": null}]},
                        "statements": [{
                            "condition": [
                                {
                                    "lhs": "card_bin",
                                    "comparison": "equal",
                                    "value": card_bin_value,
                                    "metadata": {}
                                },
                                {
                                    "lhs": "amount",
                                    "comparison": "greater_than",
                                    "value": {"type": "number", "value": 100},
                                    "metadata": {}
                                }
                            ],
                            "nested": [{
                                "condition": [{
                                    "lhs": "extended_card_bin",
                                    "comparison": "equal",
                                    "value": {"type": "number", "value": 41111111u64},
                                    "metadata": {}
                                }],
                                "nested": null
                            }]
                        }]
                    }]
                }
            }
        }))
        .expect("migrated rule payload should deserialize")
    }

    #[test]
    fn normalize_coerces_numbers_to_strings_for_str_value_keys() {
        let mut rule = migrated_rule(json!({"type": "number", "value": 411111}));

        normalize_rule_value_types(&mut rule.algorithm, &routing_config());

        let StaticRoutingAlgorithm::Advanced(program) = &rule.algorithm else {
            panic!("expected advanced algorithm");
        };
        let statement = &program.rules[0].statements[0];
        assert_eq!(
            statement.condition[0].value,
            ValueType::StrValue("411111".to_string())
        );
        // Integer-typed keys keep their numeric values.
        assert_eq!(statement.condition[1].value, ValueType::Number(100));
        // Nested statements are normalized too.
        let nested = statement.nested.as_ref().expect("nested statements");
        assert_eq!(
            nested[0].condition[0].value,
            ValueType::StrValue("41111111".to_string())
        );
    }

    #[test]
    fn migrated_rule_with_numeric_bins_passes_validation_after_normalization() {
        let mut rule = migrated_rule(json!({"type": "number", "value": 411111}));
        let config = Some(routing_config());

        let before = validate_routing_rule(&rule, &config).expect("validation should run");
        assert!(!before.is_valid);
        assert!(before
            .to_error_message()
            .contains("expected str_value, got number"));

        normalize_rule_value_types(&mut rule.algorithm, config.as_ref().expect("config"));

        let after = validate_routing_rule(&rule, &config).expect("validation should run");
        assert!(after.is_valid, "unexpected errors: {:?}", after.errors);
    }

    #[test]
    fn coerced_values_are_still_checked_against_key_constraints() {
        // A 5-digit BIN is invalid regardless of representation; coercion must not mask that.
        let mut rule = migrated_rule(json!({"type": "number", "value": 41111}));
        let config = Some(routing_config());

        normalize_rule_value_types(&mut rule.algorithm, config.as_ref().expect("config"));

        let result = validate_routing_rule(&rule, &config).expect("validation should run");
        assert!(!result.is_valid);
        assert!(result.to_error_message().contains("expected 6 characters"));
    }

    #[test]
    fn normalize_leaves_str_value_conditions_untouched() {
        let mut rule = migrated_rule(json!({"type": "str_value", "value": "411111"}));

        normalize_rule_value_types(&mut rule.algorithm, &routing_config());

        let StaticRoutingAlgorithm::Advanced(program) = &rule.algorithm else {
            panic!("expected advanced algorithm");
        };
        assert_eq!(
            program.rules[0].statements[0].condition[0].value,
            ValueType::StrValue("411111".to_string())
        );
    }
}
