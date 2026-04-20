use async_trait::async_trait;
use clickhouse::{Client, Row};
use masking::PeekInterface;
use serde::{Deserialize, Serialize};

use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::models::*;
use crate::analytics::service::{format_range, now_ms};
use crate::analytics::store::{AnalyticsReadStore, AnalyticsWriteStore};
use crate::config::ClickHouseAnalyticsConfig;
use crate::error::ApiError;
use crate::metrics::{
    ANALYTICS_SINK_WRITE_LATENCY_HISTOGRAM, ANALYTICS_SINK_WRITES_TOTAL,
};

const DOMAIN_TABLE: &str = "analytics_domain_events_v1";
const API_TABLE: &str = "analytics_api_events_v1";

#[derive(Clone)]
pub struct ClickHouseAnalyticsStore {
    client: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
struct ClickHouseDomainEventRow {
    event_id: u64,
    tenant_id: String,
    event_type: String,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    card_network: Option<String>,
    card_is_in: Option<String>,
    currency: Option<String>,
    country: Option<String>,
    auth_type: Option<String>,
    gateway: Option<String>,
    event_stage: Option<String>,
    routing_approach: Option<String>,
    rule_name: Option<String>,
    status: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    score_value: Option<f64>,
    sigma_factor: Option<f64>,
    average_latency: Option<f64>,
    tp99_latency: Option<f64>,
    transaction_count: Option<i64>,
    route: Option<String>,
    details: Option<String>,
    created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
struct ClickHouseApiEventRow {
    event_id: u64,
    tenant_id: String,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    api_flow: String,
    created_at_timestamp: i64,
    request_id: String,
    latency: u64,
    status_code: i64,
    auth_type: Option<String>,
    request: String,
    user_agent: Option<String>,
    ip_addr: Option<String>,
    url_path: String,
    response: Option<String>,
    error: Option<String>,
    event_type: String,
    http_method: String,
    infra_components: Option<String>,
    request_truncated: bool,
    response_truncated: bool,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct RouteHitRow {
    route: Option<String>,
    count: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct ScoreSnapshotRow {
    merchant_id: String,
    payment_method_type: String,
    payment_method: String,
    gateway: String,
    score_value: f64,
    sigma_factor: f64,
    average_latency: f64,
    tp99_latency: f64,
    transaction_count: i64,
    last_updated_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct ScoreSeriesRow {
    bucket_ms: i64,
    merchant_id: String,
    payment_method_type: String,
    payment_method: String,
    gateway: String,
    score_value: f64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct ErrorSummaryRow {
    route: String,
    error_code: String,
    error_message: String,
    count: i64,
    last_seen_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct RuleHitRow {
    rule_name: String,
    count: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct DecisionPointRow {
    bucket_ms: i64,
    routing_approach: String,
    count: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct GatewaySharePointRow {
    bucket_ms: i64,
    gateway: String,
    count: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct CountTileRow {
    total: i64,
    failures: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct LogSampleRow {
    route: String,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    gateway: Option<String>,
    routing_approach: Option<String>,
    status: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    event_type: String,
    created_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct AuditSummaryRow {
    lookup_key: String,
    payment_id: Option<String>,
    request_id: Option<String>,
    merchant_id: Option<String>,
    first_seen_ms: i64,
    last_seen_ms: i64,
    event_count: u64,
    latest_status: Option<String>,
    latest_gateway: Option<String>,
    latest_stage: Option<String>,
    gateways: Vec<String>,
    routes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct AuditEventRow {
    event_id: u64,
    event_type: String,
    event_stage: Option<String>,
    route: Option<String>,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    gateway: Option<String>,
    routing_approach: Option<String>,
    rule_name: Option<String>,
    status: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    score_value: Option<f64>,
    sigma_factor: Option<f64>,
    average_latency: Option<f64>,
    tp99_latency: Option<f64>,
    transaction_count: Option<i64>,
    details: Option<String>,
    created_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct DistinctDimensionRow {
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    card_network: Option<String>,
    card_is_in: Option<String>,
    currency: Option<String>,
    country: Option<String>,
    auth_type: Option<String>,
    gateway: Option<String>,
}

impl ClickHouseAnalyticsStore {
    pub async fn new(config: ClickHouseAnalyticsConfig) -> Result<Self, ApiError> {
        let mut client = Client::default()
            .with_url(config.url.clone())
            .with_database(config.database.clone())
            .with_user(config.user.clone());
        if let Some(password) = &config.password {
            client = client.with_password(password.peek().clone());
        }

        client
            .query("SELECT 1 AS one")
            .fetch_all::<SingleValueRow>()
            .await
            .map_err(|_| ApiError::DatabaseError)?;

        Ok(Self { client })
    }

    fn store_error(&self) -> ApiError {
        ApiError::DatabaseError
    }
}

#[derive(Debug, Clone, Deserialize, Row)]
struct SingleValueRow {
    #[serde(rename = "one")]
    _one: u8,
}

#[async_trait]
impl AnalyticsWriteStore for ClickHouseAnalyticsStore {
    async fn persist_domain_events(&self, events: &[DomainAnalyticsEvent]) -> Result<(), ApiError> {
        if events.is_empty() {
            return Ok(());
        }

        let timer = ANALYTICS_SINK_WRITE_LATENCY_HISTOGRAM
            .with_label_values(&["clickhouse", "domain"])
            .start_timer();
        let mut insert = self
            .client
            .insert(DOMAIN_TABLE)
            .map_err(|_| self.store_error())?;
        for event in events {
            insert
                .write(&ClickHouseDomainEventRow::from(event.clone()))
                .await
                .map_err(|_| self.store_error())?;
        }
        let result = insert.end().await.map_err(|_| self.store_error());
        let result_label = if result.is_ok() { "success" } else { "failure" };
        ANALYTICS_SINK_WRITES_TOTAL
            .with_label_values(&["clickhouse", "domain", result_label])
            .inc();
        timer.observe_duration();
        result
    }

    async fn persist_api_events(&self, events: &[ApiEvent]) -> Result<(), ApiError> {
        if events.is_empty() {
            return Ok(());
        }

        let timer = ANALYTICS_SINK_WRITE_LATENCY_HISTOGRAM
            .with_label_values(&["clickhouse", "api"])
            .start_timer();
        let mut insert = self.client.insert(API_TABLE).map_err(|_| self.store_error())?;
        for event in events {
            insert
                .write(&ClickHouseApiEventRow::from(event.clone()))
                .await
                .map_err(|_| self.store_error())?;
        }
        let result = insert.end().await.map_err(|_| self.store_error());
        let result_label = if result.is_ok() { "success" } else { "failure" };
        ANALYTICS_SINK_WRITES_TOTAL
            .with_label_values(&["clickhouse", "api", result_label])
            .inc();
        timer.observe_duration();
        result
    }

    fn sink_name(&self) -> &'static str {
        "clickhouse"
    }
}

#[async_trait]
impl AnalyticsReadStore for ClickHouseAnalyticsStore {
    async fn overview(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsOverviewResponse, ApiError> {
        if query.scope == AnalyticsScope::All {
            return Ok(AnalyticsOverviewResponse {
                generated_at_ms: now_ms(),
                scope: query.scope.as_str().to_string(),
                merchant_id: query.merchant_id.clone(),
                kpis: vec![
                    AnalyticsKpi { label: format!("Decisions / {}", format_range(query)), value: "0".to_string(), subtitle: Some("Global mode is limited to connector-level analytics".to_string()) },
                    AnalyticsKpi { label: "Score snapshots".to_string(), value: "0".to_string(), subtitle: Some("Global mode hides merchant-specific data".to_string()) },
                    AnalyticsKpi { label: "Rule hits".to_string(), value: "0".to_string(), subtitle: Some("Global mode hides merchant-specific data".to_string()) },
                    AnalyticsKpi { label: "Errors".to_string(), value: "0".to_string(), subtitle: Some("Global mode hides merchant-specific data".to_string()) },
                ],
                route_hits: vec![
                    AnalyticsRouteHit { route: "/decide_gateway".to_string(), count: 0 },
                    AnalyticsRouteHit { route: "/update_gateway".to_string(), count: 0 },
                    AnalyticsRouteHit { route: "/rule_evaluate".to_string(), count: 0 },
                ],
                top_scores: Vec::new(),
                top_errors: Vec::new(),
                top_rules: Vec::new(),
            });
        }

        let (start_ms, end_ms) = effective_window_bounds(query);
        let base_where = base_where_clause(tenant_id, query.merchant_id.as_deref(), start_ms, end_ms);

        let counts_query = format!(
            "SELECT \
                countIf(event_type = 'decision') AS total, \
                countIf(event_type = 'score_snapshot') AS score_count, \
                countIf(event_type = 'rule_hit') AS rule_hit_count, \
                countIf(event_type = 'error') AS error_count \
             FROM {DOMAIN_TABLE} {base_where}"
        );
        let count_row = self
            .client
            .query(&counts_query)
            .fetch_one::<OverviewCountRow>()
            .await
            .map_err(|_| self.store_error())?;

        let route_hits_rows = self
            .client
            .query(&format!(
                "SELECT route, count() AS count \
                 FROM {DOMAIN_TABLE} {base_where} AND event_type = 'request_hit' \
                 GROUP BY route"
            ))
            .fetch_all::<RouteHitRow>()
            .await
            .map_err(|_| self.store_error())?;

        let top_scores = self.load_score_snapshots(tenant_id, query, Some(5)).await?;
        let top_errors = self.load_error_summaries(tenant_id, query, Some(5)).await?;
        let top_rules = self.load_rule_hits(tenant_id, query, Some(5)).await?;

        Ok(AnalyticsOverviewResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            kpis: vec![
                AnalyticsKpi {
                    label: format!("Decisions / {}", format_range(query)),
                    value: count_row.total.to_string(),
                    subtitle: Some("Recorded decision events".to_string()),
                },
                AnalyticsKpi {
                    label: "Score snapshots".to_string(),
                    value: count_row.score_count.to_string(),
                    subtitle: Some("Latest gateway score updates".to_string()),
                },
                AnalyticsKpi {
                    label: "Rule hits".to_string(),
                    value: count_row.rule_hit_count.to_string(),
                    subtitle: Some("Priority-logic hits".to_string()),
                },
                AnalyticsKpi {
                    label: "Errors".to_string(),
                    value: count_row.error_count.to_string(),
                    subtitle: Some("Structured failure summaries".to_string()),
                },
            ],
            route_hits: map_route_hits(route_hits_rows),
            top_scores,
            top_errors,
            top_rules,
        })
    }

    async fn gateway_scores(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsGatewayScoresResponse, ApiError> {
        if query.scope == AnalyticsScope::All {
            return Ok(AnalyticsGatewayScoresResponse {
                generated_at_ms: now_ms(),
                scope: query.scope.as_str().to_string(),
                merchant_id: query.merchant_id.clone(),
                range: format_range(query),
                snapshots: Vec::new(),
                series: Vec::new(),
            });
        }

        Ok(AnalyticsGatewayScoresResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: format_range(query),
            snapshots: self.load_score_snapshots(tenant_id, query, None).await?,
            series: self.load_score_series(tenant_id, query).await?,
        })
    }

    async fn decisions(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsDecisionResponse, ApiError> {
        if query.scope == AnalyticsScope::All {
            return Ok(AnalyticsDecisionResponse {
                generated_at_ms: now_ms(),
                scope: query.scope.as_str().to_string(),
                merchant_id: query.merchant_id.clone(),
                range: format_range(query),
                tiles: vec![
                    AnalyticsKpi { label: "Decisions".to_string(), value: "0".to_string(), subtitle: Some("Global mode hides merchant-specific traffic volumes".to_string()) },
                    AnalyticsKpi { label: "Error rate".to_string(), value: "0.00%".to_string(), subtitle: Some("Global mode hides merchant-specific traffic volumes".to_string()) },
                ],
                series: Vec::new(),
                approaches: Vec::new(),
            });
        }

        let (start_ms, end_ms) = effective_window_bounds(query);
        let bucket = query_bucket_size_ms(start_ms, end_ms);
        let base_where = format!(
            "{} AND event_type = 'decision'",
            base_where_clause(tenant_id, query.merchant_id.as_deref(), start_ms, end_ms)
        );

        let count_row = self.client.query(&format!(
            "SELECT \
                count() AS total, \
                countIf(lowerUTF8(ifNull(status, '')) = 'failure') AS failures \
             FROM {DOMAIN_TABLE} {base_where}"
        ))
        .fetch_one::<CountTileRow>()
        .await
        .map_err(|_| self.store_error())?;

        let series = self.client.query(&format!(
            "SELECT \
                intDiv(created_at_ms, {bucket}) * {bucket} AS bucket_ms, \
                ifNull(routing_approach, 'UNKNOWN') AS routing_approach, \
                count() AS count \
             FROM {DOMAIN_TABLE} {base_where} \
             GROUP BY bucket_ms, routing_approach \
             ORDER BY bucket_ms ASC, routing_approach ASC"
        ))
        .fetch_all::<DecisionPointRow>()
        .await
        .map_err(|_| self.store_error())?
        .into_iter()
        .map(|row| AnalyticsDecisionPoint {
            bucket_ms: row.bucket_ms,
            routing_approach: row.routing_approach,
            count: row.count,
        })
        .collect();

        let approaches = self.client.query(&format!(
            "SELECT ifNull(routing_approach, 'UNKNOWN') AS rule_name, count() AS count \
             FROM {DOMAIN_TABLE} {base_where} \
             GROUP BY rule_name \
             ORDER BY count DESC, rule_name ASC"
        ))
        .fetch_all::<RuleHitRow>()
        .await
        .map_err(|_| self.store_error())?
        .into_iter()
        .map(|row| AnalyticsRuleHit {
            rule_name: row.rule_name,
            count: row.count,
        })
        .collect::<Vec<_>>();

        let error_rate = if count_row.total > 0 {
            (count_row.failures as f64 / count_row.total as f64) * 100.0
        } else {
            0.0
        };

        Ok(AnalyticsDecisionResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: format_range(query),
            tiles: vec![
                AnalyticsKpi {
                    label: "Decisions".to_string(),
                    value: count_row.total.to_string(),
                    subtitle: Some(format!("Failures: {}", count_row.failures)),
                },
                AnalyticsKpi {
                    label: "Error rate".to_string(),
                    value: format!("{error_rate:.2}%"),
                    subtitle: Some("From recorded decision events".to_string()),
                },
            ],
            series,
            approaches,
        })
    }

    async fn routing_stats(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsRoutingStatsResponse, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let bucket = query_bucket_size_ms(start_ms, end_ms);
        let base_where = base_where_clause(tenant_id, query.merchant_id.as_deref(), start_ms, end_ms);
        let gateway_share = self.client.query(&format!(
            "SELECT \
                intDiv(created_at_ms, {bucket}) * {bucket} AS bucket_ms, \
                ifNull(gateway, 'unknown') AS gateway, \
                count() AS count \
             FROM {DOMAIN_TABLE} {base_where} AND event_type = 'decision' \
             GROUP BY bucket_ms, gateway \
             ORDER BY bucket_ms ASC, gateway ASC"
        ))
        .fetch_all::<GatewaySharePointRow>()
        .await
        .map_err(|_| self.store_error())?
        .into_iter()
        .map(|row| AnalyticsGatewaySharePoint {
            bucket_ms: row.bucket_ms,
            gateway: row.gateway,
            count: row.count,
        })
        .collect();

        Ok(AnalyticsRoutingStatsResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: format_range(query),
            gateway_share,
            top_rules: self.load_rule_hits(tenant_id, query, Some(10)).await?,
            sr_trend: self.load_score_series(tenant_id, query).await?,
            available_filters: self.load_filter_options(tenant_id, query).await?,
        })
    }

    async fn log_summaries(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsLogSummariesResponse, ApiError> {
        if query.scope == AnalyticsScope::All {
            return Ok(AnalyticsLogSummariesResponse {
                generated_at_ms: now_ms(),
                scope: query.scope.as_str().to_string(),
                merchant_id: query.merchant_id.clone(),
                range: format_range(query),
                total_errors: 0,
                errors: Vec::new(),
                samples: Vec::new(),
                page: query.page.max(1),
                page_size: query.page_size.clamp(1, 50),
            });
        }

        let errors = self.load_error_summaries(tenant_id, query, Some(10)).await?;
        let total_errors = errors.iter().map(|entry| entry.count).sum();
        let (start_ms, end_ms) = effective_window_bounds(query);
        let base_where = format!(
            "{} AND event_type = 'error'",
            base_where_clause(tenant_id, query.merchant_id.as_deref(), start_ms, end_ms)
        );
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 50);
        let offset = (page - 1) * page_size;
        let samples = self.client.query(&format!(
            "SELECT \
                ifNull(route, 'unknown') AS route, merchant_id, payment_id, request_id, gateway, \
                routing_approach, status, error_code, error_message, event_type, created_at_ms \
             FROM {DOMAIN_TABLE} {base_where} \
             ORDER BY created_at_ms DESC \
             LIMIT {page_size} OFFSET {offset}"
        ))
        .fetch_all::<LogSampleRow>()
        .await
        .map_err(|_| self.store_error())?
        .into_iter()
        .map(|row| AnalyticsLogSample {
            route: row.route,
            merchant_id: row.merchant_id,
            payment_id: row.payment_id,
            request_id: row.request_id,
            gateway: row.gateway,
            routing_approach: row.routing_approach,
            status: row.status,
            error_code: row.error_code,
            error_message: row.error_message,
            event_type: Some(row.event_type),
            created_at_ms: row.created_at_ms,
        })
        .collect();

        Ok(AnalyticsLogSummariesResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: format_range(query),
            total_errors,
            errors,
            samples,
            page,
            page_size,
        })
    }

    async fn payment_audit(
        &self,
        tenant_id: &str,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        self.load_payment_audit_response(tenant_id, query, false).await
    }

    async fn preview_trace(
        &self,
        tenant_id: &str,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        self.load_payment_audit_response(tenant_id, query, true).await
    }
}

impl ClickHouseAnalyticsStore {
    async fn load_payment_audit_response(
        &self,
        tenant_id: &str,
        query: &PaymentAuditQuery,
        preview_only: bool,
    ) -> Result<PaymentAuditResponse, ApiError> {
        if query.scope == AnalyticsScope::All {
            return Ok(PaymentAuditResponse {
                generated_at_ms: now_ms(),
                scope: query.scope.as_str().to_string(),
                merchant_id: query.merchant_id.clone(),
                range: if query.start_ms.is_some() && query.end_ms.is_some() { "custom".to_string() } else { payment_audit_range(query) },
                payment_id: query.payment_id.clone(),
                request_id: query.request_id.clone(),
                gateway: query.gateway.clone(),
                route: query.route.clone(),
                status: query.status.clone(),
                event_type: query.event_type.clone(),
                error_code: query.error_code.clone(),
                page: query.page.max(1),
                page_size: query.page_size.clamp(1, 50),
                total_results: 0,
                results: Vec::new(),
                timeline: Vec::new(),
            });
        }

        let summary_rows = self.load_audit_summaries(tenant_id, query, preview_only).await?;
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
            self.load_audit_timeline(tenant_id, query, preview_only, &lookup_key)
                .await?
        } else {
            Vec::new()
        };

        Ok(PaymentAuditResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: if query.start_ms.is_some() && query.end_ms.is_some() { "custom".to_string() } else { payment_audit_range(query) },
            payment_id: query.payment_id.clone().or_else(|| results.first().and_then(|row| row.payment_id.clone())),
            request_id: query.request_id.clone().or_else(|| results.first().and_then(|row| row.request_id.clone())),
            gateway: query.gateway.clone(),
            route: if preview_only { Some("routing_evaluate".to_string()) } else { query.route.clone() },
            status: query.status.clone(),
            event_type: query.event_type.clone(),
            error_code: query.error_code.clone(),
            page,
            page_size,
            total_results,
            results,
            timeline,
        })
    }

    async fn load_score_snapshots(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
        limit: Option<usize>,
    ) -> Result<Vec<GatewayScoreSnapshot>, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let filter_suffix = score_filters(query);
        let limit_clause = limit.map(|value| format!("LIMIT {value}")).unwrap_or_default();
        let sql = format!(
            "SELECT \
                ifNull(merchant_id, '') AS merchant_id, \
                ifNull(payment_method_type, '') AS payment_method_type, \
                ifNull(payment_method, '') AS payment_method, \
                ifNull(gateway, '') AS gateway, \
                argMax(score_value, created_at_ms) AS score_value, \
                argMax(sigma_factor, created_at_ms) AS sigma_factor, \
                argMax(average_latency, created_at_ms) AS average_latency, \
                argMax(tp99_latency, created_at_ms) AS tp99_latency, \
                argMax(transaction_count, created_at_ms) AS transaction_count, \
                max(created_at_ms) AS last_updated_ms \
             FROM {DOMAIN_TABLE} \
             WHERE tenant_id = '{tenant_id}' \
               AND created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND event_type = 'score_snapshot' {filter_suffix} \
             GROUP BY merchant_id, payment_method_type, payment_method, gateway \
             ORDER BY score_value DESC, last_updated_ms DESC \
             {limit_clause}",
            tenant_id = escape_sql(tenant_id),
        );

        self.client
            .query(&sql)
            .fetch_all::<ScoreSnapshotRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| GatewayScoreSnapshot {
                merchant_id: row.merchant_id,
                payment_method_type: row.payment_method_type,
                payment_method: row.payment_method,
                gateway: row.gateway,
                score_value: row.score_value,
                sigma_factor: row.sigma_factor,
                average_latency: row.average_latency,
                tp99_latency: row.tp99_latency,
                transaction_count: row.transaction_count,
                last_updated_ms: row.last_updated_ms,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_score_series(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<Vec<GatewayScoreSeriesPoint>, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let bucket = query_bucket_size_ms(start_ms, end_ms);
        let filter_suffix = score_filters(query);
        let sql = format!(
            "SELECT \
                intDiv(created_at_ms, {bucket}) * {bucket} AS bucket_ms, \
                ifNull(merchant_id, '') AS merchant_id, \
                ifNull(payment_method_type, '') AS payment_method_type, \
                ifNull(payment_method, '') AS payment_method, \
                ifNull(gateway, '') AS gateway, \
                avg(score_value) AS score_value \
             FROM {DOMAIN_TABLE} \
             WHERE tenant_id = '{tenant_id}' \
               AND created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND event_type = 'score_snapshot' {filter_suffix} \
             GROUP BY bucket_ms, merchant_id, payment_method_type, payment_method, gateway \
             ORDER BY bucket_ms ASC, gateway ASC",
            tenant_id = escape_sql(tenant_id),
        );
        self.client
            .query(&sql)
            .fetch_all::<ScoreSeriesRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| GatewayScoreSeriesPoint {
                bucket_ms: row.bucket_ms,
                merchant_id: row.merchant_id,
                payment_method_type: row.payment_method_type,
                payment_method: row.payment_method,
                gateway: row.gateway,
                score_value: row.score_value,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_error_summaries(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
        limit: Option<usize>,
    ) -> Result<Vec<AnalyticsErrorSummary>, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let limit_clause = limit.map(|value| format!("LIMIT {value}")).unwrap_or_default();
        let sql = format!(
            "SELECT \
                ifNull(route, 'unknown') AS route, \
                ifNull(error_code, 'unknown') AS error_code, \
                ifNull(error_message, 'unknown') AS error_message, \
                count() AS count, \
                max(created_at_ms) AS last_seen_ms \
             FROM {DOMAIN_TABLE} \
             WHERE tenant_id = '{tenant_id}' \
               AND created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND event_type = 'error' \
               {merchant_filter} \
             GROUP BY route, error_code, error_message \
             ORDER BY count DESC, last_seen_ms DESC \
             {limit_clause}",
            tenant_id = escape_sql(tenant_id),
            merchant_filter = merchant_filter(query.merchant_id.as_deref()),
        );
        self.client
            .query(&sql)
            .fetch_all::<ErrorSummaryRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| AnalyticsErrorSummary {
                route: row.route,
                error_code: row.error_code,
                error_message: row.error_message,
                count: row.count,
                last_seen_ms: row.last_seen_ms,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_rule_hits(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
        limit: Option<usize>,
    ) -> Result<Vec<AnalyticsRuleHit>, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let limit_clause = limit.map(|value| format!("LIMIT {value}")).unwrap_or_default();
        let sql = format!(
            "SELECT ifNull(rule_name, 'unknown') AS rule_name, count() AS count \
             FROM {DOMAIN_TABLE} \
             WHERE tenant_id = '{tenant_id}' \
               AND created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND event_type = 'rule_hit' {merchant_filter} \
             GROUP BY rule_name \
             ORDER BY count DESC, rule_name ASC \
             {limit_clause}",
            tenant_id = escape_sql(tenant_id),
            merchant_filter = merchant_filter(query.merchant_id.as_deref()),
        );
        self.client
            .query(&sql)
            .fetch_all::<RuleHitRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| AnalyticsRuleHit {
                rule_name: row.rule_name,
                count: row.count,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_filter_options(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<RoutingFilterOptions, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let sql = format!(
            "SELECT DISTINCT payment_method_type, payment_method, card_network, card_is_in, country, currency, auth_type, gateway \
             FROM {DOMAIN_TABLE} \
             WHERE tenant_id = '{tenant_id}' \
               AND created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND event_type = 'score_snapshot' {merchant_filter}",
            tenant_id = escape_sql(tenant_id),
            merchant_filter = merchant_filter(query.merchant_id.as_deref()),
        );
        let rows = self.client
            .query(&sql)
            .fetch_all::<DistinctDimensionRow>()
            .await
            .map_err(|_| self.store_error())?;

        let gateways = rows
            .iter()
            .filter_map(|row| row.gateway.clone())
            .filter(|value| !value.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        let mut dimensions = Vec::new();
        for (key, label) in [
            ("payment_method_type", "Payment Method Type"),
            ("payment_method", "Payment Method"),
            ("card_network", "Card Network"),
            ("card_is_in", "Card ISIN"),
            ("currency", "Currency"),
            ("country", "Country"),
            ("auth_type", "Auth Type"),
        ] {
            let values = rows
                .iter()
                .filter_map(|row| match key {
                    "payment_method_type" => row.payment_method_type.clone(),
                    "payment_method" => row.payment_method.clone(),
                    "card_network" => row.card_network.clone(),
                    "card_is_in" => row.card_is_in.clone(),
                    "currency" => row.currency.clone(),
                    "country" => row.country.clone(),
                    "auth_type" => row.auth_type.clone(),
                    _ => None,
                })
                .filter(|value| !value.is_empty())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if !values.is_empty() {
                dimensions.push(RoutingFilterDimension {
                    key: key.to_string(),
                    label: label.to_string(),
                    values,
                });
            }
        }

        Ok(RoutingFilterOptions {
            dimensions,
            missing_dimensions: Vec::new(),
            gateways,
        })
    }

    async fn load_audit_summaries(
        &self,
        tenant_id: &str,
        query: &PaymentAuditQuery,
        preview_only: bool,
    ) -> Result<Vec<PaymentAuditSummary>, ApiError> {
        let sql = format!(
            "SELECT \
                lookup_key, \
                argMax(payment_id, created_at_ms) AS payment_id, \
                argMax(request_id, created_at_ms) AS request_id, \
                argMax(merchant_id, created_at_ms) AS merchant_id, \
                min(created_at_ms) AS first_seen_ms, \
                max(created_at_ms) AS last_seen_ms, \
                count() AS event_count, \
                argMax(status, created_at_ms) AS latest_status, \
                argMax(gateway, created_at_ms) AS latest_gateway, \
                argMax(event_stage, created_at_ms) AS latest_stage, \
                arrayFilter(x -> x != '', groupUniqArray(ifNull(gateway, ''))) AS gateways, \
                arrayFilter(x -> x != '', groupUniqArray(ifNull(route, ''))) AS routes \
             FROM ( \
                SELECT \
                    if(payment_id != '' AND payment_id IS NOT NULL, payment_id, request_id) AS lookup_key, \
                    payment_id, request_id, merchant_id, created_at_ms, status, gateway, event_stage, route \
                FROM {DOMAIN_TABLE} {audit_where} \
             ) \
             WHERE lookup_key != '' \
             GROUP BY lookup_key \
             ORDER BY last_seen_ms DESC, event_count DESC",
            audit_where = payment_audit_where_clause(tenant_id, query, preview_only),
        );
        self.client
            .query(&sql)
            .fetch_all::<AuditSummaryRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| PaymentAuditSummary {
                lookup_key: row.lookup_key,
                payment_id: row.payment_id,
                request_id: row.request_id,
                merchant_id: row.merchant_id,
                first_seen_ms: row.first_seen_ms,
                last_seen_ms: row.last_seen_ms,
                event_count: row.event_count as usize,
                latest_status: row.latest_status,
                latest_gateway: row.latest_gateway,
                latest_stage: row.latest_stage.map(payment_audit_stage_label),
                gateways: row.gateways,
                routes: row.routes.into_iter().map(payment_audit_route_label).collect(),
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_audit_timeline(
        &self,
        tenant_id: &str,
        query: &PaymentAuditQuery,
        preview_only: bool,
        lookup_key: &str,
    ) -> Result<Vec<PaymentAuditEvent>, ApiError> {
        let sql = format!(
            "SELECT \
                event_id, event_type, event_stage, route, merchant_id, payment_id, request_id, \
                payment_method_type, payment_method, gateway, routing_approach, rule_name, status, \
                error_code, error_message, score_value, sigma_factor, average_latency, tp99_latency, \
                transaction_count, details, created_at_ms \
             FROM {DOMAIN_TABLE} {audit_where} \
               AND (payment_id = '{lookup_key}' OR request_id = '{lookup_key}') \
             ORDER BY created_at_ms ASC, event_id ASC",
            audit_where = payment_audit_where_clause(tenant_id, query, preview_only),
            lookup_key = escape_sql(lookup_key),
        );
        self.client
            .query(&sql)
            .fetch_all::<AuditEventRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| PaymentAuditEvent {
                id: row.event_id as i64,
                event_type: row.event_type,
                event_stage: row.event_stage,
                route: row.route,
                merchant_id: row.merchant_id,
                payment_id: row.payment_id,
                request_id: row.request_id,
                payment_method_type: row.payment_method_type,
                payment_method: row.payment_method,
                gateway: row.gateway,
                routing_approach: row.routing_approach,
                rule_name: row.rule_name,
                status: row.status,
                error_code: row.error_code,
                error_message: row.error_message,
                score_value: row.score_value,
                sigma_factor: row.sigma_factor,
                average_latency: row.average_latency,
                tp99_latency: row.tp99_latency,
                transaction_count: row.transaction_count,
                details_json: row
                    .details
                    .as_ref()
                    .and_then(|value| serde_json::from_str(value).ok()),
                details: row.details,
                created_at_ms: row.created_at_ms,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }
}

#[derive(Debug, Clone, Deserialize, Row)]
struct OverviewCountRow {
    total: i64,
    score_count: i64,
    rule_hit_count: i64,
    error_count: i64,
}

impl From<DomainAnalyticsEvent> for ClickHouseDomainEventRow {
    fn from(event: DomainAnalyticsEvent) -> Self {
        Self {
            event_id: event.event_id,
            tenant_id: event.tenant_id,
            event_type: event.event_type,
            merchant_id: event.merchant_id,
            payment_id: event.payment_id,
            request_id: event.request_id,
            payment_method_type: event.payment_method_type,
            payment_method: event.payment_method,
            card_network: event.card_network,
            card_is_in: event.card_is_in,
            currency: event.currency,
            country: event.country,
            auth_type: event.auth_type,
            gateway: event.gateway,
            event_stage: event.event_stage,
            routing_approach: event.routing_approach,
            rule_name: event.rule_name,
            status: event.status,
            error_code: event.error_code,
            error_message: event.error_message,
            score_value: event.score_value,
            sigma_factor: event.sigma_factor,
            average_latency: event.average_latency,
            tp99_latency: event.tp99_latency,
            transaction_count: event.transaction_count,
            route: event.route,
            details: event.details,
            created_at_ms: event.created_at_ms,
        }
    }
}

impl From<ApiEvent> for ClickHouseApiEventRow {
    fn from(event: ApiEvent) -> Self {
        Self {
            event_id: event.event_id,
            tenant_id: event.tenant_id,
            merchant_id: event.merchant_id,
            payment_id: event.payment_id,
            api_flow: event.api_flow,
            created_at_timestamp: event.created_at_timestamp,
            request_id: event.request_id,
            latency: event.latency,
            status_code: event.status_code,
            auth_type: event.auth_type,
            request: event.request,
            user_agent: event.user_agent,
            ip_addr: event.ip_addr,
            url_path: event.url_path,
            response: event.response,
            error: event.error.and_then(|value| serde_json::to_string(&value).ok()),
            event_type: event.event_type,
            http_method: event.http_method,
            infra_components: event
                .infra_components
                .and_then(|value| serde_json::to_string(&value).ok()),
            request_truncated: event.request_truncated,
            response_truncated: event.response_truncated,
        }
    }
}

fn effective_window_bounds(query: &AnalyticsQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()));
    (start_ms, end_ms)
}

fn effective_payment_audit_window_bounds(query: &PaymentAuditQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()));
    (start_ms, end_ms)
}

fn query_bucket_size_ms(start_ms: i64, end_ms: i64) -> i64 {
    let window_ms = end_ms.saturating_sub(start_ms);
    match window_ms {
        0..=900_000 => 60 * 1000,
        900_001..=3_600_000 => 5 * 60 * 1000,
        3_600_001..=86_400_000 => 15 * 60 * 1000,
        86_400_001..=259_200_000 => 60 * 60 * 1000,
        _ => 3 * 60 * 60 * 1000,
    }
}

fn payment_audit_range(query: &PaymentAuditQuery) -> String {
    match query.range {
        AnalyticsRange::M15 => "15m".to_string(),
        AnalyticsRange::H1 => "1h".to_string(),
        AnalyticsRange::H24 => "24h".to_string(),
        AnalyticsRange::D30 => "30d".to_string(),
    }
}

fn base_where_clause(
    tenant_id: &str,
    merchant_id: Option<&str>,
    start_ms: i64,
    end_ms: i64,
) -> String {
    format!(
        "WHERE tenant_id = '{tenant_id}' \
           AND created_at_ms >= {start_ms} AND created_at_ms <= {end_ms}{}",
        merchant_filter(merchant_id),
        tenant_id = escape_sql(tenant_id),
    )
}

fn merchant_filter(merchant_id: Option<&str>) -> String {
    merchant_id
        .map(|value| format!(" AND merchant_id = '{}'", escape_sql(value)))
        .unwrap_or_default()
}

fn score_filters(query: &AnalyticsQuery) -> String {
    let mut filters = merchant_filter(query.merchant_id.as_deref());
    if let Some(value) = &query.payment_method_type {
        filters.push_str(&format!(" AND payment_method_type = '{}'", escape_sql(value)));
    }
    if let Some(value) = &query.payment_method {
        filters.push_str(&format!(" AND payment_method = '{}'", escape_sql(value)));
    }
    if let Some(value) = &query.card_network {
        filters.push_str(&format!(" AND card_network = '{}'", escape_sql(value)));
    }
    if let Some(value) = &query.card_is_in {
        filters.push_str(&format!(" AND card_is_in = '{}'", escape_sql(value)));
    }
    if let Some(value) = &query.currency {
        filters.push_str(&format!(" AND currency = '{}'", escape_sql(value)));
    }
    if let Some(value) = &query.country {
        filters.push_str(&format!(" AND country = '{}'", escape_sql(value)));
    }
    if let Some(value) = &query.auth_type {
        filters.push_str(&format!(" AND auth_type = '{}'", escape_sql(value)));
    }
    if !query.gateways.is_empty() {
        filters.push_str(&format!(
            " AND gateway IN ({})",
            query
                .gateways
                .iter()
                .map(|gateway| format!("'{}'", escape_sql(gateway)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    filters
}

fn payment_audit_where_clause(tenant_id: &str, query: &PaymentAuditQuery, preview_only: bool) -> String {
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    let mut filters = vec![
        format!("tenant_id = '{}'", escape_sql(tenant_id)),
        format!("created_at_ms >= {start_ms}"),
        format!("created_at_ms <= {end_ms}"),
    ];
    if let Some(merchant_id) = &query.merchant_id {
        filters.push(format!("merchant_id = '{}'", escape_sql(merchant_id)));
    }
    if preview_only {
        filters.push("route = 'routing_evaluate'".to_string());
        filters.push("event_type IN ('rule_evaluation_preview', 'error')".to_string());
    } else {
        filters.push("event_type IN ('decision', 'gateway_update', 'rule_hit', 'error')".to_string());
        if let Some(route) = &query.route {
            filters.push(format!("route = '{}'", escape_sql(route)));
        }
    }
    if let Some(payment_id) = &query.payment_id {
        filters.push(format!("payment_id = '{}'", escape_sql(payment_id)));
    }
    if query.payment_id.is_none() {
        if let Some(request_id) = &query.request_id {
            filters.push(format!("request_id = '{}'", escape_sql(request_id)));
        }
    }
    if let Some(gateway) = &query.gateway {
        filters.push(format!("gateway = '{}'", escape_sql(gateway)));
    }
    if let Some(status) = &query.status {
        filters.push(format!("status = '{}'", escape_sql(status)));
    }
    if let Some(event_type) = &query.event_type {
        filters.push(format!("event_type = '{}'", escape_sql(event_type)));
    }
    if let Some(error_code) = &query.error_code {
        filters.push(format!("error_code = '{}'", escape_sql(error_code)));
    }
    format!("WHERE {}", filters.join(" AND "))
}

fn map_route_hits(rows: Vec<RouteHitRow>) -> Vec<AnalyticsRouteHit> {
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        counts.insert(row.route.unwrap_or_else(|| "unknown".to_string()), row.count);
    }
    [
        ("decide_gateway", "/decide_gateway"),
        ("update_gateway_score", "/update_gateway"),
        ("routing_evaluate", "/rule_evaluate"),
    ]
    .into_iter()
    .map(|(stored_route, display_route)| AnalyticsRouteHit {
        route: display_route.to_string(),
        count: counts.get(stored_route).copied().unwrap_or(0),
    })
    .collect()
}

fn payment_audit_stage_label(stage: String) -> String {
    match stage.as_str() {
        "gateway_decided" => "Decide Gateway".to_string(),
        "score_updated" => "Update Gateway".to_string(),
        "rule_applied" => "Rule Evaluate".to_string(),
        "preview_evaluated" => "Preview Result".to_string(),
        other => other.to_string(),
    }
}

fn payment_audit_route_label(route: String) -> String {
    match route.as_str() {
        "decision_gateway" | "decide_gateway" => "Decide Gateway".to_string(),
        "update_gateway_score" => "Update Gateway".to_string(),
        "routing_evaluate" => "Rule Evaluate".to_string(),
        _ => route,
    }
}

fn escape_sql(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}
