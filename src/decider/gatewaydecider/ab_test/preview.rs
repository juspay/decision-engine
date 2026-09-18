//! Rule-layer evaluation of an A/B arm for `/routing/evaluate` and the static half of
//! `/routing/hybrid`. The SR layer is applied by the decider intercept in interceptor.rs.

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
        utils::apply_default_fallback,
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

/// Evaluate the rule layer of the served arm. An arm without a rule layer answers with the
/// request's fallback connectors, the same answer `/routing/evaluate` gives when no rule applies.
/// Returns the routing output, evaluated connectors, rule name, and the flow type the preview
/// event is recorded under.
pub async fn evaluate_rule_arm(
    side: super::ArmSide,
    rule_algorithm_id: Option<&str>,
    payload: &RoutingRequest,
    db: &crate::storage::Storage,
) -> Result<AbTestArmOutput, ContainerError<EuclidErrors>> {
    let arm = side.as_str();
    let Some(arm_algorithm_id) = rule_algorithm_id else {
        let fallback = payload
            .fallback_output
            .clone()
            .filter(|connectors| !connectors.is_empty())
            .ok_or_else(|| {
                ContainerError::from(EuclidErrors::InvalidRequest(format!(
                    "A/B {arm} arm has no rule layer; fallback_output must list the candidate connectors"
                )))
            })?;
        return Ok(AbTestArmOutput {
            output: Output::Priority(fallback.clone()),
            evaluated_output: fallback,
            rule_name: Some("default_fallback".to_string()),
            flow_type: crate::analytics::flow::FlowType::RoutingEvaluatePriority,
        });
    };

    // Static arm: fetch the arm's algorithm from DB and evaluate it.
    let arm_algorithm = crate::generics::generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(db, dsl::id.eq(arm_algorithm_id.to_string()))
    .await
    .change_context(EuclidErrors::StorageError)?;

    let arm_algorithm_data: StaticRoutingAlgorithm =
        serde_json::from_str(&arm_algorithm.algorithm_data).map_err(|e| {
            ContainerError::from(EuclidErrors::InvalidRequest(format!(
                "Invalid arm algorithm data format: {}",
                e
            )))
        })?;

    let flow_type = crate::analytics::refine_routing_evaluate_flow_type(&arm_algorithm_data);

    let (output, evaluated_output, rule_name) = match arm_algorithm_data {
        StaticRoutingAlgorithm::Single(conn) => {
            let out_enum = Output::Single(*conn.clone());
            let eval = evaluate_output(&out_enum).map_err(|_| {
                ContainerError::from(EuclidErrors::FailedToEvaluateOutput(format!(
                    "ab_test arm single: {}",
                    conn.gateway_name
                )))
            })?;
            (
                out_enum,
                eval,
                Some(format!("ab_test_{arm}_straight_through")),
            )
        }
        StaticRoutingAlgorithm::Priority(connectors) => {
            let out_enum = Output::Priority(connectors.clone());
            let eval = evaluate_output(&out_enum).map_err(|_| {
                ContainerError::from(EuclidErrors::FailedToEvaluateOutput(
                    "ab_test arm priority".into(),
                ))
            })?;
            (out_enum, eval, Some(format!("ab_test_{arm}_priority")))
        }
        StaticRoutingAlgorithm::VolumeSplit(splits) => {
            let out_enum = Output::VolumeSplit(splits.clone());
            let eval = evaluate_output(&out_enum).map_err(|_| {
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
            apply_default_fallback(&mut ir, payload.fallback_output.as_deref());

            (ir.output, ir.evaluated_output, ir.rule_name)
        }
        StaticRoutingAlgorithm::AbTest(_) => {
            return Err(ContainerError::from(EuclidErrors::InvalidRequest(
                "Nested ab_test algorithms are not supported".into(),
            )));
        }
        StaticRoutingAlgorithm::VolumeContract(_) => {
            return Err(ContainerError::from(EuclidErrors::InvalidRequest(
                "Volume-commitment configurations cannot be used as an A/B test arm".into(),
            )));
        }
    };

    Ok(AbTestArmOutput {
        output,
        evaluated_output,
        rule_name,
        flow_type,
    })
}

/// Analytics detail serialization for AB test routing evaluate (Decision Explorer preview).
/// Includes full request/response so the rule-based audit shows proper input/response content.
pub fn serialize_analytics_details(
    request: &impl serde::Serialize,
    response: &impl serde::Serialize,
    rule_name: Option<&str>,
    assignment: &super::outcome::ExperimentAssignment,
) -> Option<String> {
    crate::analytics::serialize_details(&serde_json::json!({
        "request": request,
        "response": response,
        "rule_name": rule_name,
        "preview_kind": "routing_evaluate_ab_test",
        "experiment_id": assignment.experiment_id,
        "variant_arm": assignment.side.as_str(),
        "endpoint": assignment.endpoint.as_str(),
    }))
}
