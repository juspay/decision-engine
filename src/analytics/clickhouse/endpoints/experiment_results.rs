use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::clickhouse::common::{fetch_all, DOMAIN_TABLE};
use crate::analytics::clickhouse::filters::merchant_filter;
use crate::analytics::clickhouse::query::{BoundQueryBuilder, FilterClause, OrderClause};
use crate::analytics::flow::FlowType;
use crate::analytics::models::{
    ExperimentArmMetrics, ExperimentResultsQuery, ExperimentResultsResponse, ExperimentVerdict,
};
use crate::error::ApiError;

/// Default business margin (fraction of ticket) used to value net EV when the caller does not
/// pass one. Matches the routing-time `DEFAULT_MARGIN`; for an accurate net verdict the UI should
/// pass the merchant's real margin.
pub const DEFAULT_EVALUATION_MARGIN: f64 = 1.0;

#[derive(Debug, Clone, Deserialize, Row)]
struct ArmRow {
    arm: String,
    total: u64,
    success_count: u64,
    failure_count: u64,
    avg_latency_ms: Option<f64>,
    /// Averages over the outcome events that carried multi-objective cost data. These come from
    /// `avgIf(JSONExtractFloat(...))`, which is a NON-nullable `Float64` (returns `nan` for an
    /// empty set) — so they must be `f64`, not `Option<f64>`, or RowBinary deserialization fails.
    /// `cost_event_count == 0` (or a non-finite average) means "no cost data" — handled below.
    avg_chosen_cost_bps: f64,
    avg_cost_saved_bps: f64,
    cost_event_count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &ExperimentResultsQuery,
) -> Result<ExperimentResultsResponse, ApiError> {
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);

    builder.extend_selects([
        "JSONExtractString(assumeNotNull(details), 'variant_arm') AS arm".to_string(),
        // Each payment emits two events: a routing event (status = 'ab_test_decision')
        // and an outcome event (status = 'success'/'failure'). count() would double-count
        // every resolved payment. Use uniqExact(payment_id) to count each payment once.
        "uniqExact(payment_id) AS total".to_string(),
        "uniqExactIf(payment_id, lowerUTF8(status) = 'success') AS success_count".to_string(),
        "uniqExactIf(payment_id, lowerUTF8(status) = 'failure') AS failure_count".to_string(),
        "avgIf(average_latency, average_latency > 0) AS avg_latency_ms".to_string(),
        // Cost is carried only on the outcome event (JSON `details`), and only multi-objective
        // arms have a positive chosen_cost_bps (auth-only arms emit null → 0). Average chosen cost
        // over those rows; average cost_saved over the same rows (AuthWon rows contribute 0 saved),
        // and count them so we can tell whether any cost data exists for the arm.
        "avgIf(JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps'), JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps') > 0) AS avg_chosen_cost_bps".to_string(),
        "avgIf(JSONExtractFloat(assumeNotNull(details), 'cost_saved_bps'), JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps') > 0) AS avg_cost_saved_bps".to_string(),
        "countIf(JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps') > 0) AS cost_event_count".to_string(),
    ]);

    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::RoutingEvaluateAbTest.as_str()
    )));
    builder.add_filter(FilterClause::raw(format!(
        "JSONExtractString(assumeNotNull(details), 'experiment_id') = '{}'",
        query.experiment_id.replace('\'', "\\'")
    )));
    builder.add_filter(FilterClause::raw(
        "JSONExtractString(assumeNotNull(details), 'variant_arm') IN ('control', 'variant')"
            .to_string(),
    ));

    if let Some(start) = query.start_ms {
        builder.add_filter(FilterClause::gte("created_at_ms", start));
    }
    if let Some(end) = query.end_ms {
        builder.add_filter(FilterClause::lte("created_at_ms", end));
    }

    builder.add_group_by("arm");
    builder.add_order_by(OrderClause::asc("arm"));

    let rows = fetch_all::<ArmRow>(builder.build(client)).await?;

    let control_row = rows.iter().find(|r| r.arm == "control");
    let variant_row = rows.iter().find(|r| r.arm == "variant");

    let control = arm_metrics("control", control_row, query.evaluation_margin);
    let variant = arm_metrics("variant", variant_row, query.evaluation_margin);
    let total = control.transaction_count + variant.transaction_count;

    let delta_pp = (variant.auth_rate - control.auth_rate) * 100.0;

    // Net economic value delta (bps of ticket). Two cases:
    //  - Both arms recorded cost (e.g. Autopilot value, both cost-on): exact delta of net EVs.
    //  - Only the variant recorded cost (Turn cost on: control is auth-only, no cost captured):
    //    approximate the control's cost by the variant's SR-head cost (avg_chosen + avg_saved) —
    //    the SR head is the gateway the control also routes to — so the cost benefit of turning
    //    cost on is still valued against the auth it traded. `None` for auth-only experiments.
    let net_delta_bps = match (control.net_ev_bps, variant.net_ev_bps) {
        (Some(c), Some(v)) => Some(v - c),
        (None, Some(v)) => match (variant.avg_chosen_cost_bps, variant.avg_cost_saved_bps) {
            (Some(chosen), Some(saved)) => {
                let control_implied_cost = chosen + saved;
                let control_net =
                    control.auth_rate * (query.evaluation_margin * 10_000.0 - control_implied_cost);
                Some(v - control_net)
            }
            _ => None,
        },
        _ => None,
    };

    let (p_value, confidence_interval, verdict) = compute_significance(
        &control,
        &variant,
        total,
        query.min_sample_size,
        query.guardrail_threshold_pp,
        net_delta_bps,
    );

    Ok(ExperimentResultsResponse {
        experiment_id: query.experiment_id.clone(),
        merchant_id: query.merchant_id.clone(),
        control,
        variant,
        delta_pp,
        p_value,
        confidence_interval,
        verdict,
        min_sample_size: query.min_sample_size,
        net_delta_bps,
        evaluation_margin: query.evaluation_margin,
    })
}

fn arm_metrics(arm: &str, row: Option<&ArmRow>, evaluation_margin: f64) -> ExperimentArmMetrics {
    match row {
        None => ExperimentArmMetrics {
            arm: arm.to_string(),
            transaction_count: 0,
            success_count: 0,
            failure_count: 0,
            auth_rate: 0.0,
            avg_latency_ms: None,
            avg_chosen_cost_bps: None,
            avg_cost_saved_bps: None,
            net_ev_bps: None,
        },
        Some(r) => {
            let auth_rate = if r.total > 0 {
                r.success_count as f64 / r.total as f64
            } else {
                0.0
            };
            // Cost data present only when at least one outcome event carried a chosen cost.
            // avgIf over an empty set is `nan`, so guard on both the count and finiteness.
            let has_cost = r.cost_event_count > 0;
            let avg_chosen_cost_bps =
                Some(r.avg_chosen_cost_bps).filter(|v| has_cost && v.is_finite());
            let avg_cost_saved_bps =
                Some(r.avg_cost_saved_bps).filter(|v| has_cost && v.is_finite());
            // Net economic value per attempt (bps of ticket) at the common margin M:
            //   auth_rate · (M·10_000 − avg_chosen_cost_bps)
            let net_ev_bps = avg_chosen_cost_bps
                .map(|cost| auth_rate * (evaluation_margin * 10_000.0 - cost));
            ExperimentArmMetrics {
                arm: arm.to_string(),
                transaction_count: r.total as i64,
                success_count: r.success_count as i64,
                failure_count: r.failure_count as i64,
                auth_rate,
                avg_latency_ms: r.avg_latency_ms.filter(|&v| v > 0.0),
                avg_chosen_cost_bps,
                avg_cost_saved_bps,
                net_ev_bps,
            }
        }
    }
}

fn compute_significance(
    control: &ExperimentArmMetrics,
    variant: &ExperimentArmMetrics,
    total: i64,
    min_sample_size: u32,
    guardrail_threshold_pp: f64,
    // Present for cost/autopilot experiments; when Some, the winner is judged on net economic
    // value rather than auth alone (the auth z-test still gates the auth guardrail).
    net_delta_bps: Option<f64>,
) -> (Option<f64>, Option<(f64, f64)>, ExperimentVerdict) {
    let n_c = control.transaction_count as f64;
    let n_v = variant.transaction_count as f64;

    if total < min_sample_size as i64 || n_c == 0.0 || n_v == 0.0 {
        return (None, None, ExperimentVerdict::CollectingData);
    }

    let p_c = control.auth_rate;
    let p_v = variant.auth_rate;
    let delta = p_v - p_c;

    // Guardrail check: if variant auth is degraded beyond threshold, flag immediately — cost
    // savings never justify an auth drop past the safety guardrail.
    if (p_c - p_v) * 100.0 > guardrail_threshold_pp {
        return (None, None, ExperimentVerdict::GuardrailBreached);
    }

    let k_c = control.success_count as f64;
    let k_v = variant.success_count as f64;
    let p_pool = (k_c + k_v) / (n_c + n_v);
    let se = (p_pool * (1.0 - p_pool) * (1.0 / n_c + 1.0 / n_v)).sqrt();

    let (p_value, ci) = if se == 0.0 {
        (None, None)
    } else {
        let z = delta / se;
        let margin = 1.96 * se;
        // Report the CI in percentage points to match `delta_pp` and the UI's "pp" label
        // (delta/margin are proportions here, so ×100). Fixes the CI showing 100× too small.
        (
            Some(two_tailed_p_value(z)),
            Some(((delta - margin) * 100.0, (delta + margin) * 100.0)),
        )
    };

    // Cost/autopilot experiment: judge on net economic value. v1 uses the net-delta point
    // estimate (past the auth guardrail above); a rigorous test on the continuous net metric
    // would need a per-transaction value distribution, which the aggregate query doesn't carry.
    if let Some(net) = net_delta_bps {
        let verdict = if net > 0.0 {
            ExperimentVerdict::VariantWins
        } else if net < 0.0 {
            ExperimentVerdict::VariantLoses
        } else {
            ExperimentVerdict::NotSignificant
        };
        return (p_value, ci, verdict);
    }

    // Auth-only experiment: two-proportion z-test on auth rate.
    let verdict = match p_value {
        Some(p) if p < 0.05 => {
            if delta > 0.0 {
                ExperimentVerdict::VariantWins
            } else {
                ExperimentVerdict::VariantLoses
            }
        }
        Some(_) => ExperimentVerdict::NotSignificant,
        None => ExperimentVerdict::NotSignificant,
    };

    (p_value, ci, verdict)
}

/// Two-tailed p-value from z-score.
/// Uses the Abramowitz & Stegun erfc approximation (7.1.26), max error < 1.5e-7.
fn two_tailed_p_value(z: f64) -> f64 {
    erfc_approx(z.abs() / std::f64::consts::SQRT_2)
}

fn erfc_approx(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    poly * (-x * x).exp()
}
