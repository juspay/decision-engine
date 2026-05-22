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

#[derive(Debug, Clone, Deserialize, Row)]
struct ArmRow {
    arm: String,
    total: u64,
    success_count: u64,
    failure_count: u64,
    avg_latency_ms: Option<f64>,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &ExperimentResultsQuery,
) -> Result<ExperimentResultsResponse, ApiError> {
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);

    builder.extend_selects([
        "JSONExtractString(assumeNotNull(details), 'variant_arm') AS arm".to_string(),
        // Each payment produces a routing event (status = 'ab_test_decision') later
        // replaced by an outcome event (status = 'success'/'failure') via ReplacingMergeTree.
        // count() therefore gives one row per payment after deduplication.
        "count() AS total".to_string(),
        "countIf(lowerUTF8(status) = 'success') AS success_count".to_string(),
        "countIf(lowerUTF8(status) = 'failure') AS failure_count".to_string(),
        "avgIf(average_latency, average_latency > 0) AS avg_latency_ms".to_string(),
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
        "JSONExtractString(assumeNotNull(details), 'variant_arm') IN ('control', 'variant')".to_string(),
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

    let control = arm_metrics("control", control_row);
    let variant = arm_metrics("variant", variant_row);
    let total = control.transaction_count + variant.transaction_count;

    let delta_pp = (variant.auth_rate - control.auth_rate) * 100.0;
    let (p_value, confidence_interval, verdict) = compute_significance(
        &control,
        &variant,
        total,
        query.min_sample_size,
        query.guardrail_threshold_pp,
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
    })
}

fn arm_metrics(arm: &str, row: Option<&ArmRow>) -> ExperimentArmMetrics {
    match row {
        None => ExperimentArmMetrics {
            arm: arm.to_string(),
            transaction_count: 0,
            success_count: 0,
            failure_count: 0,
            auth_rate: 0.0,
            avg_latency_ms: None,
        },
        Some(r) => {
            let auth_rate = if r.total > 0 {
                r.success_count as f64 / r.total as f64
            } else {
                0.0
            };
            ExperimentArmMetrics {
                arm: arm.to_string(),
                transaction_count: r.total as i64,
                success_count: r.success_count as i64,
                failure_count: r.failure_count as i64,
                auth_rate,
                avg_latency_ms: r.avg_latency_ms.filter(|&v| v > 0.0),
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
) -> (Option<f64>, Option<(f64, f64)>, ExperimentVerdict) {
    let n_c = control.transaction_count as f64;
    let n_v = variant.transaction_count as f64;

    if total < min_sample_size as i64 || n_c == 0.0 || n_v == 0.0 {
        return (None, None, ExperimentVerdict::CollectingData);
    }

    let p_c = control.auth_rate;
    let p_v = variant.auth_rate;
    let delta = p_v - p_c;

    // Guardrail check: if variant is degraded beyond threshold, flag immediately.
    if (p_c - p_v) * 100.0 > guardrail_threshold_pp {
        return (None, None, ExperimentVerdict::GuardrailBreached);
    }

    let k_c = control.success_count as f64;
    let k_v = variant.success_count as f64;
    let p_pool = (k_c + k_v) / (n_c + n_v);
    let se = (p_pool * (1.0 - p_pool) * (1.0 / n_c + 1.0 / n_v)).sqrt();

    if se == 0.0 {
        return (None, None, ExperimentVerdict::NotSignificant);
    }

    let z = delta / se;
    let p_value = two_tailed_p_value(z);

    // 95% confidence interval
    let margin = 1.96 * se;
    let ci = (delta - margin, delta + margin);

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
            + t * (-0.284496736
                + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    poly * (-x * x).exp()
}
