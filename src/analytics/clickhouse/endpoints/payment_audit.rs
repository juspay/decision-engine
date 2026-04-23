use crate::analytics::flow::AnalyticsRoute;
use crate::analytics::models::{AnalyticsScope, PaymentAuditQuery, PaymentAuditResponse};
use crate::analytics::service::now_ms;
use crate::error::ApiError;

use super::super::metrics;
use super::super::time::payment_audit_range;

pub async fn load(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Result<PaymentAuditResponse, ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(PaymentAuditResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: if query.start_ms.is_some() && query.end_ms.is_some() {
                "custom".to_string()
            } else {
                payment_audit_range(query)
            },
            payment_id: query.payment_id.clone(),
            request_id: query.request_id.clone(),
            gateway: query.gateway.clone(),
            route: query.route.clone(),
            status: query.status.clone(),
            flow_type: query.flow_type.clone(),
            error_code: query.error_code.clone(),
            page: query.page.max(1),
            page_size: query.page_size.clamp(1, 50),
            total_results: 0,
            results: Vec::new(),
            timeline: Vec::new(),
        });
    }

    let summary_rows = metrics::audit_summaries::load(client, query, preview_only).await?;
    let total_results = summary_rows.len();
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 50);
    let offset = (page - 1) * page_size;
    let results = summary_rows
        .iter()
        .skip(offset)
        .take(page_size)
        .cloned()
        .collect::<Vec<_>>();
    let selected_lookup_key = query
        .payment_id
        .clone()
        .or_else(|| query.request_id.clone())
        .or_else(|| results.first().map(|row| row.lookup_key.clone()));

    let timeline = if let Some(lookup_key) = selected_lookup_key.clone() {
        metrics::audit_timeline::load(client, query, preview_only, &lookup_key).await?
    } else {
        Vec::new()
    };

    Ok(PaymentAuditResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
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
        error_code: query.error_code.clone(),
        page,
        page_size,
        total_results,
        results,
        timeline,
    })
}
