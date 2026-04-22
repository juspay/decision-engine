use async_trait::async_trait;
use clickhouse::{Client, Row};
use masking::PeekInterface;
use serde::Deserialize;

use crate::analytics::flow::{AnalyticsRoute, FlowType};
use crate::analytics::models::*;
use crate::analytics::service::{format_range, now_ms};
use crate::analytics::store::AnalyticsReadStore;
use crate::config::ClickHouseAnalyticsConfig;
use crate::error::ApiError;

const DOMAIN_TABLE: &str = "analytics_domain_events";
const OVERVIEW_SCORE_FLOW_TYPES: &[FlowType] = &[
    FlowType::UpdateGatewayScoreScoreSnapshot,
    FlowType::UpdateScoreLegacyScoreSnapshot,
];
const OVERVIEW_ERROR_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
    FlowType::RoutingEvaluateError,
];
const ROUTE_HIT_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayRequestHit,
    FlowType::UpdateGatewayScoreRequestHit,
    FlowType::RoutingEvaluateRequestHit,
];
const PAYMENT_AUDIT_PREVIEW_FLOW_TYPES: &[FlowType] = &[
    FlowType::RoutingEvaluateSingle,
    FlowType::RoutingEvaluatePriority,
    FlowType::RoutingEvaluateVolumeSplit,
    FlowType::RoutingEvaluateAdvanced,
    FlowType::RoutingEvaluatePreview,
    FlowType::RoutingEvaluateError,
];
const PAYMENT_AUDIT_DYNAMIC_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayDecision,
    FlowType::UpdateGatewayScoreUpdate,
    FlowType::UpdateScoreLegacyScoreSnapshot,
    FlowType::DecideGatewayRuleHit,
    FlowType::DecideGatewayError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
];

#[derive(Clone)]
pub struct ClickHouseAnalyticsStore {
    client: Client,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct RouteHitRow {
    route: Option<String>,
    count: u64,
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
    count: u64,
    last_seen_ms: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct RuleHitRow {
    rule_name: String,
    count: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct DecisionPointRow {
    bucket_ms: i64,
    routing_approach: String,
    count: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct GatewaySharePointRow {
    bucket_ms: i64,
    gateway: String,
    count: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct CountTileRow {
    total: u64,
    failures: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct LogSampleRow {
    route: String,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    gateway: Option<String>,
    routing_approach: Option<String>,
    status: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    flow_type: String,
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
    flow_type: String,
    event_stage: Option<String>,
    route: Option<String>,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
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
impl AnalyticsReadStore for ClickHouseAnalyticsStore {
    async fn overview(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsOverviewResponse, ApiError> {
        if query.scope == AnalyticsScope::All {
            return Ok(AnalyticsOverviewResponse {
                generated_at_ms: now_ms(),
                scope: query.scope.as_str().to_string(),
                merchant_id: query.merchant_id.clone(),
                kpis: vec![
                    AnalyticsKpi {
                        label: format!("Decisions / {}", format_range(query)),
                        value: "0".to_string(),
                        subtitle: Some(
                            "Global mode is limited to connector-level analytics".to_string(),
                        ),
                    },
                    AnalyticsKpi {
                        label: "Score snapshots".to_string(),
                        value: "0".to_string(),
                        subtitle: Some("Global mode hides merchant-specific data".to_string()),
                    },
                    AnalyticsKpi {
                        label: "Rule hits".to_string(),
                        value: "0".to_string(),
                        subtitle: Some("Global mode hides merchant-specific data".to_string()),
                    },
                    AnalyticsKpi {
                        label: "Errors".to_string(),
                        value: "0".to_string(),
                        subtitle: Some("Global mode hides merchant-specific data".to_string()),
                    },
                ],
                route_hits: vec![
                    AnalyticsRouteHit {
                        route: "/decide_gateway".to_string(),
                        count: 0,
                    },
                    AnalyticsRouteHit {
                        route: "/update_gateway".to_string(),
                        count: 0,
                    },
                    AnalyticsRouteHit {
                        route: "/rule_evaluate".to_string(),
                        count: 0,
                    },
                ],
                top_scores: Vec::new(),
                top_errors: Vec::new(),
                top_rules: Vec::new(),
            });
        }

        let (start_ms, end_ms) = effective_window_bounds(query);
        let base_where = base_where_clause(query.merchant_id.as_deref(), start_ms, end_ms);

        let counts_query = format!(
            "SELECT \
                countIf(flow_type = '{decision_flow_type}') AS total, \
                countIf(flow_type IN {score_flow_types}) AS score_count, \
                countIf(flow_type = '{rule_hit_flow_type}') AS rule_hit_count, \
                countIf(flow_type IN {error_flow_types}) AS error_count \
             FROM {DOMAIN_TABLE} {base_where}",
            decision_flow_type = FlowType::DecideGatewayDecision.as_str(),
            score_flow_types = flow_type_list_sql(OVERVIEW_SCORE_FLOW_TYPES),
            rule_hit_flow_type = FlowType::DecideGatewayRuleHit.as_str(),
            error_flow_types = flow_type_list_sql(OVERVIEW_ERROR_FLOW_TYPES),
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
                 FROM {DOMAIN_TABLE} {base_where} AND flow_type IN {route_hit_flow_types} \
                 GROUP BY route",
                route_hit_flow_types = flow_type_list_sql(ROUTE_HIT_FLOW_TYPES),
            ))
            .fetch_all::<RouteHitRow>()
            .await
            .map_err(|_| self.store_error())?;

        let top_scores = self.load_score_snapshots(query, Some(5)).await?;
        let top_errors = self.load_error_summaries(query, Some(5)).await?;
        let top_rules = self.load_rule_hits(query, Some(5)).await?;

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
            snapshots: self.load_score_snapshots(query, None).await?,
            series: self.load_score_series(query).await?,
        })
    }

    async fn decisions(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsDecisionResponse, ApiError> {
        if query.scope == AnalyticsScope::All {
            return Ok(AnalyticsDecisionResponse {
                generated_at_ms: now_ms(),
                scope: query.scope.as_str().to_string(),
                merchant_id: query.merchant_id.clone(),
                range: format_range(query),
                tiles: vec![
                    AnalyticsKpi {
                        label: "Decisions".to_string(),
                        value: "0".to_string(),
                        subtitle: Some(
                            "Global mode hides merchant-specific traffic volumes".to_string(),
                        ),
                    },
                    AnalyticsKpi {
                        label: "Error rate".to_string(),
                        value: "0.00%".to_string(),
                        subtitle: Some(
                            "Global mode hides merchant-specific traffic volumes".to_string(),
                        ),
                    },
                ],
                series: Vec::new(),
                approaches: Vec::new(),
            });
        }

        let (start_ms, end_ms) = effective_window_bounds(query);
        let bucket = query_bucket_size_ms(start_ms, end_ms);
        let base_where = format!(
            "{} AND flow_type = '{}'",
            base_where_clause(query.merchant_id.as_deref(), start_ms, end_ms),
            FlowType::DecideGatewayDecision.as_str(),
        );

        let count_row = self
            .client
            .query(&format!(
                "SELECT \
                count() AS total, \
                countIf(lowerUTF8(ifNull(status, '')) = 'failure') AS failures \
             FROM {DOMAIN_TABLE} {base_where}"
            ))
            .fetch_one::<CountTileRow>()
            .await
            .map_err(|_| self.store_error())?;

        let series = self
            .client
            .query(&format!(
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
                count: row.count as i64,
            })
            .collect();

        let approaches = self
            .client
            .query(&format!(
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
                count: row.count as i64,
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
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsRoutingStatsResponse, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let bucket = query_bucket_size_ms(start_ms, end_ms);
        let base_where = base_where_clause(query.merchant_id.as_deref(), start_ms, end_ms);
        let gateway_share = self
            .client
            .query(&format!(
                "SELECT \
                intDiv(created_at_ms, {bucket}) * {bucket} AS bucket_ms, \
                ifNull(gateway, 'unknown') AS gateway, \
                count() AS count \
             FROM {DOMAIN_TABLE} {base_where} AND flow_type = '{decision_flow_type}' \
             GROUP BY bucket_ms, gateway \
             ORDER BY bucket_ms ASC, gateway ASC",
                decision_flow_type = FlowType::DecideGatewayDecision.as_str(),
            ))
            .fetch_all::<GatewaySharePointRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| AnalyticsGatewaySharePoint {
                bucket_ms: row.bucket_ms,
                gateway: row.gateway,
                count: row.count as i64,
            })
            .collect();

        Ok(AnalyticsRoutingStatsResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: format_range(query),
            gateway_share,
            top_rules: self.load_rule_hits(query, Some(10)).await?,
            sr_trend: self.load_score_series(query).await?,
            available_filters: self.load_filter_options(query).await?,
        })
    }

    async fn log_summaries(
        &self,
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

        let errors = self.load_error_summaries(query, Some(10)).await?;
        let total_errors = errors.iter().map(|entry| entry.count).sum();
        let (start_ms, end_ms) = effective_window_bounds(query);
        let filter_suffix = analytics_dimension_filters(query);
        let base_where = format!(
            "{}{} AND flow_type IN {}",
            base_where_clause(query.merchant_id.as_deref(), start_ms, end_ms),
            filter_suffix,
            flow_type_list_sql(OVERVIEW_ERROR_FLOW_TYPES),
        );
        let page = query.page.max(1);
        let page_size = query.page_size.clamp(1, 50);
        let offset = (page - 1) * page_size;
        let samples = self
            .client
            .query(&format!(
                "SELECT \
                ifNull(route, 'unknown') AS route, merchant_id, payment_id, request_id, \
                global_request_id, trace_id, gateway, routing_approach, status, error_code, \
                error_message, flow_type, created_at_ms \
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
                global_request_id: row.global_request_id,
                trace_id: row.trace_id,
                gateway: row.gateway,
                routing_approach: row.routing_approach,
                status: row.status,
                error_code: row.error_code,
                error_message: row.error_message,
                flow_type: Some(row.flow_type),
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
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        self.load_payment_audit_response(query, false).await
    }

    async fn preview_trace(
        &self,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        self.load_payment_audit_response(query, true).await
    }
}

impl ClickHouseAnalyticsStore {
    async fn load_payment_audit_response(
        &self,
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

        let summary_rows = self.load_audit_summaries(query, preview_only).await?;
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
            self.load_audit_timeline(query, preview_only, &lookup_key)
                .await?
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

    async fn load_score_snapshots(
        &self,
        query: &AnalyticsQuery,
        limit: Option<usize>,
    ) -> Result<Vec<GatewayScoreSnapshot>, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let filter_suffix = score_filters(query);
        let limit_clause = limit
            .map(|value| format!("LIMIT {value}"))
            .unwrap_or_default();
        let sql = format!(
            "SELECT \
                ifNull(merchant_id, '') AS merchant_id, \
                ifNull(payment_method_type, '') AS payment_method_type, \
                ifNull(payment_method, '') AS payment_method, \
                ifNull(gateway, '') AS gateway, \
                ifNull(argMax(score_value, created_at_ms), 0.0) AS score_value, \
                ifNull(argMax(sigma_factor, created_at_ms), 0.0) AS sigma_factor, \
                ifNull(argMax(average_latency, created_at_ms), 0.0) AS average_latency, \
                ifNull(argMax(tp99_latency, created_at_ms), 0.0) AS tp99_latency, \
                ifNull(argMax(transaction_count, created_at_ms), 0) AS transaction_count, \
                max(created_at_ms) AS last_updated_ms \
             FROM {DOMAIN_TABLE} \
             WHERE created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND flow_type IN {score_flow_types} {filter_suffix} \
             GROUP BY merchant_id, payment_method_type, payment_method, gateway \
             ORDER BY score_value DESC, last_updated_ms DESC \
             {limit_clause}",
            score_flow_types = flow_type_list_sql(OVERVIEW_SCORE_FLOW_TYPES),
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
                avg(ifNull(score_value, 0.0)) AS score_value \
             FROM {DOMAIN_TABLE} \
             WHERE created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND flow_type IN {score_flow_types} {filter_suffix} \
             GROUP BY bucket_ms, merchant_id, payment_method_type, payment_method, gateway \
             ORDER BY bucket_ms ASC, gateway ASC",
            score_flow_types = flow_type_list_sql(OVERVIEW_SCORE_FLOW_TYPES),
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
        query: &AnalyticsQuery,
        limit: Option<usize>,
    ) -> Result<Vec<AnalyticsErrorSummary>, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let filter_suffix = analytics_dimension_filters(query);
        let limit_clause = limit
            .map(|value| format!("LIMIT {value}"))
            .unwrap_or_default();
        let sql = format!(
            "SELECT \
                ifNull(route, 'unknown') AS route, \
                ifNull(error_code, 'unknown') AS error_code, \
                ifNull(error_message, 'unknown') AS error_message, \
                count() AS count, \
                max(created_at_ms) AS last_seen_ms \
             FROM {DOMAIN_TABLE} \
             WHERE created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND flow_type IN {error_flow_types} \
               {filter_suffix} \
             GROUP BY route, error_code, error_message \
             ORDER BY count DESC, last_seen_ms DESC \
             {limit_clause}",
            filter_suffix = merchant_filter(query.merchant_id.as_deref()) + &filter_suffix,
            error_flow_types = flow_type_list_sql(OVERVIEW_ERROR_FLOW_TYPES),
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
                count: row.count as i64,
                last_seen_ms: row.last_seen_ms,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_rule_hits(
        &self,
        query: &AnalyticsQuery,
        limit: Option<usize>,
    ) -> Result<Vec<AnalyticsRuleHit>, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let limit_clause = limit
            .map(|value| format!("LIMIT {value}"))
            .unwrap_or_default();
        let sql = format!(
            "SELECT ifNull(rule_name, 'unknown') AS rule_name, count() AS count \
             FROM {DOMAIN_TABLE} \
             WHERE created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND flow_type = '{rule_hit_flow_type}' {merchant_filter} \
             GROUP BY rule_name \
             ORDER BY count DESC, rule_name ASC \
             {limit_clause}",
            merchant_filter = merchant_filter(query.merchant_id.as_deref()),
            rule_hit_flow_type = FlowType::DecideGatewayRuleHit.as_str(),
        );
        self.client
            .query(&sql)
            .fetch_all::<RuleHitRow>()
            .await
            .map_err(|_| self.store_error())?
            .into_iter()
            .map(|row| AnalyticsRuleHit {
                rule_name: row.rule_name,
                count: row.count as i64,
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_filter_options(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<RoutingFilterOptions, ApiError> {
        let (start_ms, end_ms) = effective_window_bounds(query);
        let sql = format!(
            "SELECT DISTINCT payment_method_type, payment_method, card_network, card_is_in, country, currency, auth_type, gateway \
             FROM {DOMAIN_TABLE} \
             WHERE created_at_ms >= {start_ms} AND created_at_ms <= {end_ms} \
               AND flow_type IN {score_flow_types} {merchant_filter}",
            merchant_filter = merchant_filter(query.merchant_id.as_deref()),
            score_flow_types = flow_type_list_sql(OVERVIEW_SCORE_FLOW_TYPES),
        );
        let rows = self
            .client
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
                    ifNull(if(payment_id != '' AND payment_id IS NOT NULL, payment_id, request_id), '') AS lookup_key, \
                    payment_id, request_id, merchant_id, created_at_ms, status, gateway, event_stage, route \
                FROM {DOMAIN_TABLE} {audit_where} \
             ) \
             WHERE lookup_key != '' \
             GROUP BY lookup_key \
             ORDER BY last_seen_ms DESC, event_count DESC",
            audit_where = payment_audit_where_clause(query, preview_only),
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
                routes: row
                    .routes
                    .into_iter()
                    .map(payment_audit_route_label)
                    .collect(),
            })
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    async fn load_audit_timeline(
        &self,
        query: &PaymentAuditQuery,
        preview_only: bool,
        lookup_key: &str,
    ) -> Result<Vec<PaymentAuditEvent>, ApiError> {
        let sql = format!(
            "SELECT \
                event_id, flow_type, event_stage, route, merchant_id, payment_id, request_id, \
                global_request_id, trace_id, payment_method_type, payment_method, gateway, \
                routing_approach, rule_name, status, error_code, error_message, score_value, \
                sigma_factor, average_latency, tp99_latency, transaction_count, details, \
                created_at_ms \
             FROM {DOMAIN_TABLE} {audit_where} \
               AND (payment_id = '{lookup_key}' OR request_id = '{lookup_key}') \
             ORDER BY created_at_ms ASC, event_id ASC",
            audit_where = payment_audit_where_clause(query, preview_only),
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
                flow_type: row.flow_type,
                event_stage: row.event_stage,
                route: row.route,
                merchant_id: row.merchant_id,
                payment_id: row.payment_id,
                request_id: row.request_id,
                global_request_id: row.global_request_id,
                trace_id: row.trace_id,
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
    total: u64,
    score_count: u64,
    rule_hit_count: u64,
    error_count: u64,
}

fn effective_window_bounds(query: &AnalyticsQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let min_start_ms = end_ms.saturating_sub(MAX_ANALYTICS_LOOKBACK_MS);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()))
        .max(min_start_ms);
    (start_ms, end_ms)
}

fn effective_payment_audit_window_bounds(query: &PaymentAuditQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let min_start_ms = end_ms.saturating_sub(MAX_ANALYTICS_LOOKBACK_MS);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()))
        .max(min_start_ms);
    (start_ms, end_ms)
}

fn query_bucket_size_ms(start_ms: i64, end_ms: i64) -> i64 {
    let window_ms = end_ms.saturating_sub(start_ms);
    match window_ms {
        0..=900_000 => 60 * 1000,
        900_001..=3_600_000 => 5 * 60 * 1000,
        3_600_001..=86_400_000 => 15 * 60 * 1000,
        86_400_001..=259_200_000 => 60 * 60 * 1000,
        259_200_001..=2_592_000_000 => 3 * 60 * 60 * 1000,
        2_592_000_001..=15_552_000_000 => 24 * 60 * 60 * 1000,
        _ => 7 * 24 * 60 * 60 * 1000,
    }
}

fn payment_audit_range(query: &PaymentAuditQuery) -> String {
    match query.range {
        AnalyticsRange::M15 => "15m".to_string(),
        AnalyticsRange::H1 => "1h".to_string(),
        AnalyticsRange::H24 => "24h".to_string(),
        AnalyticsRange::D30 => "30d".to_string(),
        AnalyticsRange::M18 => "18mo".to_string(),
    }
}

fn base_where_clause(merchant_id: Option<&str>, start_ms: i64, end_ms: i64) -> String {
    format!(
        "WHERE created_at_ms >= {start_ms} AND created_at_ms <= {end_ms}{}",
        merchant_filter(merchant_id),
    )
}

fn merchant_filter(merchant_id: Option<&str>) -> String {
    merchant_id
        .map(|value| format!(" AND merchant_id = '{}'", escape_sql(value)))
        .unwrap_or_default()
}

fn score_filters(query: &AnalyticsQuery) -> String {
    let mut filters = merchant_filter(query.merchant_id.as_deref());
    filters.push_str(&analytics_dimension_filters(query));
    filters
}

fn analytics_dimension_filters(query: &AnalyticsQuery) -> String {
    let mut filters = String::new();
    if let Some(value) = &query.payment_method_type {
        filters.push_str(&format!(
            " AND payment_method_type = '{}'",
            escape_sql(value)
        ));
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

fn payment_audit_where_clause(query: &PaymentAuditQuery, preview_only: bool) -> String {
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    let mut filters = vec![
        format!("created_at_ms >= {start_ms}"),
        format!("created_at_ms <= {end_ms}"),
    ];
    if let Some(merchant_id) = &query.merchant_id {
        filters.push(format!("merchant_id = '{}'", escape_sql(merchant_id)));
    }
    if preview_only {
        filters.push("route = 'routing_evaluate'".to_string());
        filters.push(format!(
            "flow_type IN {}",
            flow_type_list_sql(PAYMENT_AUDIT_PREVIEW_FLOW_TYPES)
        ));
    } else {
        filters.push(format!(
            "flow_type IN {}",
            flow_type_list_sql(PAYMENT_AUDIT_DYNAMIC_FLOW_TYPES)
        ));
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
    if let Some(flow_type) = &query.flow_type {
        filters.push(format!("flow_type = '{}'", escape_sql(flow_type)));
    }
    if let Some(error_code) = &query.error_code {
        filters.push(format!("error_code = '{}'", escape_sql(error_code)));
    }
    format!("WHERE {}", filters.join(" AND "))
}

fn map_route_hits(rows: Vec<RouteHitRow>) -> Vec<AnalyticsRouteHit> {
    let mut counts = std::collections::HashMap::new();
    for row in rows {
        counts.insert(
            row.route.unwrap_or_else(|| "unknown".to_string()),
            row.count as i64,
        );
    }
    [
        AnalyticsRoute::DecideGateway,
        AnalyticsRoute::UpdateGatewayScore,
        AnalyticsRoute::RoutingEvaluate,
    ]
    .into_iter()
    .map(|route| AnalyticsRouteHit {
        route: route.overview_label().unwrap_or(route.as_str()).to_string(),
        count: counts.get(route.as_str()).copied().unwrap_or(0),
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
    AnalyticsRoute::from_stored_value(&route)
        .map(|route| route.payment_audit_label().to_string())
        .unwrap_or(route)
}

fn escape_sql(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn flow_type_list_sql(flow_types: &[FlowType]) -> String {
    format!(
        "({})",
        flow_types
            .iter()
            .map(|flow_type| format!("'{}'", flow_type.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}
