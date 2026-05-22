use crate::app::get_tenant_app_state;
use crate::euclid::types::{RoutingAlgorithm, StaticRoutingAlgorithm};
use crate::generics::generic_find_one;
use crate::logger;
use diesel::associations::HasTable;
use diesel::prelude::*;

#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl;

pub struct StaticArmResult {
    pub decided_gateway: String,
    pub fallback_gateways: Vec<String>,
    pub rule_name: Option<String>,
}

pub async fn evaluate_static_arm(algorithm_id: &str, payment_id: &str) -> Option<StaticArmResult> {
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
        // Advanced (rule-based) arms in real payment flow need full payment parameters
        // which aren't available here without significant refactoring.
        // Graceful degradation: fall back to SR routing for this arm.
        StaticRoutingAlgorithm::Advanced(_) => {
            logger::warn!(
                "ab_test evaluator: Advanced/rule-based arm '{}' cannot be evaluated in real payment flow without payment parameters — falling back to SR routing",
                algorithm_id
            );
            None
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
