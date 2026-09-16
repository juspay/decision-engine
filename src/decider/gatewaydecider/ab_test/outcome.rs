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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InflightContext {
    experiment_id: String,
    variant_arm: String,
    gateway: Option<String>,
    /// Whether the dynamic decider scored this payment. False only for a rule-only arm, whose
    /// gateway was picked by the rule and which therefore has no SR score update to report.
    #[serde(default)]
    sr_scored: Option<bool>,
    /// The field `sr_scored` replaced, read so records written before the rename keep their
    /// meaning for the rest of their hour-long TTL. Never written.
    #[serde(default, skip_serializing)]
    is_static_arm: Option<bool>,
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

impl InflightContext {
    fn sr_scored(&self) -> bool {
        self.sr_scored
            .or_else(|| self.is_static_arm.map(|is_static| !is_static))
            .unwrap_or(true)
    }
}

/// The experiment record for a payment already routed under an active experiment, if any.
/// A payment can be routed more than once (a merchant retry that re-calls `/decide-gateway`),
/// and the later calls read this to carry forward what the first one established.
async fn load_inflight(payment_id: &str) -> Option<InflightContext> {
    let state = get_tenant_app_state().await;
    state
        .redis_conn
        .get_key(&inflight_key(payment_id), "ab_test_inflight")
        .await
        .ok()
        .flatten()
}

/// Whether this payment's gateway was picked by a rule alone, with the dynamic decider never
/// running. Those payments have no SR score update to report, so the caller suppresses the
/// `UpdateGatewayScoreUpdate` audit event for them; every other payment — including a hybrid
/// arm, where a rule narrowed the field but SR still chose — was really scored and keeps it.
pub async fn is_rule_only_arm_inflight(payment_id: &str) -> bool {
    load_inflight(payment_id)
        .await
        .map(|ctx| !ctx.sr_scored())
        .unwrap_or(false)
}

pub async fn store_inflight(
    payment_id: &str,
    experiment_id: &str,
    variant_arm: &str,
    gateway: Option<&str>,
    sr_scored: bool,
    amount: Option<f64>,
) {
    let existing = load_inflight(payment_id).await;
    let state = get_tenant_app_state().await;
    let key = inflight_key(payment_id);
    let ctx = InflightContext {
        experiment_id: experiment_id.to_string(),
        // The arm a payment was first routed under is the arm it stays in. Assignment is a
        // function of the split, so raising `variant_split_pct` mid-experiment moves some
        // payments from control to variant — a retry after that ramp must not re-file the
        // payment's outcome under an arm its first attempt never ran.
        variant_arm: existing
            .as_ref()
            .map(|prev| prev.variant_arm.clone())
            .unwrap_or_else(|| variant_arm.to_string()),
        gateway: gateway.map(str::to_string),
        sr_scored: Some(sr_scored),
        is_static_arm: None,
        cost_saved_bps: None,
        chosen_cost_bps: None,
        margin: None,
        amount,
        first_outcome_emitted: existing
            .as_ref()
            .map(|prev| prev.first_outcome_emitted)
            .unwrap_or(false),
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
/// backfills the decided gateway. Only ever called for arms the decider ran, so `sr_scored`
/// stays true.
#[allow(clippy::too_many_arguments)]
pub async fn record_cost_outcome(
    payment_id: &str,
    experiment_id: &str,
    variant_arm: &str,
    gateway: Option<&str>,
    cost_saved_bps: Option<f64>,
    chosen_cost_bps: Option<f64>,
    margin: Option<f64>,
    amount: Option<f64>,
) {
    let existing = load_inflight(payment_id).await;
    let state = get_tenant_app_state().await;
    let key = inflight_key(payment_id);
    let ctx = InflightContext {
        experiment_id: experiment_id.to_string(),
        variant_arm: existing
            .as_ref()
            .map(|prev| prev.variant_arm.clone())
            .unwrap_or_else(|| variant_arm.to_string()),
        gateway: gateway.map(str::to_string),
        sr_scored: Some(true),
        is_static_arm: None,
        cost_saved_bps,
        chosen_cost_bps,
        margin,
        amount,
        first_outcome_emitted: existing
            .as_ref()
            .map(|prev| prev.first_outcome_emitted)
            .unwrap_or(false),
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

    // One outcome event per attempt: the first is flagged `first_attempt` (feeds FAAR), and on
    // failure the key is kept alive so a later retry can emit its own event (feeds NAR). The key
    // is deleted on success — a payment resolves at most once — or by TTL for terminal failures.
    let first_attempt = !ctx.first_outcome_emitted;
    if is_success {
        let _ = state.redis_conn.delete_key(&key).await;
    } else {
        let next = InflightContext {
            first_outcome_emitted: true,
            ..ctx.clone()
        };
        if let Err(e) = state
            .redis_conn
            .set_key_with_ttl(&key, next, INFLIGHT_TTL_SECS)
            .await
        {
            logger::warn!(
                "ab_test outcome: failed to mark first outcome for {}: {:?}",
                payment_id,
                e
            );
        }
    }

    let status = if is_success { "success" } else { "failure" };
    let details = serialize_details(&serde_json::json!({
        "experiment_id": ctx.experiment_id,
        "variant_arm": ctx.variant_arm,
        "preview_kind": "routing_evaluate_ab_test",
        "outcome_source": "score_update",
        "cost_saved_bps": ctx.cost_saved_bps,
        "chosen_cost_bps": ctx.chosen_cost_bps,
        "margin": ctx.margin,
        "amount": ctx.amount,
        "first_attempt": first_attempt,
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
