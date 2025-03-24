use std::sync::Arc;

use dynamo::{configs::GlobalSrConfig, utils::Encode};
use error_stack::ResultExt;

use rand::{rngs::StdRng, Rng, SeedableRng};

use crate::{
    app::AppState,
    errors::{ApiError, ContainerError},
    handlers::simulate::types::{
        AlgorithmType, BaselineData, CalculateSrConfigWrapper, ModelOutcome,
        SimulationOutcomeOfEachTxn, SrConfig, SuccessBasedRoutingResponse,
        UpdateWindowConfigWrapper,
    },
    headers_extractors::Headers,
    utils,
};

#[allow(clippy::too_many_arguments)]
pub async fn perform_success_based_routing(
    merchant_id: String,
    headers: Headers,
    model_config: SrConfig,
    connectors: Vec<String>,
    state: Arc<AppState>,
    baseline_data_result: &BaselineData,
    unique_param_suffix: String,
    algo_type: &AlgorithmType,
) -> Result<SuccessBasedRoutingResponse, ContainerError<ApiError>> {
    let num_of_payments = baseline_data_result.baseline_payments.len();
    let overall_baseline_success_rate = baseline_data_result.success_rate;
    let baseline_chunk_wise_sr = baseline_data_result.baseline_chunk_wise_sr.clone();

    let mut model_outcome_of_each_txn = Vec::with_capacity(baseline_data_result.total_attempts);

    let mut total_success_count = 0;
    let mut total_failed_payments = 0;
    let mut total_revenue = 0.0;
    let mut num_of_first_attempt_success = 0;

    let sr_algorithm = state.sr_algorithm.get_algo(algo_type);

    let tenant_id = Some(headers.tenant_id.clone());
    for chunk in baseline_chunk_wise_sr.chunk_wise_sr {
        let connectors = connectors.clone();
        let baseline_outcome_of_each_txn = baseline_data_result.baseline_payments.clone();

        let start = chunk.start;
        let end = chunk.end;
        let connectors_sr = chunk.connectors_sr;
        let mut rng = StdRng::from_entropy();
        let chunk_length = end.saturating_sub(start) + 1;
        let mut model_outcomes: Vec<ModelOutcome> = Vec::with_capacity(chunk_length);

        let cal_sr_config =
            CalculateSrConfigWrapper::from((model_config, GlobalSrConfig::default()))
                .0
                .encode_to_value()
                .change_context(ApiError::FailedToSerializeToJson)?;

        let update_win_config =
            UpdateWindowConfigWrapper::from((model_config, GlobalSrConfig::default()))
                .0
                .encode_to_value()
                .change_context(ApiError::FailedToSerializeToJson)?;

        for payment in baseline_outcome_of_each_txn
            .into_iter()
            .take(end + 1)
            .skip(start)
        {
            let mut model_simulated_payment_succeeded = false;

            for (attempt_no, attempt) in payment.payment_attempts.into_iter().enumerate() {
                if !model_simulated_payment_succeeded {
                    let param = attempt.params.get_concatenated(unique_param_suffix.clone());

                    let (_, suggested_connector) = sr_algorithm
                        .perform_routing(
                            &merchant_id,
                            &param,
                            connectors.clone(),
                            cal_sr_config.clone(),
                            &tenant_id,
                        )
                        .await
                        .change_context(ApiError::FailedToFetchSr)?
                        .first()
                        .ok_or(ApiError::Unexpected(
                            "Success rates returned from dynamo is empty",
                        ))?
                        .clone();

                    let baseline_selected_connector = &attempt.payment_gateway;
                    let baseline_selected_connector_status = attempt.payment_status;

                    let (processed_connector, processed_connector_status, suggested_uplift) = {
                        if baseline_selected_connector == &suggested_connector {
                            (
                                baseline_selected_connector.to_string(),
                                baseline_selected_connector_status,
                                0.0,
                            )
                        } else {
                            let suggested_connector_sr = connectors_sr.get(&suggested_connector);
                            if let Some(suggested_connector_sr) = suggested_connector_sr {
                                let baseline_connector_sr = connectors_sr
                                    .get(baseline_selected_connector)
                                    .ok_or(ApiError::Unexpected(
                                        "Failed to get SR for a baseline connector",
                                    ))?;
                                let simulated_status = rng.gen_bool(*suggested_connector_sr);
                                let suggested_uplift =
                                    suggested_connector_sr - baseline_connector_sr;

                                (suggested_connector, simulated_status, suggested_uplift)
                            } else {
                                (
                                    baseline_selected_connector.to_string(),
                                    baseline_selected_connector_status,
                                    0.0,
                                )
                            }
                        }
                    };

                    if processed_connector_status {
                        if attempt_no == 0 {
                            num_of_first_attempt_success += 1;
                        }

                        model_simulated_payment_succeeded = true;
                        total_success_count += 1;
                        total_revenue += attempt.amount;
                    } else {
                        total_failed_payments += 1;
                    }

                    model_outcomes.push(ModelOutcome {
                        payment_gateway: processed_connector.clone(),
                        status: processed_connector_status,
                        suggested_uplift,
                    });

                    let report = vec![(processed_connector, processed_connector_status)];

                    sr_algorithm
                        .update_window(
                            &merchant_id,
                            &param,
                            report,
                            update_win_config.clone(),
                            &tenant_id,
                        )
                        .await
                        .change_context(ApiError::FailedToUpdateWindow)?;
                } else {
                    model_outcomes.push(ModelOutcome {
                        payment_gateway: "N/A".to_string(),
                        status: false,
                        suggested_uplift: 0.0,
                    });
                }
            }
        }

        model_outcome_of_each_txn.extend(model_outcomes);
    }

    let overall_model_success_rate =
        (total_success_count as f64) / (baseline_data_result.total_attempts as f64);

    let improvement = (overall_model_success_rate - overall_baseline_success_rate)
        / overall_baseline_success_rate;

    let faar = (num_of_first_attempt_success as f64) / (num_of_payments as f64);

    let mut simulation_outcome_of_each_txn = Vec::with_capacity(num_of_payments);

    let mut txn_count = 0;

    for payment_index in 0..num_of_payments {
        let payment = baseline_data_result.baseline_payments[payment_index].clone();

        for attempt in payment.payment_attempts {
            simulation_outcome_of_each_txn.push(SimulationOutcomeOfEachTxn {
                txn_no: txn_count + 1,
                baseline_record: attempt,
                model_connector: model_outcome_of_each_txn[txn_count].payment_gateway.clone(),
                model_status: model_outcome_of_each_txn[txn_count].status,
                suggested_uplift: utils::to_percentage(
                    model_outcome_of_each_txn[txn_count].suggested_uplift,
                ),
            });
            txn_count += 1;
        }
    }

    Ok(SuccessBasedRoutingResponse {
        model_success_rate: overall_model_success_rate,
        total_failed_payments,
        total_revenue,
        faar,
        improvement,
        simulation_outcome_of_each_txn,
        total_txn_count: baseline_data_result.total_attempts,
    })
}
