use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::types::DomainDeciderRequestForApiCallV2;
use crate::euclid::ast::ValueType;
use crate::euclid::interpreter::InterpreterBackend;
use crate::euclid::types::{Context, RoutingAlgorithm, StaticRoutingAlgorithm};
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
}

/// Build the Euclid interpreter context from the payment for a rule-based (Advanced) arm.
/// v1 scope: card dimensions only (`payment_method`, `payment_method_type`, `card_type`/`card`).
/// Values are lowercased to match the Rule-Based builder's enum casing — the interpreter compares
/// enum variants with exact, case-sensitive string equality, so a casing mismatch silently skips
/// the rule. Dimensions not populated here make any rule that references them fall through to the
/// program's default_selection.
fn build_card_context(dreq: &DomainDeciderRequestForApiCallV2) -> Context {
    let mut params: HashMap<String, Option<ValueType>> = HashMap::new();

    params.insert(
        "payment_method".to_string(),
        Some(ValueType::EnumVariant(dreq.payment_method().to_lowercase())),
    );
    params.insert(
        "payment_method_type".to_string(),
        Some(ValueType::EnumVariant(
            dreq.payment_method_type().to_lowercase(),
        )),
    );
    if let Some(card_type) = dreq.card_type() {
        // CardType Display is SCREAMING_SNAKE ("DEBIT") — config uses "debit"/"credit".
        let ct = card_type.to_lowercase();
        params.insert(
            "card_type".to_string(),
            Some(ValueType::EnumVariant(ct.clone())),
        );
        // The `card` dimension in the routing-keys config is also debit/credit — populate both so
        // rules keyed on either name match.
        params.insert("card".to_string(), Some(ValueType::EnumVariant(ct)));
    }

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
        }),
        StaticRoutingAlgorithm::Priority(connectors) => {
            let first = connectors.first()?;
            let fallbacks = connectors
                .iter()
                .skip(1)
                .map(|c| c.gateway_name.clone())
                .collect();
            Some(StaticArmResult {
                decided_gateway: first.gateway_name.clone(),
                fallback_gateways: fallbacks,
                rule_name: Some("ab_test_static_priority".to_string()),
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
            let mut cumulative: u64 = 0;
            for split in &splits {
                cumulative += split.split as u64;
                if slot < cumulative {
                    return Some(StaticArmResult {
                        decided_gateway: split.output.gateway_name.clone(),
                        fallback_gateways: vec![],
                        rule_name: Some("ab_test_static_volume_split".to_string()),
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
            let ctx = build_card_context(dreq);
            let result = InterpreterBackend::eval_program(&program, &ctx)
                .inspect_err(|e| {
                    logger::warn!(
                        "ab_test evaluator: Advanced arm '{}' interpreter error: {:?} — falling back to SR routing",
                        algorithm_id,
                        e.error_type
                    )
                })
                .ok()?;

            let mut gateways = result.evaluated_output.into_iter().map(|c| c.gateway_name);
            let decided_gateway = gateways.next()?;
            Some(StaticArmResult {
                decided_gateway,
                fallback_gateways: gateways.collect(),
                rule_name: result
                    .rule_name
                    .or_else(|| Some("ab_test_rule_based".to_string())),
            })
        }
        StaticRoutingAlgorithm::AbTest(_) => {
            logger::error!(
                "ab_test evaluator: nested ab_test arm '{}' — skipping",
                algorithm_id
            );
            None
        }
    }
}
