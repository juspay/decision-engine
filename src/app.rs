use crate::redis::commands::RedisConnectionWrapper;
use axum::http::HeaderValue;
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post},
};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use error_stack::ResultExt;
use std::sync::Arc;
use std::time::Instant;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::OnceCell as TokioOnceCell;
use tower::ServiceBuilder;
use tower_http::trace as tower_trace;

use crate::{
    api_client::ApiClient,
    config::{self, GlobalConfig, TenantConfig},
    error, logger, middleware as custom_middleware, routes, storage,
    tenant::GlobalAppState,
    utils,
};

use once_cell::sync::OnceCell;
pub static APP_STATE: OnceCell<Arc<GlobalAppState>> = OnceCell::new();
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;
pub async fn get_tenant_app_state() -> Arc<TenantAppState> {
    let app_state = APP_STATE.get().expect("GlobalAppState not set");
    let tenant_app_state = GlobalAppState::get_app_state_of_tenant(app_state, "public")
        .await
        .unwrap();
    tenant_app_state
}

type Storage = storage::Storage;

async fn ensure_request_id(mut request: Request<Body>, next: Next) -> Response {
    let header_value = request
        .headers()
        .get(storage::consts::X_REQUEST_ID)
        .filter(|value| !value.as_bytes().is_empty())
        .cloned()
        .unwrap_or_else(generate_request_id_header_value);

    request
        .headers_mut()
        .insert(storage::consts::X_REQUEST_ID, header_value.clone());

    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(storage::consts::X_REQUEST_ID, header_value);

    response
}

fn generate_request_id_header_value() -> HeaderValue {
    loop {
        let request_id = storage::utils::generate_uuid();
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            return value;
        }
    }
}

fn extract_json_path(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let nested_value = path
        .iter()
        .try_fold(value, |current, key| current.get(*key))?;

    nested_value.as_str().map(str::to_string)
}

fn parse_request_metadata(
    captured: &crate::analytics::CapturedBody,
) -> (Option<String>, Option<String>, Option<String>) {
    let request_json = serde_json::from_slice::<serde_json::Value>(&captured.bytes).ok();
    let merchant_id = request_json.as_ref().and_then(|value| {
        [
            &["merchant_id"][..],
            &["merchantId"][..],
            &["created_by"][..],
            &["createdBy"][..],
        ]
        .into_iter()
        .find_map(|path| extract_json_path(value, path))
    });
    let payment_id = request_json.as_ref().and_then(|value| {
        [
            &["payment_id"][..],
            &["paymentId"][..],
            &["payment_info", "payment_id"][..],
            &["paymentInfo", "paymentId"][..],
        ]
        .into_iter()
        .find_map(|path| extract_json_path(value, path))
    });
    let auth_type = request_json.as_ref().and_then(|value| {
        [
            &["authentication_type"][..],
            &["authenticationType"][..],
            &["auth_type"][..],
            &["authType"][..],
            &["payment_info", "authentication_type"][..],
            &["payment_info", "auth_type"][..],
            &["paymentInfo", "authenticationType"][..],
            &["paymentInfo", "authType"][..],
        ]
        .into_iter()
        .find_map(|path| extract_json_path(value, path))
    });
    (merchant_id, payment_id, auth_type)
}

fn parse_response_error(
    status_code: u16,
    captured: &crate::analytics::CapturedBody,
) -> Option<serde_json::Value> {
    if status_code < 400 {
        return None;
    }
    serde_json::from_slice::<serde_json::Value>(&captured.bytes).ok()
}

async fn capture_api_event(
    State(global_state): State<Arc<GlobalAppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !global_state.analytics_runtime.write_enabled() {
        return next.run(request).await;
    }

    let path = request.uri().path().to_string();
    let Some(flow_context) = crate::analytics::classify_request(request.method(), &path) else {
        return next.run(request).await;
    };

    let started_at = Instant::now();
    let (parts, body) = request.into_parts();
    let (request_body, request_handle) = crate::analytics::CaptureBody::new(body);

    let request_id = parts
        .headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let global_request_id = crate::analytics::global_request_id_from_headers(&parts.headers);
    let trace_id = crate::analytics::trace_id_from_headers(&parts.headers);
    let user_agent = parts
        .headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let ip_addr = parts
        .headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(',').next().unwrap_or(value).trim().to_string())
        .or_else(|| {
            parts
                .headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        });
    let method = parts.method.to_string();

    let response = next
        .run(Request::from_parts(parts, Body::new(request_body)))
        .await;
    let status_code = response.status().as_u16();
    let (response_parts, response_body) = response.into_parts();
    let (response_body, response_handle) = crate::analytics::CaptureBody::new(response_body);

    let runtime = global_state.analytics_runtime.clone();
    tokio::spawn(async move {
        let request_capture = request_handle.wait().await;
        let response_capture = response_handle.wait().await;
        let (merchant_id, payment_id, auth_type) = parse_request_metadata(&request_capture);
        let error = parse_response_error(status_code, &response_capture);

        runtime.enqueue_api_event(crate::analytics::ApiEvent {
            event_id: crate::analytics::next_event_id(),
            merchant_id,
            payment_id,
            api_flow: flow_context.api_flow,
            flow_type: flow_context.flow_type,
            created_at_timestamp: crate::analytics::now_ms(),
            request_id,
            global_request_id,
            trace_id,
            latency: started_at.elapsed().as_millis() as u64,
            status_code,
            auth_type,
            request: String::from_utf8_lossy(&request_capture.bytes).to_string(),
            user_agent,
            ip_addr,
            url_path: path,
            response: Some(String::from_utf8_lossy(&response_capture.bytes).to_string()),
            error,
            http_method: method,
        });
    });

    Response::from_parts(response_parts, Body::new(response_body))
}

///
/// TenantAppState:
///
///
/// The tenant specific appstate that is passed to main storage endpoints
///
#[derive(Clone)]
pub struct TenantAppState {
    pub db: Storage,
    pub redis_conn: Arc<RedisConnectionWrapper>,
    pub config: config::TenantConfig,
    pub api_client: ApiClient,
    pub pm_filter_graph_bundle:
        Arc<TokioOnceCell<Arc<crate::euclid::pm_filter_graph::PmFilterGraphBundle>>>,
}

#[allow(clippy::expect_used)]
impl TenantAppState {
    ///
    /// Construct new app state with configuration
    ///
    pub async fn new(
        global_config: &GlobalConfig,
        tenant_config: TenantConfig,
        api_client: ApiClient,
    ) -> error_stack::Result<Self, error::ConfigurationError> {
        let db = storage::Storage::new(
            #[cfg(feature = "mysql")]
            &global_config.database,
            #[cfg(feature = "postgres")]
            &global_config.pg_database,
            &tenant_config.tenant_secrets.schema,
        )
        .await
        .change_context(error::ConfigurationError::DatabaseError)?;

        let redis_conn = redis_interface::RedisConnectionPool::new(&global_config.redis)
            .await
            .expect("Failed to create Redis connection Pool");

        Ok(Self {
            db,
            redis_conn: Arc::new(RedisConnectionWrapper::new(
                redis_conn,
                global_config.compression_filepath.clone(),
            )),
            api_client,
            config: tenant_config,
            pm_filter_graph_bundle: Arc::new(TokioOnceCell::new()),
        })
    }

    pub async fn get_pm_filter_graph_bundle(
        &self,
    ) -> Option<Arc<crate::euclid::pm_filter_graph::PmFilterGraphBundle>> {
        self.pm_filter_graph_bundle
            .get_or_try_init(|| async {
                let bundle = crate::euclid::pm_filter_graph::build_pm_filter_graph_bundle(
                    &self.config.pm_filters,
                    self.config.routing_config.as_ref(),
                )
                .map_err(|err| {
                    logger::error!(
                        tenant_id = %self.config.tenant_id,
                        error = %err,
                        "Failed to build pm_filters constraint graph; failing open"
                    );
                    err
                })?;

                logger::info!(
                    tenant_id = %self.config.tenant_id,
                    explicit_connector_count = bundle.explicit_connectors.len(),
                    has_default_rules = bundle.has_default_rules,
                    graph_node_count = bundle.node_count,
                    graph_edge_count = bundle.edge_count,
                    "pm_filters constraint graph built successfully"
                );

                Ok::<Arc<crate::euclid::pm_filter_graph::PmFilterGraphBundle>, String>(Arc::new(
                    bundle,
                ))
            })
            .await
            .ok()
            .cloned()
    }
}

///
/// The server responsible for the custodian APIs and main open_router APIs this will perform all storage, retrieval and
/// deletion operation
///
pub async fn server_builder(
    global_app_state: Arc<GlobalAppState>,
) -> Result<(), error::ConfigurationError>
where
{
    let socket_addr = std::net::SocketAddr::new(
        global_app_state.global_config.server.host.parse()?,
        global_app_state.global_config.server.port,
    );

    if APP_STATE.set(global_app_state.clone()).is_err() {
        panic!("Failed to set global app state");
    }

    // Create a signal stream for SIGTERM
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");

    // Create an axum_server handle for graceful shutdown
    let handle = Handle::new();

    // Spawn a task to listen for SIGTERM and trigger shutdown
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        sigterm.recv().await;
        logger::error!("SIGTERM signal received, shutting down...");
        let app_state = APP_STATE.get().expect("GlobalAppState not set");
        app_state.set_not_ready(); // Set readiness flag to false
        handle_clone.shutdown(); // Trigger axum_server shutdown
    });

    // Routes that require API key authentication
    let protected_router = axum::Router::new()
        .route(
            "/routing/create",
            axum::routing::post(crate::euclid::handlers::routing_rules::routing_create),
        )
        .route(
            "/routing/activate",
            axum::routing::post(crate::euclid::handlers::routing_rules::activate_routing_rule),
        )
        .route(
            "/routing/list/:created_by",
            axum::routing::post(
                crate::euclid::handlers::routing_rules::list_all_routing_algorithm_id,
            ),
        )
        .route(
            "/routing/list/active/:created_by",
            axum::routing::post(
                crate::euclid::handlers::routing_rules::list_active_routing_algorithm,
            ),
        )
        .route(
            "/routing/evaluate",
            axum::routing::post(crate::euclid::handlers::routing_rules::routing_evaluate),
        )
        .route(
            "/decision_gateway",
            post(routes::decision_gateway::decision_gateway),
        )
        .route(
            "/rule/create",
            post(routes::rule_configuration::create_rule_config),
        )
        .route(
            "/rule/get",
            post(routes::rule_configuration::get_rule_config),
        )
        .route(
            "/rule/update",
            post(routes::rule_configuration::update_rule_config),
        )
        .route(
            "/rule/delete",
            post(routes::rule_configuration::delete_rule_config),
        )
        .route(
            "/merchant-account/:merchant-id",
            get(routes::merchant_account_config::get_merchant_config),
        )
        .route(
            "/merchant-account/:merchant-id",
            delete(routes::merchant_account_config::delete_merchant_config),
        )
        .route(
            "/merchant-account/:merchant-id/debit-routing",
            get(routes::merchant_account_config::get_debit_routing),
        )
        .route(
            "/merchant-account/:merchant-id/debit-routing",
            post(routes::merchant_account_config::update_debit_routing),
        )
        .route(
            "/config-sr-dimension",
            axum::routing::post(crate::euclid::handlers::routing_rules::config_sr_dimensions),
        )
        .route(
            "/config/routing-keys",
            axum::routing::get(crate::euclid::handlers::routing_rules::get_routing_config),
        )
        .route("/update-score", post(routes::update_score::update_score))
        .route(
            "/decide-gateway",
            post(routes::decide_gateway::decide_gateway),
        )
        .route(
            "/routing/hybrid",
            post(routes::hybrid_routing::hybrid_routing_evaluate),
        )
        .route(
            "/update-gateway-score",
            post(routes::update_gateway_score::update_gateway_score),
        )
        .route("/api-key/create", post(routes::api_key::create_api_key))
        .route(
            "/api-key/list/:merchant_id",
            get(routes::api_key::list_api_keys),
        )
        .route("/api-key/:key_id", delete(routes::api_key::revoke_api_key))
        .nest("/analytics", routes::analytics::serve())
        .layer(middleware::from_fn(custom_middleware::authenticate));

    // Routes that do not require authentication (public)
    let public_router = axum::Router::new()
        .route(
            "/merchant-account/create",
            post(routes::merchant_account_config::create_merchant_config),
        )
        .route("/auth/signup", post(routes::user_auth::signup))
        .route("/auth/login", post(routes::user_auth::login))
        .route("/auth/logout", post(routes::user_auth::logout))
        .route("/auth/me", get(routes::user_auth::me))
        .route("/auth/merchants", get(routes::user_auth::list_merchants))
        .route(
            "/auth/switch-merchant",
            post(routes::user_auth::switch_merchant),
        )
        .route(
            "/onboarding/merchant",
            post(routes::user_auth::create_merchant),
        )
        .route("/merchant/members", get(routes::user_auth::list_members))
        .route(
            "/merchant/members/invite",
            post(routes::user_auth::invite_member),
        );

    let router = axum::Router::new()
        .merge(protected_router)
        .merge(public_router);

    let middleware = ServiceBuilder::new()
        .layer(middleware::from_fn(ensure_request_id))
        .layer(middleware::from_fn_with_state(
            global_app_state.clone(),
            capture_api_event,
        ))
        .layer(
            tower_trace::TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| utils::record_fields_from_header(request))
                .on_request(tower_trace::DefaultOnRequest::new().level(tracing::Level::INFO))
                .on_response(
                    tower_trace::DefaultOnResponse::new()
                        .level(tracing::Level::INFO)
                        .latency_unit(tower_http::LatencyUnit::Micros),
                )
                .on_failure(
                    tower_trace::DefaultOnFailure::new()
                        .latency_unit(tower_http::LatencyUnit::Micros)
                        .level(tracing::Level::ERROR),
                ),
        );

    let router = router
        .nest("/health", routes::health::serve())
        .layer(middleware)
        .with_state(global_app_state.clone());

    logger::info!(
        category = "SERVER",
        action = "main_server_startup",
        bind_address = %socket_addr,
        tls_enabled = global_app_state.global_config.tls.is_some(),
        request_id_header = storage::consts::X_REQUEST_ID,
        "Main HTTP server listening"
    );

    if let Some(tls_config) = &global_app_state.global_config.tls {
        let tcp_listener = std::net::TcpListener::bind(socket_addr)?;
        let rusttls_config =
            RustlsConfig::from_pem_file(&tls_config.certificate, &tls_config.private_key).await?;

        axum_server::from_tcp_rustls(tcp_listener, rusttls_config)
            .handle(handle)
            .serve(router.into_make_service())
            .await?;
    } else {
        let tcp_listener = std::net::TcpListener::bind(socket_addr)?;

        axum_server::from_tcp(tcp_listener)
            .handle(handle) // Attach the handle for graceful shutdown
            .serve(router.into_make_service())
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use axum::body::Bytes;

    use super::parse_request_metadata;

    #[test]
    fn parse_request_metadata_supports_decide_gateway_shape() {
        let captured = crate::analytics::CapturedBody {
            bytes: Bytes::from_static(
                br#"{
                    "merchantId":"demo_merchant",
                    "paymentInfo":{
                        "paymentId":"pay_001",
                        "authType":"THREE_DS"
                    }
                }"#,
            ),
        };

        let (merchant_id, payment_id, auth_type) = parse_request_metadata(&captured);

        assert_eq!(merchant_id.as_deref(), Some("demo_merchant"));
        assert_eq!(payment_id.as_deref(), Some("pay_001"));
        assert_eq!(auth_type.as_deref(), Some("THREE_DS"));
    }

    #[test]
    fn parse_request_metadata_keeps_snake_case_support() {
        let captured = crate::analytics::CapturedBody {
            bytes: Bytes::from_static(
                br#"{
                    "merchant_id":"demo_merchant",
                    "payment_id":"pay_002",
                    "authentication_type":"NO_THREE_DS"
                }"#,
            ),
        };

        let (merchant_id, payment_id, auth_type) = parse_request_metadata(&captured);

        assert_eq!(merchant_id.as_deref(), Some("demo_merchant"));
        assert_eq!(payment_id.as_deref(), Some("pay_002"));
        assert_eq!(auth_type.as_deref(), Some("NO_THREE_DS"));
    }
}
