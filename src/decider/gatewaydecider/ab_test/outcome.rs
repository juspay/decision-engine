use crate::analytics::{
    events::DomainAnalyticsEvent, service::serialize_details, AnalyticsFlowContext, AnalyticsRoute,
    ApiFlow, FlowType,
};
use crate::app::get_tenant_app_state;
use crate::logger;
use serde::{Deserialize, Serialize};

const INFLIGHT_TTL_SECS: i64 = 3600;

fn inflight_key(payment_id: &str) -> String {
    format!("ab_test_inflight:{}", payment_id)
}

#[derive(Debug, Serialize, Deserialize)]
struct InflightContext {
    experiment_id: String,
    variant_arm: String,
    gateway: Option<String>,
    is_static_arm: bool,
    /// Multi-objective cost outcome, filled in after routing (SR arms only) via
    /// `record_cost_outcome`. `None` when the arm ran auth-only (cost off / not yet enriched).
    #[serde(default)]
    cost_saved_bps: Option<f64>,
    #[serde(default)]
    chosen_cost_bps: Option<f64>,
    #[serde(default)]
    margin: Option<f64>,
}

/// Returns true only for static-arm AB test payments.
/// SR arm payments went through real SR routing and need UpdateGatewayScoreUpdate
/// to show their outcome in the auth-rate audit — don't suppress it for them.
pub async fn is_static_arm_inflight(payment_id: &str) -> bool {
    let state = get_tenant_app_state().await;
    let key = inflight_key(payment_id);
    let ctx: Option<InflightContext> = state
        .redis_conn
        .get_key(&key, "ab_test_inflight")
        .await
        .ok()
        .flatten();
    ctx.map(|c| c.is_static_arm).unwrap_or(false)
}

pub async fn store_inflight(
    payment_id: &str,
    experiment_id: &str,
    variant_arm: &str,
    gateway: Option<&str>,
    is_static_arm: bool,
) {
    let state = get_tenant_app_state().await;
    let key = inflight_key(payment_id);
    let ctx = InflightContext {
        experiment_id: experiment_id.to_string(),
        variant_arm: variant_arm.to_string(),
        gateway: gateway.map(str::to_string),
        is_static_arm,
        cost_saved_bps: None,
        chosen_cost_bps: None,
        margin: None,
    };
    if let Err(e) = state
        .redis_conn
        .set_key_with_ttl(&key, ctx, INFLIGHT_TTL_SECS)
        .await
    {
        logger::warn!(
            "ab_test outcome: failed to store inflight context for {}: {:?}",
            payment_id,
            e
        );
    }
}

/// After routing completes for an SR-arm A/B payment, re-write the inflight record with the
/// now-known decided gateway and the multi-objective cost outcome (cost saved, chosen PSP cost,
/// margin). Lets the later outcome event (`emit_if_in_flight`) attribute cost per arm. Cost
/// fields are `None` when the arm ran auth-only (multi-objective off), in which case this still
/// backfills the decided gateway. Only ever called for SR arms, so `is_static_arm` stays false.
#[allow(clippy::too_many_arguments)]
pub async fn record_cost_outcome(
    payment_id: &str,
    experiment_id: &str,
    variant_arm: &str,
    gateway: Option<&str>,
    cost_saved_bps: Option<f64>,
    chosen_cost_bps: Option<f64>,
    margin: Option<f64>,
) {
    let state = get_tenant_app_state().await;
    let key = inflight_key(payment_id);
    let ctx = InflightContext {
        experiment_id: experiment_id.to_string(),
        variant_arm: variant_arm.to_string(),
        gateway: gateway.map(str::to_string),
        is_static_arm: false,
        cost_saved_bps,
        chosen_cost_bps,
        margin,
    };
    if let Err(e) = state
        .redis_conn
        .set_key_with_ttl(&key, ctx, INFLIGHT_TTL_SECS)
        .await
    {
        logger::warn!(
            "ab_test outcome: failed to enrich inflight context for {}: {:?}",
            payment_id,
            e
        );
    }
}

pub async fn emit_if_in_flight(payment_id: &str, merchant_id: &str, is_success: bool) {
    let state = get_tenant_app_state().await;
    let key = inflight_key(payment_id);

    let ctx: Option<InflightContext> = state
        .redis_conn
        .get_key(&key, "ab_test_inflight")
        .await
        .ok()
        .flatten();

    let Some(ctx) = ctx else { return };
    let _ = state.redis_conn.delete_key(&key).await;

    let status = if is_success { "success" } else { "failure" };
    let details = serialize_details(&serde_json::json!({
        "experiment_id": ctx.experiment_id,
        "variant_arm": ctx.variant_arm,
        "preview_kind": "routing_evaluate_ab_test",
        "outcome_source": "score_update",
        "cost_saved_bps": ctx.cost_saved_bps,
        "chosen_cost_bps": ctx.chosen_cost_bps,
        "margin": ctx.margin,
    }));

    DomainAnalyticsEvent::record_decision(
        AnalyticsFlowContext::new(ApiFlow::RuleBasedRouting, FlowType::RoutingEvaluateAbTest),
        Some(merchant_id.to_string()),
        Some("AB_TEST_REAL_PAYMENT".to_string()),
        ctx.gateway,
        Some(status.to_string()),
        AnalyticsRoute::RoutingEvaluate,
        None,
        details,
        Some(payment_id.to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        // card_network, currency, country — not available on the A/B outcome path.
        None,
        None,
        None,
    );

    logger::debug!(
        "ab_test outcome: emitted {} for payment_id={} experiment={} arm={}",
        status,
        payment_id,
        ctx.experiment_id,
        ctx.variant_arm
    );
}
