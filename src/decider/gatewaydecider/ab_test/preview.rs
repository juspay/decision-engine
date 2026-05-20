//! AB test arm evaluation for the Decision Explorer (routing evaluate / preview flow).
//! Real payment intercept lives in interceptor.rs + evaluator.rs.

#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl;

use crate::{
    error::ContainerError,
    euclid::{
        ast::{ConnectorInfo, Output},
        errors::EuclidErrors,
        interpreter::{evaluate_output, InterpreterBackend},
        types::{Context, RoutingAlgorithm, RoutingRequest, StaticRoutingAlgorithm},
    },
};
use diesel::{associations::HasTable, ExpressionMethods};
use error_stack::ResultExt;

pub struct AbTestArmOutput {
    pub output: Output,
    pub evaluated_output: Vec<ConnectorInfo>,
    pub rule_name: Option<String>,
    pub flow_type: crate::analytics::flow::FlowType,
}

/// Evaluate the selected AB test arm for the Decision Explorer preview flow.
/// Returns the routing output, evaluated connectors, rule name, and the arm's
/// flow type (used so preview events get the correct summary_kind in the audit).
pub async fn evaluate_arm(
    arm: &str,
    arm_algorithm_id: &str,
    payload: &RoutingRequest,
    default_output_present: bool,
    db: &crate::storage::Storage,
) -> Result<AbTestArmOutput, ContainerError<EuclidErrors>> {
    // SR arm: in simulation, pick the first fallback connector as a proxy for what
    // SR scoring would select in a real payment.
    if arm_algorithm_id == "sr_routing" {
        let chosen = payload
            .fallback_output
            .as_deref()
            .and_then(|cs| cs.first().cloned())
            .ok_or_else(|| {
                ContainerError::from(EuclidErrors::InvalidRequest(
                    "SR routing arm requires at least one connector in fallback_output".into(),
                ))
            })?;
        let out_enum = Output::Single(chosen.clone());
        let (_, evaluated) = evaluate_output(&out_enum).map_err(|_| {
            ContainerError::from(EuclidErrors::FailedToEvaluateOutput(
                "ab_test sr routing arm evaluation".into(),
            ))
        })?;
        return Ok(AbTestArmOutput {
            output: out_enum,
            evaluated_output: evaluated,
            rule_name: Some(format!("ab_test_{arm}_sr_routing")),
            flow_type: crate::analytics::flow::FlowType::RoutingEvaluateSingle,
        });
    }

    // Static arm: fetch the arm's algorithm from DB and evaluate it.
    let arm_algorithm = crate::generics::generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(db, dsl::id.eq(arm_algorithm_id.to_string()))
    .await
    .change_context(EuclidErrors::StorageError)?;

    let arm_algorithm_data: StaticRoutingAlgorithm =
        serde_json::from_str(&arm_algorithm.algorithm_data)
            .map_err(|e| {
                ContainerError::from(EuclidErrors::InvalidRequest(format!(
                    "Invalid arm algorithm data format: {}",
                    e
                )))
            })?;

    let flow_type = crate::analytics::refine_routing_evaluate_flow_type(&arm_algorithm_data);

    let (output, evaluated_output, rule_name) = match arm_algorithm_data {
        StaticRoutingAlgorithm::Single(conn) => {
            let out_enum = Output::Single(*conn.clone());
            let (_, eval) = evaluate_output(&out_enum).map_err(|_| {
                ContainerError::from(EuclidErrors::FailedToEvaluateOutput(format!(
                    "ab_test arm single: {}",
                    conn.gateway_name
                )))
            })?;
            (out_enum, eval, Some(format!("ab_test_{arm}_straight_through")))
        }
        StaticRoutingAlgorithm::Priority(connectors) => {
            let out_enum = Output::Priority(connectors.clone());
            let (_, eval) = evaluate_output(&out_enum).map_err(|_| {
                ContainerError::from(EuclidErrors::FailedToEvaluateOutput(
                    "ab_test arm priority".into(),
                ))
            })?;
            (out_enum, eval, Some(format!("ab_test_{arm}_priority")))
        }
        StaticRoutingAlgorithm::VolumeSplit(splits) => {
            let out_enum = Output::VolumeSplit(splits.clone());
            let (_, eval) = evaluate_output(&out_enum).map_err(|_| {
                ContainerError::from(EuclidErrors::FailedToEvaluateOutput(
                    "ab_test arm volume_split".into(),
                ))
            })?;
            (out_enum, eval, Some(format!("ab_test_{arm}_volume_split")))
        }
        StaticRoutingAlgorithm::Advanced(program) => {
            let ctx = Context::new(payload.parameters.clone());
            let mut ir = InterpreterBackend::eval_program(&program, &ctx).map_err(|e| {
                ContainerError::from(EuclidErrors::InvalidRequest(format!(
                    "AB test arm interpreter error: {:?}",
                    e.error_type
                )))
            })?;
            if default_output_present && ir.output == program.default_selection {
                if let Some(fallback) = payload.fallback_output.clone() {
                    ir.rule_name = Some("default_fallback".to_string());
                    ir.output = Output::Priority(fallback.clone());
                    ir.evaluated_output = vec![fallback.into_iter().next().unwrap_or_default()];
                }
            }
            (ir.output, ir.evaluated_output, ir.rule_name)
        }
        StaticRoutingAlgorithm::AbTest(_) => {
            return Err(ContainerError::from(EuclidErrors::InvalidRequest(
                "Nested ab_test algorithms are not supported".into(),
            )));
        }
    };

    Ok(AbTestArmOutput { output, evaluated_output, rule_name, flow_type })
}

/// Analytics detail serialization for AB test routing evaluate (Decision Explorer preview).
/// Includes full request/response so the rule-based audit shows proper input/response content.
pub fn serialize_analytics_details(
    request: &impl serde::Serialize,
    response: &impl serde::Serialize,
    rule_name: Option<&str>,
    experiment_id: &str,
    variant_arm: &str,
) -> Option<String> {
    crate::analytics::serialize_details(&serde_json::json!({
        "request": request,
        "response": response,
        "rule_name": rule_name,
        "preview_kind": "routing_evaluate_ab_test",
        "experiment_id": experiment_id,
        "variant_arm": variant_arm,
    }))
}
