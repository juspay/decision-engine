use crate::analytics::{
    events::DomainAnalyticsEvent, service::serialize_details, AnalyticsFlowContext, AnalyticsRoute,
    ApiFlow, FlowType,
};
use crate::app::get_tenant_app_state;
use crate::euclid::types::ExperimentEndpoint;
use crate::logger;
use serde::{Deserialize, Serialize};

const INFLIGHT_TTL_SECS: i64 = 3600;

/// One hash per payment, with a field per endpoint that routed it under the experiment. A payment
/// routed on several endpoints (e.g. `/routing/evaluate` then `/decide-gateway`) keeps one
/// decision record for each, so its outcome is attributed to every endpoint's arm.
fn inflight_key(payment_id: &str) -> String {
    format!("ab_test_inflight_by_endpoint:{}", payment_id)
}

async fn load_inflight(payment_id: &str) -> Vec<InflightContext> {
    let state = get_tenant_app_state().await;
    let fields = match state
        .redis_conn
        .hgetall_strings(&inflight_key(payment_id))
        .await
    {
        Ok(fields) => fields,
        Err(e) => {
            logger::warn!(
                "ab_test outcome: failed to load inflight context for {}: {:?}",
                payment_id,
                e
            );
            return Vec::new();
        }
    };
    fields
        .into_values()
        .filter_map(|raw| serde_json::from_str::<InflightContext>(&raw).ok())
        .collect()
}

async fn save_inflight(payment_id: &str, ctx: &InflightContext, action: &str) {
    let Ok(value) = serde_json::to_string(ctx) else {
        return;
    };
    let field = ctx.endpoint.unwrap_or(ExperimentEndpoint::Unknown).as_str();
    let state = get_tenant_app_state().await;
    if let Err(e) = state
        .redis_conn
        .hset_with_ttl(&inflight_key(payment_id), field, value, INFLIGHT_TTL_SECS)
        .await
    {
        logger::warn!(
            "ab_test outcome: failed to {} inflight context for {} on {}: {:?}",
            action,
            payment_id,
            field,
            e
        );
    }
}

/// Which experiment, arm and endpoint a payment's decision is attributed to.
#[derive(Debug, Clone)]
pub struct ExperimentAssignment {
    pub experiment_id: String,
    pub side: super::ArmSide,
    pub endpoint: ExperimentEndpoint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InflightContext {
    experiment_id: String,
    variant_arm: String,
    /// Endpoint the payment was routed on. Absent on records written before endpoint tagging.
    #[serde(default)]
    endpoint: Option<ExperimentEndpoint>,
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
    /// Payment amount, carried so the outcome event can value cost savings in money (TCS).
    #[serde(default)]
    amount: Option<f64>,
    /// True once the first attempt's outcome event has been emitted. Later attempts (gateway
    /// retries) emit follow-up events with `first_attempt = false`, so analytics can compute
    /// both FAAR (first-attempt auth rate) and NAR (net auth rate after retries).
    #[serde(default)]
    first_outcome_emitted: bool,
}

/// Whether the payment already had its first attempt's outcome recorded on `endpoint`. A payment
/// can be routed more than once (a merchant retry that re-calls `/decide-gateway`), and each
/// routing call's `store_inflight`/`record_cost_outcome` keeps this flag so a second attempt's
/// outcome is flagged `first_attempt = false` and FAAR counts the payment once.
async fn existing_first_outcome_emitted(payment_id: &str, endpoint: ExperimentEndpoint) -> bool {
    let state = get_tenant_app_state().await;
    state
        .redis_conn
        .hget::<InflightContext>(
            &inflight_key(payment_id),
            endpoint.as_str(),
            "ab_test_inflight",
        )
        .await
        .ok()
        .flatten()
        .is_some_and(|c| c.first_outcome_emitted)
}

pub async fn store_inflight(
    payment_id: &str,
    assignment: &ExperimentAssignment,
    gateway: Option<&str>,
    is_static_arm: bool,
    amount: Option<f64>,
) {
    write_inflight(
        payment_id,
        assignment,
        gateway,
        is_static_arm,
        CostOutcome::default(),
        amount,
        "store",
    )
    .await;
}

/// Multi-objective cost outcome of an SR-arm decision. All `None` when the arm ran auth-only.
#[derive(Debug, Clone, Default)]
pub struct CostOutcome {
    pub cost_saved_bps: Option<f64>,
    pub chosen_cost_bps: Option<f64>,
    pub margin: Option<f64>,
}

/// After routing completes for an SR-arm A/B payment, re-write the inflight record with the
/// now-known decided gateway and the multi-objective cost outcome (cost saved, chosen PSP cost,
/// margin). Lets the later outcome event (`emit_if_in_flight`) attribute cost per arm. Cost
/// fields are `None` when the arm ran auth-only (multi-objective off), in which case this still
/// backfills the decided gateway. Only ever called for SR arms, so `is_static_arm` stays false.
pub async fn record_cost_outcome(
    payment_id: &str,
    assignment: &ExperimentAssignment,
    gateway: Option<&str>,
    cost: CostOutcome,
    amount: Option<f64>,
) {
    write_inflight(
        payment_id, assignment, gateway, false, cost, amount, "enrich",
    )
    .await;
}

async fn write_inflight(
    payment_id: &str,
    assignment: &ExperimentAssignment,
    gateway: Option<&str>,
    is_static_arm: bool,
    cost: CostOutcome,
    amount: Option<f64>,
    action: &str,
) {
    let first_outcome_emitted =
        existing_first_outcome_emitted(payment_id, assignment.endpoint).await;
    let ctx = InflightContext {
        experiment_id: assignment.experiment_id.clone(),
        variant_arm: assignment.side.as_str().to_string(),
        endpoint: Some(assignment.endpoint),
        gateway: gateway.map(str::to_string),
        is_static_arm,
        cost_saved_bps: cost.cost_saved_bps,
        chosen_cost_bps: cost.chosen_cost_bps,
        margin: cost.margin,
        amount,
        first_outcome_emitted,
    };
    save_inflight(payment_id, &ctx, action).await;
}

/// Emits one outcome event per endpoint that routed the payment under the experiment.
pub async fn emit_if_in_flight(payment_id: &str, merchant_id: &str, is_success: bool) {
    let records = load_inflight(payment_id).await;
    if records.is_empty() {
        return;
    }

    // One outcome event per attempt: the first is flagged `first_attempt` (feeds FAAR), and on
    // failure the records are kept so a later retry can emit its own event (feeds NAR). The hash
    // is deleted on success — a payment resolves at most once — or by TTL for terminal failures.
    if is_success {
        let state = get_tenant_app_state().await;
        let _ = state.redis_conn.delete_key(&inflight_key(payment_id)).await;
    }

    for ctx in records {
        let first_attempt = !ctx.first_outcome_emitted;
        if !is_success && first_attempt {
            let next = InflightContext {
                first_outcome_emitted: true,
                ..ctx.clone()
            };
            save_inflight(payment_id, &next, "mark first outcome of").await;
        }
        emit_outcome_event(payment_id, merchant_id, is_success, first_attempt, ctx);
    }
}

fn emit_outcome_event(
    payment_id: &str,
    merchant_id: &str,
    is_success: bool,
    first_attempt: bool,
    ctx: InflightContext,
) {
    let status = if is_success { "success" } else { "failure" };
    let details = serialize_details(&serde_json::json!({
        "experiment_id": ctx.experiment_id,
        "variant_arm": ctx.variant_arm,
        "endpoint": ctx.endpoint.map(|e| e.as_str()),
        "preview_kind": "routing_evaluate_ab_test",
        "outcome_source": "score_update",
        "cost_saved_bps": ctx.cost_saved_bps,
        "chosen_cost_bps": ctx.chosen_cost_bps,
        "margin": ctx.margin,
        "amount": ctx.amount,
        "first_attempt": first_attempt,
    }));

    logger::debug!(
        "ab_test outcome: emitting {} for payment_id={} experiment={} arm={} endpoint={:?}",
        status,
        payment_id,
        ctx.experiment_id,
        ctx.variant_arm,
        ctx.endpoint
    );

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
}
