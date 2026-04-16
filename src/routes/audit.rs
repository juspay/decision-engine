use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Jsonb, Nullable, Text};
use diesel_async::RunQueryDsl;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};

use crate::logger;
use crate::tenant::GlobalAppState;

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub range: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RequestsQuery {
    pub endpoint: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub range: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EndpointStats {
    pub endpoint: String,
    pub method: String,
    pub count: i64,
    pub avg_latency_ms: i64,
    pub error_count: i64,
    pub last_hit: Option<String>,
}

#[derive(Debug, QueryableByName)]
struct StatsRow {
    #[diesel(sql_type = Text)]
    endpoint: String,
    #[diesel(sql_type = Text)]
    method: String,
    #[diesel(sql_type = BigInt)]
    count: i64,
    #[diesel(sql_type = BigInt)]
    avg_latency_ms: i64,
    #[diesel(sql_type = BigInt)]
    error_count: i64,
    #[diesel(sql_type = Nullable<Text>)]
    last_hit: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: String,
    pub endpoint: String,
    pub method: String,
    pub request_headers: Option<serde_json::Value>,
    pub request_body: Option<serde_json::Value>,
    pub response_status: i32,
    pub response_body: Option<serde_json::Value>,
    pub latency_ms: i32,
    pub merchant_id: Option<String>,
    pub request_id: String,
}

#[derive(Debug, QueryableByName)]
struct AuditRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    timestamp: String,
    #[diesel(sql_type = Text)]
    endpoint: String,
    #[diesel(sql_type = Text)]
    method: String,
    #[diesel(sql_type = Nullable<Jsonb>)]
    request_headers: Option<serde_json::Value>,
    #[diesel(sql_type = Nullable<Jsonb>)]
    request_body: Option<serde_json::Value>,
    #[diesel(sql_type = Integer)]
    response_status: i32,
    #[diesel(sql_type = Nullable<Jsonb>)]
    response_body: Option<serde_json::Value>,
    #[diesel(sql_type = Integer)]
    latency_ms: i32,
    #[diesel(sql_type = Nullable<Text>)]
    merchant_id: Option<String>,
    #[diesel(sql_type = Text)]
    request_id: String,
}

#[derive(Debug, QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(Debug, Serialize)]
pub struct PaginatedRequests {
    pub data: Vec<AuditLogEntry>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
}

fn range_to_interval(range: &str) -> &str {
    match range {
        "1h" => "1 hour",
        "6h" => "6 hours",
        "24h" => "24 hours",
        "7d" => "7 days",
        _ => "24 hours",
    }
}

pub async fn audit_stats(
    State(state): State<Arc<GlobalAppState>>,
    Query(params): Query<StatsQuery>,
) -> Result<Json<Vec<EndpointStats>>, (StatusCode, String)> {
    let tenant_state = state.get_app_state_of_tenant("public").await.map_err(|e| {
        logger::error!("Failed to get tenant state: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get database connection".to_string(),
        )
    })?;

    let conn = tenant_state.db.get_conn().await.map_err(|e| {
        logger::error!("Failed to get DB connection: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get database connection".to_string(),
        )
    })?;

    let range = params.range.as_deref().unwrap_or("24h");
    let interval = range_to_interval(range);

    let rows: Vec<StatsRow> = if let Some(ref ep) = params.endpoint {
        diesel::sql_query(format!(
            "SELECT endpoint, method, \
             COUNT(*)::bigint as count, \
             COALESCE(AVG(latency_ms)::bigint, 0) as avg_latency_ms, \
             COUNT(*) FILTER (WHERE response_status >= 400)::bigint as error_count, \
             MAX(timestamp)::text as last_hit \
             FROM audit_log \
             WHERE timestamp > NOW() - INTERVAL '{}' AND endpoint LIKE $1 \
             GROUP BY endpoint, method \
             ORDER BY count DESC",
            interval
        ))
        .bind::<Text, _>(format!("%{}%", ep))
        .load(&*conn)
        .await
        .map_err(|e| {
            logger::error!("Failed to query audit stats: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database query failed: {}", e),
            )
        })?
    } else {
        diesel::sql_query(format!(
            "SELECT endpoint, method, \
             COUNT(*)::bigint as count, \
             COALESCE(AVG(latency_ms)::bigint, 0) as avg_latency_ms, \
             COUNT(*) FILTER (WHERE response_status >= 400)::bigint as error_count, \
             MAX(timestamp)::text as last_hit \
             FROM audit_log \
             WHERE timestamp > NOW() - INTERVAL '{}' \
             GROUP BY endpoint, method \
             ORDER BY count DESC",
            interval
        ))
        .load(&*conn)
        .await
        .map_err(|e| {
            logger::error!("Failed to query audit stats: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database query failed: {}", e),
            )
        })?
    };

    let stats = rows
        .into_iter()
        .map(|r| EndpointStats {
            endpoint: r.endpoint,
            method: r.method,
            count: r.count,
            avg_latency_ms: r.avg_latency_ms,
            error_count: r.error_count,
            last_hit: r.last_hit,
        })
        .collect();

    Ok(Json(stats))
}

pub async fn audit_requests(
    State(state): State<Arc<GlobalAppState>>,
    Query(params): Query<RequestsQuery>,
) -> Result<Json<PaginatedRequests>, (StatusCode, String)> {
    let tenant_state = state.get_app_state_of_tenant("public").await.map_err(|e| {
        logger::error!("Failed to get tenant state: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get database connection".to_string(),
        )
    })?;

    let conn = tenant_state.db.get_conn().await.map_err(|e| {
        logger::error!("Failed to get DB connection: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get database connection".to_string(),
        )
    })?;

    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).min(100);
    let offset = (page - 1) * per_page;
    let range = params.range.as_deref().unwrap_or("24h");
    let interval = range_to_interval(range);

    let (rows, total): (Vec<AuditRow>, i64) = if let Some(ref ep) = params.endpoint {
        let count_row: CountRow = diesel::sql_query(format!(
            "SELECT COUNT(*)::bigint as count FROM audit_log \
             WHERE timestamp > NOW() - INTERVAL '{}' AND endpoint = $1",
            interval
        ))
        .bind::<Text, _>(ep.clone())
        .get_result(&*conn)
        .await
        .unwrap_or(CountRow { count: 0 });

        let rows = diesel::sql_query(format!(
            "SELECT id, timestamp::text as timestamp, endpoint, method, \
             request_headers, request_body, response_status, response_body, \
             latency_ms, merchant_id, request_id \
             FROM audit_log \
             WHERE timestamp > NOW() - INTERVAL '{}' AND endpoint = $1 \
             ORDER BY timestamp DESC \
             LIMIT {} OFFSET {}",
            interval, per_page, offset
        ))
        .bind::<Text, _>(ep.clone())
        .load(&*conn)
        .await
        .map_err(|e| {
            logger::error!("Failed to query audit requests: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database query failed: {}", e),
            )
        })?;

        (rows, count_row.count)
    } else {
        let count_row: CountRow = diesel::sql_query(format!(
            "SELECT COUNT(*)::bigint as count FROM audit_log \
             WHERE timestamp > NOW() - INTERVAL '{}'",
            interval
        ))
        .get_result(&*conn)
        .await
        .unwrap_or(CountRow { count: 0 });

        let rows = diesel::sql_query(format!(
            "SELECT id, timestamp::text as timestamp, endpoint, method, \
             request_headers, request_body, response_status, response_body, \
             latency_ms, merchant_id, request_id \
             FROM audit_log \
             WHERE timestamp > NOW() - INTERVAL '{}' \
             ORDER BY timestamp DESC \
             LIMIT {} OFFSET {}",
            interval, per_page, offset
        ))
        .load(&*conn)
        .await
        .map_err(|e| {
            logger::error!("Failed to query audit requests: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database query failed: {}", e),
            )
        })?;

        (rows, count_row.count)
    };

    let data = rows
        .into_iter()
        .map(|r| AuditLogEntry {
            id: r.id,
            timestamp: r.timestamp,
            endpoint: r.endpoint,
            method: r.method,
            request_headers: r.request_headers,
            request_body: r.request_body,
            response_status: r.response_status,
            response_body: r.response_body,
            latency_ms: r.latency_ms,
            merchant_id: r.merchant_id,
            request_id: r.request_id,
        })
        .collect();

    Ok(Json(PaginatedRequests {
        data,
        page,
        per_page,
        total,
    }))
}

pub async fn audit_request_by_id(
    State(state): State<Arc<GlobalAppState>>,
    Path(id): Path<String>,
) -> Result<Json<AuditLogEntry>, (StatusCode, String)> {
    let tenant_state = state.get_app_state_of_tenant("public").await.map_err(|e| {
        logger::error!("Failed to get tenant state: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get database connection".to_string(),
        )
    })?;

    let conn = tenant_state.db.get_conn().await.map_err(|e| {
        logger::error!("Failed to get DB connection: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to get database connection".to_string(),
        )
    })?;

    let row: AuditRow = diesel::sql_query(
        "SELECT id, timestamp::text as timestamp, endpoint, method, \
         request_headers, request_body, response_status, response_body, \
         latency_ms, merchant_id, request_id \
         FROM audit_log WHERE id = $1",
    )
    .bind::<Text, _>(id)
    .get_result(&*conn)
    .await
    .map_err(|e| {
        logger::error!("Failed to query audit request: {:?}", e);
        (
            StatusCode::NOT_FOUND,
            "Audit log entry not found".to_string(),
        )
    })?;

    Ok(Json(AuditLogEntry {
        id: row.id,
        timestamp: row.timestamp,
        endpoint: row.endpoint,
        method: row.method,
        request_headers: row.request_headers,
        request_body: row.request_body,
        response_status: row.response_status,
        response_body: row.response_body,
        latency_ms: row.latency_ms,
        merchant_id: row.merchant_id,
        request_id: row.request_id,
    }))
}
