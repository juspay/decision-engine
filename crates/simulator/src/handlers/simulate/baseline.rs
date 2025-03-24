use std::collections::{HashMap, HashSet};

use crate::{
    errors::{ApiError, ContainerError},
    handlers::simulate::types::{
        BaselineData, BaselineSrOfChunk, BaselineSrOfChunks, ConnectorStats, Payment,
    },
};

pub fn analyze_baseline_data(
    payments: Vec<Payment>,
    total_chunks: usize,
) -> Result<BaselineData, ContainerError<ApiError>> {
    let num_of_payments = payments.len();
    let mut num_of_attempts = 0;
    let mut num_of_first_attempt_success = 0;
    let mut total_revenue = 0.0;
    let mut total_failed_payments = 0;

    let each_chunk_size = (num_of_payments as f64 / total_chunks as f64).ceil() as usize;

    let mut chunk_wise_sr_of_connectors = BaselineSrOfChunks::default();
    let mut connectors = HashSet::new();

    let mut index = 0;
    let mut total_success_count = 0;
    let mut current_chunk_start_index = 0;

    for current_chunk_count in 0..total_chunks {
        let mut connectors_stats_of_current_chunk = HashMap::new();

        let current_chunk_end_index = if current_chunk_count == total_chunks - 1 {
            num_of_payments
        } else {
            std::cmp::min(index + each_chunk_size, num_of_payments)
        };

        while index < current_chunk_end_index {
            let payment = &payments[index];
            num_of_first_attempt_success += payment.first_attempt_payment_status as usize;

            for attempt in &payment.payment_attempts {
                num_of_attempts += 1;

                let connector_stats = connectors_stats_of_current_chunk
                    .entry(attempt.payment_gateway.clone())
                    .or_insert(ConnectorStats::default());

                if attempt.payment_status {
                    connector_stats.success_count += 1;
                    total_success_count += 1;
                    total_revenue += attempt.amount;
                } else {
                    total_failed_payments += 1;
                }

                connector_stats.total_count += 1;
            }

            index += 1;
        }

        let mut connectors_sr_of_current_chunk = HashMap::new();
        connectors_stats_of_current_chunk
            .into_iter()
            .for_each(|(connector, stats)| {
                connectors_sr_of_current_chunk.insert(
                    connector,
                    stats.success_count as f64 / stats.total_count as f64,
                );
            });

        connectors.extend(connectors_sr_of_current_chunk.keys().cloned());

        chunk_wise_sr_of_connectors.push(BaselineSrOfChunk {
            start: current_chunk_start_index,
            end: current_chunk_end_index.saturating_sub(1),
            connectors_sr: connectors_sr_of_current_chunk,
        });

        current_chunk_start_index = index;
    }

    let overall_success_rate = (total_success_count as f64) / (num_of_attempts as f64);
    let faar = (num_of_first_attempt_success as f64) / (num_of_payments as f64);

    let chunk_wise_sr_of_connectors = if payments.is_empty() {
        BaselineSrOfChunks::default()
    } else {
        chunk_wise_sr_of_connectors
    };

    Ok(BaselineData {
        connectors: connectors.into_iter().collect(),
        total_attempts: num_of_attempts,
        success_rate: overall_success_rate,
        total_failed_payments,
        total_revenue,
        faar,
        baseline_chunk_wise_sr: chunk_wise_sr_of_connectors,
        baseline_payments: payments,
    })
}
