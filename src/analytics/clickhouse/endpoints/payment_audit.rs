use crate::analytics::flow::AnalyticsRoute;
use crate::analytics::models::{PaymentAuditQuery, PaymentAuditResponse};
use crate::error::ApiError;

use super::super::metrics;
use super::super::time::payment_audit_range;

pub async fn load(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Result<PaymentAuditResponse, ApiError> {
    let requested_lookup_key = crate::analytics::derive_lookup_key(
        query.payment_id.as_deref(),
        query.request_id.as_deref(),
    );
    fn is_success(status: Option<&str>) -> bool {
        matches!(
            status.map(|s| s.to_uppercase()).as_deref(),
            Some("SUCCESS" | "CHARGED" | "AUTHORIZED")
        )
    }
    fn is_failure(status: Option<&str>) -> bool {
        let upper = status.map(|s| s.to_uppercase());
        matches!(upper.as_deref(), Some(s) if s == "FAILURE" || s.contains("FAILED") || s.contains("DECLINED"))
    }

    let (total_results, total_success, total_failure, results) =
        if let Some(lookup_key) = requested_lookup_key.clone() {
            let results =
                metrics::audit_summaries::load_exact(client, query, preview_only, &lookup_key)
                    .await
                    .map_err(|error| {
                        crate::logger::error!(
                            ?error,
                            ?preview_only,
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
                .filter(|r| is_success(r.latest_status.as_deref()))
                .count();
            let failure = results
                .iter()
                .filter(|r| is_failure(r.latest_status.as_deref()))
                .count();
            (results.len(), success, failure, results)
        } else {
            let (total_results, total_success, total_failure) =
                metrics::audit_summaries::count(client, query, preview_only)
                    .await
                    .map_err(|error| {
                        crate::logger::error!(
                            ?error,
                            ?preview_only,
                            merchant_id = %query.merchant_id,
                            payment_id = ?query.payment_id,
                            request_id = ?query.request_id,
                            "payment audit summary count failed"
                        );
                        error
                    })?;
            let results = metrics::audit_summaries::load_page(client, query, preview_only)
                .await
                .map_err(|error| {
                    crate::logger::error!(
                        ?error,
                        ?preview_only,
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
    let selected_lookup_key = results
        .first()
        .map(|row| row.lookup_key.clone())
        .or(requested_lookup_key);

    let timeline = if let Some(lookup_key) = selected_lookup_key.clone() {
        metrics::audit_timeline::load(client, query, preview_only, &lookup_key)
            .await
            .map_err(|error| {
                crate::logger::error!(
                    ?error,
                    ?preview_only,
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

    Ok(PaymentAuditResponse {
        merchant_id: query.merchant_id.clone(),
        range: if query.start_ms.is_some() && query.end_ms.is_some() {
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
        gateway: query.gateway.clone(),
        route: if preview_only {
            Some(AnalyticsRoute::RoutingEvaluate.as_str().to_string())
        } else {
            query.route.clone()
        },
        status: query.status.clone(),
        flow_type: query.flow_type.clone(),
        routing_approach: query.routing_approach.clone(),
        error_code: query.error_code.clone(),
        page,
        page_size,
        total_results,
        total_success,
        total_failure,
        results,
        timeline,
    })
}
