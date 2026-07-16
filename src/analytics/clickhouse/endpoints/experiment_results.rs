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

/// Default business margin (fraction of ticket) used to value EV when the caller does not
/// pass one. Matches the routing-time `DEFAULT_MARGIN`; for an accurate verdict the UI should
/// pass the merchant's real margin.
pub const DEFAULT_EVALUATION_MARGIN: f64 = 1.0;

#[derive(Debug, Clone, Deserialize, Row)]
struct ArmRow {
    arm: String,
    total: u64,
    success_count: u64,
    /// Payments with at least one terminal outcome (success or failure).
    resolved_count: u64,
    avg_latency_ms: Option<f64>,
    /// Averages over the outcome events that carried multi-objective cost data. These come from
    /// `avgIf(JSONExtractFloat(...))`, which is a NON-nullable `Float64` (returns `nan` for an
    /// empty set) — so they must be `f64`, not `Option<f64>`, or RowBinary deserialization fails.
    /// `cost_event_count == 0` (or a non-finite average) means "no cost data" — handled below.
    avg_chosen_cost_bps: f64,
    avg_cost_saved_bps: f64,
    cost_event_count: u64,
    /// Σ saved and Σ saved² over SUCCESSFUL outcomes (missing key extracts as 0, and sumIf over
    /// an empty set is 0 — safe as plain f64). These feed the per-transaction EV variance for
    /// the z-test: v = success ? (margin·10⁴ + saved_bps) : 0. Saved is measured against the SR
    /// head (the same gateway a cost-blind arm would have used), so a rule/auth arm's saved ≡ 0
    /// is a true fact ("never ran cost routing"), not an approximation — unlike absolute cost,
    /// which would require charging one arm a real fee while crediting the other with none.
    saved_success_sum: f64,
    saved_success_sq_sum: f64,
    /// Payments whose FIRST attempt succeeded (FAAR numerator).
    first_attempt_success_count: u64,
    /// Fees saved in money on successful payments: Σ (saved_bps/10⁴)·amount.
    total_cost_saved: f64,
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
        // NAR numerator: a payment counts as success if ANY attempt succeeded (retries emit
        // follow-up outcome events). resolved = payments with at least one terminal outcome;
        // eventual failures = resolved − successes (computed in Rust), so a payment that failed
        // then succeeded on retry is not double-counted as a failure.
        "uniqExactIf(payment_id, lowerUTF8(status) = 'success') AS success_count".to_string(),
        "uniqExactIf(payment_id, lowerUTF8(status) IN ('success', 'failure')) AS resolved_count".to_string(),
        "avgIf(average_latency, average_latency > 0) AS avg_latency_ms".to_string(),
        // Cost is carried only on the outcome event (JSON `details`), and only multi-objective
        // arms have a positive chosen_cost_bps (auth-only arms emit null → 0). Average chosen cost
        // over those rows; average cost_saved over the same rows (AuthWon rows contribute 0 saved),
        // and count them so we can tell whether any cost data exists for the arm.
        "avgIf(JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps'), JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps') > 0) AS avg_chosen_cost_bps".to_string(),
        "avgIf(JSONExtractFloat(assumeNotNull(details), 'cost_saved_bps'), JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps') > 0) AS avg_cost_saved_bps".to_string(),
        "countIf(JSONExtractFloat(assumeNotNull(details), 'chosen_cost_bps') > 0) AS cost_event_count".to_string(),
        // Savings moments over successful outcomes — a fee is only actually saved when the
        // payment succeeds. Together with success_count these give mean and variance of the
        // per-transaction value for the EV z-test. Arms that never ran cost routing carry no
        // cost_saved_bps, which extracts as 0 — correctly, since they saved nothing.
        "sumIf(JSONExtractFloat(assumeNotNull(details), 'cost_saved_bps'), lowerUTF8(status) = 'success') AS saved_success_sum".to_string(),
        "sumIf(pow(JSONExtractFloat(assumeNotNull(details), 'cost_saved_bps'), 2), lowerUTF8(status) = 'success') AS saved_success_sq_sum".to_string(),
        // FAAR numerator: success on the FIRST attempt. Events emitted before first-attempt
        // tracking existed lack the key — those were only ever emitted for the first attempt,
        // so a missing key counts as first attempt.
        "uniqExactIf(payment_id, lowerUTF8(status) = 'success' AND (JSONExtractBool(assumeNotNull(details), 'first_attempt') OR NOT JSONHas(assumeNotNull(details), 'first_attempt'))) AS first_attempt_success_count".to_string(),
        // TCS: fees saved in money, realized on successful payments. Events without an amount
        // (pre-tracking) contribute 0.
        "sumIf((JSONExtractFloat(assumeNotNull(details), 'cost_saved_bps') / 10000.0) * JSONExtractFloat(assumeNotNull(details), 'amount'), lowerUTF8(status) = 'success') AS total_cost_saved".to_string(),
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

    // EV delta (bps of ticket) — the quantity the z-test below runs on: EV = mean of
    // success·(margin·10⁴ + saved_bps). Arms without cost data extract saved as 0 (they truly
    // saved nothing — no absolute-cost asymmetry between arms). Surfaced in the response only
    // for cost experiments (at least one arm ran multi-objective); for auth-only experiments the
    // same test collapses to a pure auth z-test and the UI shows the auth delta instead.
    let is_cost_experiment =
        control.avg_cost_saved_bps.is_some() || variant.avg_cost_saved_bps.is_some();
    let net_delta_bps = match (is_cost_experiment, control.net_ev_bps, variant.net_ev_bps) {
        (true, Some(c), Some(v)) => Some(v - c),
        _ => None,
    };

    let (p_value, confidence_interval, verdict) = compute_significance(
        &control,
        &variant,
        ev_stats(control_row, query.evaluation_margin),
        ev_stats(variant_row, query.evaluation_margin),
        total,
        query.min_sample_size,
        query.guardrail_threshold_pp,
        is_cost_experiment,
        query.evaluation_margin,
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
            first_attempt_auth_rate: 0.0,
            total_cost_saved: None,
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
            let first_attempt_auth_rate = if r.total > 0 {
                r.first_attempt_success_count as f64 / r.total as f64
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
            // Economic value per transaction (bps of ticket) at the common margin M: the mean of
            //   v = success · (M·10_000 + saved_bps)
            // over every transaction in the arm — a success earns the margin plus whatever fee
            // it saved versus the SR head (the gateway a cost-blind arm would have used on that
            // same transaction); a failure earns 0. Arms that never ran cost routing record no
            // cost_saved_bps (extracted as 0), so their EV is auth_rate · M — a true statement
            // ("saved nothing"), not an approximation of an unmeasured absolute fee.
            let net_ev_bps = (r.total > 0).then(|| {
                (evaluation_margin * 10_000.0 * r.success_count as f64 + r.saved_success_sum)
                    / r.total as f64
            });
            ExperimentArmMetrics {
                arm: arm.to_string(),
                transaction_count: r.total as i64,
                success_count: r.success_count as i64,
                // Eventual failures: resolved payments that never succeeded on any attempt.
                failure_count: r.resolved_count.saturating_sub(r.success_count) as i64,
                auth_rate,
                first_attempt_auth_rate,
                // TCS is only meaningful for arms that ran cost routing; rule/auth arms save 0.
                total_cost_saved: has_cost.then_some(r.total_cost_saved),
                avg_latency_ms: r.avg_latency_ms.filter(|&v| v > 0.0),
                avg_chosen_cost_bps,
                avg_cost_saved_bps,
                net_ev_bps,
            }
        }
    }
}

/// Per-arm moments of the per-transaction value `v = success · (margin·10⁴ + saved_bps)` —
/// the distribution the EV z-test runs on. `None` when the arm has fewer than 2 transactions
/// (variance undefined).
struct ArmEvStats {
    n: f64,
    mean: f64,
    var: f64,
}

fn ev_stats(row: Option<&ArmRow>, evaluation_margin: f64) -> Option<ArmEvStats> {
    let r = row?;
    if r.total < 2 {
        return None;
    }
    let n = r.total as f64;
    let m4 = evaluation_margin * 10_000.0;
    let k = r.success_count as f64;
    let mean = (m4 * k + r.saved_success_sum) / n;
    // Σv² over the arm: failures contribute 0; each success contributes (m4 + saved)².
    let sum_sq = m4 * m4 * k + 2.0 * m4 * r.saved_success_sum + r.saved_success_sq_sum;
    let var = ((sum_sq - n * mean * mean) / (n - 1.0)).max(0.0);
    Some(ArmEvStats { n, mean, var })
}

#[allow(clippy::too_many_arguments)]
fn compute_significance(
    control: &ExperimentArmMetrics,
    variant: &ExperimentArmMetrics,
    control_stats: Option<ArmEvStats>,
    variant_stats: Option<ArmEvStats>,
    total: i64,
    min_sample_size: u32,
    guardrail_threshold_pp: f64,
    is_cost_experiment: bool,
    evaluation_margin: f64,
) -> (Option<f64>, Option<(f64, f64)>, ExperimentVerdict) {
    let n_c = control.transaction_count as f64;
    let n_v = variant.transaction_count as f64;

    if total < min_sample_size as i64 || n_c == 0.0 || n_v == 0.0 {
        return (None, None, ExperimentVerdict::CollectingData);
    }

    // Guardrail check: if variant auth is degraded beyond threshold, flag immediately — cost
    // savings never justify an auth drop past the safety guardrail. Deliberately a point
    // estimate with no significance test (safety should trip eagerly); the CollectingData gate
    // above keeps it from firing on a handful of transactions.
    if (control.auth_rate - variant.auth_rate) * 100.0 > guardrail_threshold_pp {
        return (None, None, ExperimentVerdict::GuardrailBreached);
    }

    // EV z-test: two-sample z on the per-transaction value v = success·(margin·10⁴ + saved_bps).
    // For arms that never ran cost routing saved ≡ 0, so v = margin·10⁴·success and this reduces
    // exactly to the (unpooled) two-proportion auth z-test — one test covers all six experiment
    // shapes. The verdict requires significance: a positive EV delta inside the noise band stays
    // NotSignificant instead of prematurely declaring a winner.
    let (Some(c), Some(v)) = (control_stats, variant_stats) else {
        return (None, None, ExperimentVerdict::NotSignificant);
    };
    let delta = v.mean - c.mean;
    let se = (c.var / c.n + v.var / v.n).sqrt();
    if se == 0.0 || !se.is_finite() {
        return (None, None, ExperimentVerdict::NotSignificant);
    }

    let z = delta / se;
    let p_value = two_tailed_p_value(z);
    let half = 1.96 * se;
    // CI units follow what the UI shows: bps of EV delta for cost experiments; auth percentage
    // points for auth-only experiments (exact conversion there — saved ≡ 0 means the EV delta
    // is margin·10⁴ × Δauth, so pp = bps / (margin·100)).
    let ci = if is_cost_experiment {
        (delta - half, delta + half)
    } else {
        let to_pp = evaluation_margin * 100.0;
        ((delta - half) / to_pp, (delta + half) / to_pp)
    };

    let verdict = if p_value < 0.05 {
        if delta > 0.0 {
            ExperimentVerdict::VariantWins
        } else {
            ExperimentVerdict::VariantLoses
        }
    } else {
        ExperimentVerdict::NotSignificant
    };

    (Some(p_value), Some(ci), verdict)
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
