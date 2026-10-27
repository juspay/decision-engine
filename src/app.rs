use crate::redis::commands::RedisConnectionWrapper;
use axum::http::HeaderValue;
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use error_stack::ResultExt;
use std::sync::Arc;
use std::time::Instant;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::OnceCell as TokioOnceCell;
use tower::ServiceBuilder;
use tower_http::trace as tower_trace;

use crate::euclid::handlers::routing_rules;
use crate::middleware::metered;
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

/// Count responses for matched routes without emitting an additional failure event.
async fn record_api_response(request: Request<Body>, next: Next) -> Response {
    // Keep metric labels bounded to registered routes; unmatched requests still get completion logs.
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str().to_owned());
    let method = request.method().to_string();
    let response = next.run(request).await;
    if let Some(route) = route {
        let status = response.status();
        crate::metrics::API_RESPONSE_COUNTER
            .with_label_values(&[route.as_str(), method.as_str(), status.as_str()])
            .inc();
    }
    response
}

/// Generate IDs before tracing and keep request logs outside authentication.
fn with_request_logging<S>(router: axum::Router<S>) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let middleware = ServiceBuilder::new()
        .layer(middleware::from_fn(ensure_request_id))
        .layer(
            tower_trace::TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| utils::record_fields_from_header(request))
                .on_request(tower_trace::DefaultOnRequest::new().level(tracing::Level::INFO))
                .on_response(
                    |response: &Response, latency: std::time::Duration, span: &tracing::Span| {
                        span.in_scope(|| {
                            logger::info!(
                                status_code = response.status().as_u16(),
                                latency_ms = latency.as_millis() as u64,
                                "finished processing request"
                            );
                        });
                    },
                )
                // Application errors are logged at their source; completion records status.
                .on_failure(()),
        )
        .layer(middleware::from_fn(record_api_response));
    router.layer(middleware)
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
/// Routes that act on one merchant. Every request authenticates (a dashboard session, a merchant
/// api key, or the admin secret on behalf of a merchant), and a session is held to its own
/// merchant: by [`custom_middleware::enforce_merchant_scope`] for the `:merchant_id` path
/// parameter, and by the [`crate::auth::AuthorizedMerchant`] each handler resolves before it
/// reaches per-merchant storage.
fn merchant_routes() -> axum::Router<Arc<GlobalAppState>> {
    axum::Router::new()
        .route(
            "/routing/create",
            metered!(post, routing_rules::routing_create),
        )
        .route(
            "/routing/activate",
            metered!(post, routing_rules::activate_routing_rule),
        )
        .route(
            "/routing/deactivate",
            metered!(post, routing_rules::deactivate_routing_rule),
        )
        .route(
            "/routing/update",
            metered!(post, routing_rules::update_routing_rule),
        )
        // Rule deletion is disabled for parity with the Hyperswitch dashboard, which
        // cannot delete routing rules yet (juspay/hyperswitch-control-center#5495,
        // item 4). Re-enable by uncommenting once both dashboards can offer it.
        // .route(
        //     "/routing/delete",
        //     metered!(post, routing_rules::delete_routing_rule),
        // )
        .route(
            "/routing/list/:merchant_id",
            metered!(post, routing_rules::list_all_routing_algorithm_id),
        )
        .route(
            "/routing/list/active/:merchant_id",
            metered!(post, routing_rules::list_active_routing_algorithm),
        )
        .route(
            "/routing/evaluate",
            metered!(post, routing_rules::routing_evaluate),
        )
        .route(
            "/routing/evaluate/batch",
            metered!(post, routing_rules::routing_evaluate_batch),
        )
        .route(
            "/decision_gateway",
            metered!(post, routes::decision_gateway::decision_gateway),
        )
        .route(
            "/rule/create",
            metered!(post, routes::rule_configuration::create_rule_config),
        )
        .route(
            "/rule/get",
            metered!(post, routes::rule_configuration::get_rule_config),
        )
        .route(
            "/rule/update",
            metered!(post, routes::rule_configuration::update_rule_config),
        )
        .route(
            "/rule/delete",
            metered!(post, routes::rule_configuration::delete_rule_config),
        )
        .route(
            "/merchant-account/:merchant_id",
            metered!(
                get,
                routes::merchant_account_config::get_merchant_config,
                "merchant_account_get"
            ),
        )
        .route(
            "/merchant-account/:merchant_id",
            metered!(
                delete,
                routes::merchant_account_config::delete_merchant_config,
                "merchant_account_delete"
            ),
        )
        .route(
            "/merchant-account/:merchant_id/debit-routing",
            metered!(
                get,
                routes::merchant_account_config::get_debit_routing,
                "merchant_debit_routing_get"
            ),
        )
        .route(
            "/merchant-account/:merchant_id/connectors",
            metered!(
                get,
                routes::connector_credentials::list_connector_credentials
            ),
        )
        .route(
            "/merchant-account/:merchant_id/connectors/:connector/credentials",
            metered!(
                post,
                routes::connector_credentials::set_connector_credentials
            ),
        )
        .route(
            "/merchant-account/:merchant_id/connectors/:connector/credentials/:account",
            metered!(
                delete,
                routes::connector_credentials::delete_connector_credentials
            ),
        )
        .route(
            "/merchant-account/:merchant_id/connectors/:connector/report",
            // Monthly settlement reports run to several GB. The handler streams the body to disk
            // and parses in batches (O(batch) RAM), so lift the body cap to the same hard limit it
            // enforces itself. DefaultBodyLimit otherwise errors the stream at its default.
            metered!(post, routes::report_upload::upload_report).layer(
                axum::extract::DefaultBodyLimit::max(routes::report_upload::MAX_UPLOAD_BYTES),
            ),
        )
        .route(
            // Header preflight: the dashboard posts only the first few KB of the merchant's file to
            // check it parses before committing to a multi-GB upload. Cap the body accordingly —
            // anything larger is a client bug, and the handler stops reading at the cap anyway.
            "/merchant-account/:merchant_id/connectors/:connector/report/validate-headers",
            metered!(post, routes::report_upload::validate_report_headers).layer(
                axum::extract::DefaultBodyLimit::max(
                    crate::cost_ingestion::preflight::HEADER_SAMPLE_BYTES,
                ),
            ),
        )
        .route(
            // Which connectors support report ingestion, straight off the registry — the dashboard's
            // connector picker reads this instead of keeping its own list.
            "/cost-ingestion/connectors",
            metered!(get, routes::report_upload::list_ingest_connectors),
        )
        .route(
            // Preview a *candidate* column mapping: what the merchant's rows actually become under
            // it. Small JSON body (a mapping + a file sample), so the default body limit applies.
            "/merchant-account/:merchant_id/connectors/:connector/report/preview",
            metered!(post, routes::report_upload::preview_column_mapping),
        )
        .route(
            // A settlement source's saved column mapping, applied to every future ingestion of it.
            "/merchant-account/:merchant_id/connectors/:connector/report/column-mapping",
            metered!(get, routes::report_upload::get_column_mapping)
                .merge(metered!(put, routes::report_upload::set_column_mapping))
                .merge(metered!(
                    delete,
                    routes::report_upload::delete_column_mapping
                )),
        )
        .route(
            // "Use a sample file": run a curated demo report (fetched from configured URL) through
            // the same pipeline, so a merchant without a report file can still exercise the flow.
            "/merchant-account/:merchant_id/connectors/:connector/report/sample",
            metered!(post, routes::report_upload::run_sample_report),
        )
        .route(
            "/merchant-account/:merchant_id/connectors/:connector/invoice",
            // Invoices are small (a few hundred lines): buffered and processed synchronously, so the
            // computed cost add-on is returned in the response. Cap the body to reject a settlement
            // report accidentally uploaded here.
            metered!(post, routes::invoice_upload::upload_invoice).layer(
                axum::extract::DefaultBodyLimit::max(routes::invoice_upload::MAX_INVOICE_BYTES),
            ),
        )
        .route(
            "/merchant-account/:merchant_id/connectors/:connector/invoice-addon",
            metered!(delete, routes::invoice_upload::delete_addon),
        )
        .route(
            "/merchant-account/:merchant_id/invoice-addons",
            metered!(get, routes::invoice_upload::list_addons),
        )
        .route(
            "/merchant-account/:merchant_id/invoice-reconciliation",
            metered!(get, routes::invoice_upload::get_reconciliation),
        )
        .route(
            "/merchant-account/:merchant_id/connector-fees",
            metered!(get, routes::connector_fees::list_connector_fees),
        )
        .route(
            "/merchant-account/:merchant_id/connectors/:connector/fee-override",
            metered!(put, routes::connector_fees::set_fee_override).merge(metered!(
                delete,
                routes::connector_fees::delete_fee_override
            )),
        )
        .route(
            "/merchant-account/:merchant_id/cost-clusters",
            metered!(get, routes::cost_clusters::list_cost_clusters),
        )
        .route(
            "/merchant-account/:merchant_id/cost-cluster-facets",
            metered!(get, routes::cost_clusters::list_cost_cluster_facets),
        )
        .route(
            "/merchant-account/:merchant_id/cost-clusters/:cluster-key/fee-override",
            metered!(put, routes::cost_clusters::set_cluster_override).merge(metered!(
                delete,
                routes::cost_clusters::delete_cluster_override
            )),
        )
        .route(
            "/merchant-account/:merchant_id/seed-costs",
            metered!(get, routes::seed_costs::get_seed_costs)
                .merge(metered!(put, routes::seed_costs::set_seed_costs))
                .merge(metered!(delete, routes::seed_costs::delete_seed_costs)),
        )
        .route(
            "/merchant-account/:merchant_id/seed-costs/simulate",
            metered!(post, routes::seed_costs::simulate_seed_costs),
        )
        .route(
            "/merchant-account/:merchant_id/cost-coverage",
            metered!(get, routes::cost_coverage::get_cost_coverage),
        )
        .route(
            "/merchant-account/:merchant_id/volume-commitment/dashboard",
            metered!(get, routes::volume_commitment::get_dashboard),
        )
        .route(
            "/merchant-account/:merchant_id/volume-commitment/projection",
            metered!(get, routes::volume_commitment::get_projection),
        )
        .route(
            "/merchant-account/:merchant_id/volume-commitment/impact",
            metered!(get, routes::volume_commitment::get_impact),
        )
        .route(
            "/merchant-account/:merchant_id/volume-commitment/samples",
            metered!(get, routes::volume_commitment::get_samples),
        )
        // Called by the volume-commitment scheduler when a merchant's forecast comes due. Behind
        // the same auth as any other write, and the handler narrows further: the admin secret
        // (which the scheduler presents) may run anything, a session or api key only its own
        // merchant, and the every-merchant sweep is admin-only.
        .route(
            "/volume-commitment/run-forecast",
            metered!(post, routes::volume_commitment::run_forecast),
        )
        .route(
            "/merchant-account/:merchant_id/cost-ingestions",
            metered!(get, routes::report_upload::list_ingestions),
        )
        .route(
            "/merchant-account/:merchant_id/cost-ingestions/:ingestion-id",
            metered!(delete, routes::report_upload::delete_ingestion),
        )
        .route(
            "/merchant-account/:merchant_id/cost-price-changes",
            metered!(get, routes::report_upload::list_price_changes),
        )
        .route(
            "/merchant-account/:merchant_id/debit-routing",
            metered!(
                post,
                routes::merchant_account_config::update_debit_routing,
                "merchant_debit_routing_update"
            ),
        )
        .route(
            "/merchant-account/:merchant_id/features",
            metered!(
                get,
                routes::merchant_account_config::get_merchant_features,
                "merchant_features_get"
            ),
        )
        .route(
            "/merchant-account/:merchant_id/features/:feature",
            metered!(
                post,
                routes::merchant_account_config::update_merchant_feature,
                "merchant_feature_update"
            ),
        )
        .route(
            "/config-sr-dimension",
            metered!(post, routing_rules::config_sr_dimensions),
        )
        .route(
            "/config-sr-dimension/:merchant_id",
            metered!(get, routing_rules::get_sr_dimensions),
        )
        .route(
            "/config/routing-keys",
            metered!(get, routing_rules::get_routing_config),
        )
        .route("/gsm/options", metered!(get, routes::gsm::gsm_options))
        .route(
            "/update-score",
            metered!(post, routes::update_score::update_score),
        )
        .route(
            "/decide-gateway",
            metered!(post, routes::decide_gateway::decide_gateway),
        )
        .route(
            "/routing/hybrid",
            metered!(post, routes::hybrid_routing::hybrid_routing_evaluate),
        )
        .route(
            "/update-gateway-score",
            metered!(post, routes::update_gateway_score::update_gateway_score),
        )
        .route(
            "/gateway-score/reset",
            metered!(post, routes::gateway_score::reset_gateway_scores),
        )
        .route(
            "/api-key/create",
            metered!(post, routes::api_key::create_api_key),
        )
        .route(
            "/api-key/list/:merchant_id",
            metered!(get, routes::api_key::list_api_keys),
        )
        .route(
            "/api-key/:key_id",
            metered!(delete, routes::api_key::revoke_api_key),
        )
        .nest("/analytics", routes::analytics::serve())
        // Ordering matters: layers run outermost-first, so `authenticate` (added last) runs before
        // `enforce_merchant_scope` (added here, inner). The scope guard therefore sees the
        // `AuthContext` that `authenticate` inserts, and rejects a per-merchant session reaching
        // across to another merchant's `:merchant_id` route.
        .layer(middleware::from_fn(
            custom_middleware::enforce_merchant_scope,
        ))
        .layer(middleware::from_fn(custom_middleware::authenticate))
}

/// Deployment-level routes for service callers such as Hyperswitch: provisioning merchants,
/// syncing the org hierarchy, and handing a merchant session to the dashboard. Only the admin
/// secret reaches them, checked once by [`custom_middleware::require_admin_secret`].
fn admin_routes() -> axum::Router<Arc<GlobalAppState>> {
    axum::Router::new()
        .route(
            "/merchant-account/create",
            metered!(
                post,
                routes::merchant_account_config::create_merchant_config,
                "merchant_account_create"
            ),
        )
        .route(
            "/admin/hierarchy/reconcile",
            metered!(post, routes::hierarchy::reconcile_hierarchy),
        )
        .route(
            "/admin/profiles/:profile_id/connectors",
            metered!(post, routes::profile_connectors::replace_profile_connectors),
        )
        .route(
            "/admin/hierarchy/sync",
            metered!(post, routes::hierarchy::sync_hierarchy),
        )
        .route(
            "/auth/admin/merchant-token",
            metered!(post, routes::user_auth::admin_merchant_token),
        )
        .route_layer(middleware::from_fn(custom_middleware::require_admin_secret))
}

/// Routes that take no deployment credential. The account routes sign users in and manage their
/// own sessions, verifying the dashboard JWT in the handler; the settlement webhook is
/// authenticated by the connector's signature.
fn public_routes() -> axum::Router<Arc<GlobalAppState>> {
    axum::Router::new()
        .route(
            "/webhooks/settlement/:merchant_id/:connector",
            metered!(post, routes::settlement_webhook::settlement_webhook),
        )
        .route("/auth/signup", metered!(post, routes::user_auth::signup))
        .route("/auth/login", metered!(post, routes::user_auth::login))
        .route("/auth/logout", metered!(post, routes::user_auth::logout))
        .route("/auth/me", metered!(get, routes::user_auth::me))
        .route(
            "/auth/merchants",
            metered!(get, routes::user_auth::list_merchants),
        )
        .route(
            "/auth/switch-merchant",
            metered!(post, routes::user_auth::switch_merchant),
        )
        .route(
            "/auth/super-admin/enter-merchant",
            metered!(post, routes::user_auth::enter_merchant),
        )
        .route(
            "/auth/super-admin/exit",
            metered!(post, routes::user_auth::exit_merchant),
        )
        .route(
            "/auth/super-admin/lookup",
            metered!(post, routes::user_auth::lookup_merchants),
        )
        .route(
            "/onboarding/merchant",
            metered!(post, routes::user_auth::create_merchant),
        )
        .route(
            "/merchant/members",
            metered!(get, routes::user_auth::list_members),
        )
        .route(
            "/merchant/members/invite",
            metered!(post, routes::user_auth::invite_member),
        )
        .route(
            "/auth/verify-email",
            metered!(get, routes::user_auth::verify_email),
        )
        .route(
            "/auth/forgot-password",
            metered!(post, routes::user_auth::forgot_password),
        )
        .route(
            "/auth/reset-password",
            metered!(post, routes::user_auth::reset_password),
        )
        .route(
            "/auth/change-password",
            metered!(post, routes::user_auth::change_password),
        )
        .route(
            "/auth/admin/merchant-token/exchange",
            metered!(post, routes::user_auth::exchange_merchant_token),
        )
}

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

    // Background job: periodically auto-calibrate SRv3 bucket size + hedging from observed
    // traffic for merchants enrolled via the `autopilot_enabled` flag. Spawned after
    // APP_STATE is set so config reads/writes resolve the tenant app state.
    crate::sr_auto_calibration::spawn(
        global_app_state.analytics_runtime.clone(),
        global_app_state.global_config.sr_auto_calibration.clone(),
    );

    // Background job: drain the settlement-report ingest queue (download → parse → stage).
    // No-op unless `cost_ingestion.worker_enabled` is set.
    crate::cost_ingestion::worker::spawn(
        global_app_state.global_config.cost_ingestion.clone(),
        global_app_state.global_config.analytics.clickhouse.clone(),
    );

    // Background job: poll every pull-based connector's reporting API for ready reports and enqueue
    // them. Connector-agnostic; no-op unless `report_poll_enabled`.
    crate::cost_ingestion::poller::spawn(global_app_state.global_config.cost_ingestion.clone());

    // Background job: refresh the in-house cost serving view from the fitted models, so the
    // multi-objective router can price candidates from our own ingested data.
    crate::cost_ingestion::serving::spawn(
        global_app_state.global_config.analytics.clickhouse.clone(),
    );

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

    let router = axum::Router::new()
        .merge(merchant_routes())
        .merge(admin_routes())
        .merge(public_routes());

    let router =
        router
            .nest("/health", routes::health::serve())
            .layer(middleware::from_fn_with_state(
                global_app_state.clone(),
                capture_api_event,
            ));
    let router = with_request_logging(router).with_state(global_app_state.clone());

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

    // Drain in-flight async feedback tasks before the runtime shuts down.
    // close() stops new tasks from being accepted; wait() is event-driven
    // (no polling) and resolves as soon as the last task completes.
    let tracker = &crate::routes::update_gateway_score::FEEDBACK_TASK_TRACKER;
    tracker.close();
    tokio::select! {
        _ = tracker.wait() => {
            logger::info!("Shutdown: all feedback tasks drained cleanly");
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
            logger::warn!("Shutdown: feedback tasks still pending after 5 s drain — proceeding");
        }
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
