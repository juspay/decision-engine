use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::types::DomainDeciderRequestForApiCallV2;
use crate::euclid::ast::ValueType;
use crate::euclid::interpreter::InterpreterBackend;
use crate::euclid::types::{
    Context, KeyDataType, RoutingAlgorithm, StaticRoutingAlgorithm, TomlConfig,
};
use crate::euclid::utils::parse_enum_values;
use crate::generics::generic_find_one;
use crate::logger;
use diesel::associations::HasTable;
use diesel::prelude::*;
use std::collections::HashMap;

#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl;

pub struct StaticArmResult {
    pub decided_gateway: String,
    pub fallback_gateways: Vec<String>,
    pub rule_name: Option<String>,
    /// Every gateway the rule admits for this payment, in declaration order. A `Rule` arm
    /// answers with `decided_gateway`; a `Hybrid` arm hands this whole set to SR to pick from,
    /// which is why it is tracked separately — for a volume split the two differ, the pick
    /// being one sampled output and the candidate set being all of them.
    pub candidates: Vec<String>,
}

/// Resolve a raw payment value to the spelling the routing-keys config declares for `key`.
/// The interpreter compares `EnumVariant`s with exact, case-sensitive string equality, so a
/// rule built from that config's dropdown only matches a context value spelled the same way.
/// An exact match wins; otherwise the first case-insensitive match in declaration order.
///
/// Falls back to the lowercased raw value when the config is unavailable or declares nothing
/// matching — the casing the Rule-Based builder uses for most keys. Note that `card_network`
/// declares several casings of the same network ("visa", "VISA", "Visa", ...); the first is
/// chosen, so a rule authored against one of the others still won't match.
fn resolve_enum_value(config: Option<&TomlConfig>, key: &str, raw: &str) -> String {
    let declared = config
        .and_then(|c| c.keys.keys.get(key))
        .filter(|key_config| key_config.data_type == KeyDataType::Enum)
        .map(parse_enum_values)
        .unwrap_or_default();

    if declared.iter().any(|value| value == raw) {
        return raw.to_string();
    }

    declared
        .into_iter()
        .find(|value| value.eq_ignore_ascii_case(raw))
        .unwrap_or_else(|| raw.to_lowercase())
}

fn insert_enum(
    params: &mut HashMap<String, Option<ValueType>>,
    config: Option<&TomlConfig>,
    key: &str,
    raw: &str,
) {
    params.insert(
        key.to_string(),
        Some(ValueType::EnumVariant(resolve_enum_value(config, key, raw))),
    );
}

/// Build the Euclid interpreter context from the payment for a rule-based (Advanced) arm.
///
/// Covers the dimensions `/config/routing-keys` exposes that the payment carries directly:
/// `payment_method`, `payment_method_type`, `card_type`/`card`, `currency`, `card_network`,
/// `authentication_type`, and `amount`/`payment_amount`. A dimension not populated here makes
/// any rule referencing it fall through to the program's `default_selection`, silently — which
/// is why the set is kept in step with the config rather than trimmed to card fields.
///
/// Still absent: `issuer_country` / `billing_country`. The payment carries ISO-2 ("US"), while
/// the config declares full names ("UnitedStatesOfAmerica"), and no mapping between the two
/// exists yet — populating them would need that mapping first.
fn build_payment_context(
    dreq: &DomainDeciderRequestForApiCallV2,
    config: Option<&TomlConfig>,
) -> Context {
    let mut params: HashMap<String, Option<ValueType>> = HashMap::new();

    insert_enum(&mut params, config, "payment_method", dreq.payment_method());
    insert_enum(
        &mut params,
        config,
        "payment_method_type",
        dreq.payment_method_type(),
    );
    insert_enum(&mut params, config, "currency", &dreq.currency());

    if let Some(card_type) = dreq.card_type() {
        // CardType Display is SCREAMING_SNAKE ("DEBIT"); the config declares "debit"/"credit".
        insert_enum(&mut params, config, "card_type", &card_type);
        // The `card` dimension in the routing-keys config is also debit/credit — populate both
        // so rules keyed on either name match.
        insert_enum(&mut params, config, "card", &card_type);
    }

    if let Some(card_network) = dreq.card_network() {
        insert_enum(&mut params, config, "card_network", &card_network);
    }

    if let Some(auth_type) = dreq.auth_type() {
        // AuthType Display is SCREAMING_SNAKE ("NO_THREE_DS"); the config declares "no_three_ds".
        insert_enum(&mut params, config, "authentication_type", &auth_type);
    }

    // Both keys are declared as `integer` and the builder offers either, so populate both from
    // the same figure. `ValueType::Number` is a u64, and the config floors these at 0.
    let amount = dreq.payment_info.amount.max(0.0).round() as u64;
    params.insert("amount".to_string(), Some(ValueType::Number(amount)));
    params.insert(
        "payment_amount".to_string(),
        Some(ValueType::Number(amount)),
    );

    Context::new(params)
}

pub async fn evaluate_static_arm(
    algorithm_id: &str,
    payment_id: &str,
    dreq: &DomainDeciderRequestForApiCallV2,
) -> Option<StaticArmResult> {
    let state = get_tenant_app_state().await;

    let algorithm = generic_find_one::<<RoutingAlgorithm as HasTable>::Table, _, RoutingAlgorithm>(
        &state.db,
        dsl::id.eq(algorithm_id.to_string()),
    )
    .await
    .ok()?;

    let parsed: StaticRoutingAlgorithm = serde_json::from_str(&algorithm.algorithm_data)
        .inspect_err(|e| {
            logger::error!(
                "ab_test evaluator: failed to parse algorithm {}: {}",
                algorithm_id,
                e
            )
        })
        .ok()?;

    match parsed {
        StaticRoutingAlgorithm::Single(conn) => Some(StaticArmResult {
            decided_gateway: conn.gateway_name.clone(),
            fallback_gateways: vec![],
            rule_name: Some("ab_test_static_single".to_string()),
            candidates: vec![conn.gateway_name.clone()],
        }),
        StaticRoutingAlgorithm::Priority(connectors) => {
            let candidates: Vec<String> =
                connectors.iter().map(|c| c.gateway_name.clone()).collect();
            let (first, fallbacks) = candidates.split_first()?;
            Some(StaticArmResult {
                decided_gateway: first.clone(),
                fallback_gateways: fallbacks.to_vec(),
                rule_name: Some("ab_test_static_priority".to_string()),
                candidates,
            })
        }
        StaticRoutingAlgorithm::VolumeSplit(splits) => {
            // Deterministic split via djb2 hash of payment_id (same as arm assignment)
            let hash = payment_id.bytes().fold(5381u64, |acc, b| {
                acc.wrapping_mul(33).wrapping_add(b as u64)
            });
            let total_weight: u64 = splits.iter().map(|s| s.split as u64).sum();
            if total_weight == 0 {
                return None;
            }
            let slot = hash % total_weight;
            // Every output is a candidate; only the pick is sampled. A `Hybrid` arm over a
            // volume split therefore hands SR the whole set of processors the split uses,
            // which is the comparison "would SR beat my fixed split" actually needs.
            let candidates: Vec<String> = splits
                .iter()
                .map(|s| s.output.gateway_name.clone())
                .collect();
            let mut cumulative: u64 = 0;
            for split in &splits {
                cumulative += split.split as u64;
                if slot < cumulative {
                    return Some(StaticArmResult {
                        decided_gateway: split.output.gateway_name.clone(),
                        fallback_gateways: vec![],
                        rule_name: Some("ab_test_static_volume_split".to_string()),
                        candidates,
                    });
                }
            }
            None
        }
        // Advanced (rule-based) arm: evaluate the Euclid program against the payment's card
        // dimensions. When no rule matches, eval_program returns the program's default_selection
        // (the merchant's catch-all gateway) — that is the correct rule-based outcome, not an SR
        // fallback. We only fall back to SR (None) if the program can't be evaluated or yields no
        // gateway.
        StaticRoutingAlgorithm::Advanced(program) => {
            let ctx = build_payment_context(dreq, state.config.routing_config.as_ref());
            let result = InterpreterBackend::eval_program(&program, &ctx)
                .inspect_err(|e| {
                    logger::warn!(
                        "ab_test evaluator: Advanced arm '{}' interpreter error: {:?} — falling back to SR routing",
                        algorithm_id,
                        e.error_type
                    )
                })
                .ok()?;

            let candidates: Vec<String> = result
                .evaluated_output
                .into_iter()
                .map(|c| c.gateway_name)
                .collect();
            let (decided_gateway, fallbacks) = candidates.split_first()?;
            Some(StaticArmResult {
                decided_gateway: decided_gateway.clone(),
                fallback_gateways: fallbacks.to_vec(),
                rule_name: result
                    .rule_name
                    .or_else(|| Some("ab_test_rule_based".to_string())),
                candidates,
            })
        }
        StaticRoutingAlgorithm::AbTest(_) => {
            logger::error!(
                "ab_test evaluator: nested ab_test arm '{}' — skipping",
                algorithm_id
            );
            None
        }
        StaticRoutingAlgorithm::VolumeContract(_) => {
            logger::error!(
                "ab_test evaluator: arm '{}' is a volume-commitment configuration, not an algorithm — skipping",
                algorithm_id
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_enum_value;
    use crate::euclid::types::{KeyConfig, KeyDataType, KeysConfig, TomlConfig};
    use std::collections::BTreeMap;

    fn config_with(key: &str, data_type: KeyDataType, values: Option<&str>) -> TomlConfig {
        let mut keys = BTreeMap::new();
        keys.insert(
            key.to_string(),
            KeyConfig {
                data_type,
                values: values.map(str::to_string),
                min_value: None,
                max_value: None,
                min_length: None,
                max_length: None,
                exact_length: None,
                regex: None,
            },
        );
        TomlConfig {
            keys: KeysConfig { keys },
        }
    }

    #[test]
    fn a_value_already_spelled_as_the_config_declares_it_is_left_alone() {
        let config = config_with("currency", KeyDataType::Enum, Some("USD, AED, EUR"));
        assert_eq!(
            resolve_enum_value(Some(&config), "currency", "AED"),
            "AED".to_string()
        );
    }

    /// AuthType/CardType Display are SCREAMING_SNAKE while the config declares lower snake —
    /// the interpreter compares case-sensitively, so the declared spelling has to win.
    #[test]
    fn a_value_in_another_case_is_rewritten_to_the_declared_spelling() {
        let config = config_with(
            "authentication_type",
            KeyDataType::Enum,
            Some("three_ds, no_three_ds"),
        );
        assert_eq!(
            resolve_enum_value(Some(&config), "authentication_type", "NO_THREE_DS"),
            "no_three_ds".to_string()
        );
    }

    /// `card_network` declares the same network in several casings; the first declared wins, so
    /// the choice is at least deterministic across restarts.
    #[test]
    fn the_first_declared_casing_wins_when_a_key_declares_several() {
        let config = config_with(
            "card_network",
            KeyDataType::Enum,
            Some("visa, VISA, Visa, mastercard, MASTERCARD"),
        );
        assert_eq!(
            resolve_enum_value(Some(&config), "card_network", "MasterCard"),
            "mastercard".to_string()
        );
    }

    #[test]
    fn a_value_the_config_does_not_declare_falls_back_to_lowercase() {
        let config = config_with("card_type", KeyDataType::Enum, Some("debit, credit"));
        assert_eq!(
            resolve_enum_value(Some(&config), "card_type", "PREPAID"),
            "prepaid".to_string()
        );
    }

    #[test]
    fn an_unavailable_config_falls_back_to_lowercase() {
        assert_eq!(
            resolve_enum_value(None, "currency", "AED"),
            "aed".to_string()
        );
    }

    /// A key the config declares as something other than an enum has no candidate spellings to
    /// match against — don't invent any.
    #[test]
    fn a_non_enum_key_falls_back_to_lowercase() {
        let config = config_with("issuer_name", KeyDataType::StrValue, None);
        assert_eq!(
            resolve_enum_value(Some(&config), "issuer_name", "HDFC"),
            "hdfc".to_string()
        );
    }
}
