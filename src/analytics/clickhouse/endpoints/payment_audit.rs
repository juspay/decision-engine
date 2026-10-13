use crate::analytics::flow::AnalyticsRoute;
use crate::analytics::models::{
    PaymentAuditOutcome, PaymentAuditQuery, PaymentAuditResponse, PaymentAuditScope,
};
use crate::error::ApiError;

use super::super::metrics;
use super::super::time::payment_audit_range;

pub async fn load(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Result<PaymentAuditResponse, ApiError> {
    let requested_lookup_key = crate::analytics::derive_lookup_key(
        query.payment_id.as_deref(),
        query.request_id.as_deref(),
    );

    let (total_results, total_success, total_failure, mut results) =
        if let Some(lookup_key) = requested_lookup_key.clone() {
            let results = metrics::audit_summaries::load_exact(client, query, scope, &lookup_key)
                .await
                .map_err(|error| {
                    crate::logger::error!(
                        ?error,
                        ?scope,
                        lookup_key,
                        merchant_id = %query.merchant_id,
                        payment_id = ?query.payment_id,
                        request_id = ?query.request_id,
                        "payment audit exact summary load failed"
                    );
                    error
                })?;
            let success = results
                .iter()
                .filter(|r| r.outcome == PaymentAuditOutcome::Success)
                .count();
            let failure = results
                .iter()
                .filter(|r| r.outcome == PaymentAuditOutcome::Failure)
                .count();
            (results.len(), success, failure, results)
        } else {
            let (total_results, total_success, total_failure) =
                metrics::audit_summaries::count(client, query, scope)
                    .await
                    .map_err(|error| {
                        crate::logger::error!(
                            ?error,
                            ?scope,
                            merchant_id = %query.merchant_id,
                            payment_id = ?query.payment_id,
                            request_id = ?query.request_id,
                            "payment audit summary count failed"
                        );
                        error
                    })?;
            let results = metrics::audit_summaries::load_page(client, query, scope)
                .await
                .map_err(|error| {
                    crate::logger::error!(
                        ?error,
                        ?scope,
                        merchant_id = %query.merchant_id,
                        payment_id = ?query.payment_id,
                        request_id = ?query.request_id,
                        "payment audit summary page load failed"
                    );
                    error
                })?;
            (total_results, total_success, total_failure, results)
        };
    let page = query.page;
    let page_size = query.page_size;
    let exact_lookup = requested_lookup_key.is_some();
    let selected_lookup_key = results
        .first()
        .map(|row| row.lookup_key.clone())
        .or(requested_lookup_key);

    // The timeline belongs to the list's first result, so later pages of the same list don't
    // re-read it.
    let timeline_lookup_key = if exact_lookup || page == 1 {
        selected_lookup_key.clone()
    } else {
        None
    };
    let timeline = if let Some(lookup_key) = timeline_lookup_key.as_deref() {
        metrics::audit_timeline::load(client, query, scope, lookup_key)
            .await
            .map_err(|error| {
                crate::logger::error!(
                    ?error,
                    ?scope,
                    lookup_key,
                    merchant_id = %query.merchant_id,
                    payment_id = ?query.payment_id,
                    request_id = ?query.request_id,
                    "payment audit timeline load failed"
                );
                error
            })?
    } else {
        Vec::new()
    };
    // Each result's A/B assignment, in one lookup for the page. A failure leaves `experiments`
    // unset rather than claiming "not in an experiment".
    let experiment_keys: Vec<String> = results.iter().map(|row| row.lookup_key.clone()).collect();
    let assignments = match metrics::audit_experiments::load(
        client,
        &query.merchant_id,
        &experiment_keys,
    )
    .await
    {
        Ok(assignments) => Some(assignments),
        Err(error) => {
            crate::logger::warn!(
                ?error,
                merchant_id = %query.merchant_id,
                "payment audit experiment assignments load failed"
            );
            None
        }
    };
    for row in &mut results {
        row.experiments = assignments
            .as_ref()
            .map(|by_key| by_key.get(&row.lookup_key).cloned().unwrap_or_default());
    }

    Ok(PaymentAuditResponse {
        merchant_id: query.merchant_id.clone(),
        range: if exact_lookup {
            "all".to_string()
        } else if query.start_ms.is_some() && query.end_ms.is_some() {
            "custom".to_string()
        } else {
            payment_audit_range(query)
        },
        payment_id: query
            .payment_id
            .clone()
            .or_else(|| results.first().and_then(|row| row.payment_id.clone())),
        request_id: query
            .request_id
            .clone()
            .or_else(|| results.first().and_then(|row| row.request_id.clone())),
        // Echoed comma-separated, as they were requested.
        gateway: (!query.gateway.is_empty()).then(|| query.gateway.join(",")),
        route: if scope == PaymentAuditScope::Preview {
            Some(AnalyticsRoute::RoutingEvaluate.as_str().to_string())
        } else {
            (!query.route.is_empty()).then(|| query.route.join(","))
        },
        status: query.status.clone(),
        flow_type: query.flow_type.clone(),
        routing_approach: query.routing_approach.clone(),
        error_code: query.error_code.clone(),
        scope: scope.as_str().to_string(),
        routing_kind: query.routing_kind.map(|kind| kind.as_str().to_string()),
        page,
        page_size,
        total_results,
        total_success,
        total_failure,
        results,
        timeline,
    })
}
