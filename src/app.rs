use crate::redis::commands::RedisConnectionWrapper;
use axum::{
    extract::Request,
    routing::{delete, get, post},
};
use axum_server::{tls_rustls::RustlsConfig, Handle};
use error_stack::ResultExt;
use masking::ExposeInterface;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tower_http::trace as tower_trace;

use crate::{
    api_client::ApiClient,
    config::{self, GlobalConfig, TenantConfig},
    error, logger,
    pagos_client::PagosApiClient,
    routes, storage,
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
    pub pagos_client: Option<PagosApiClient>,
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
            &global_config.database,
            &tenant_config.tenant_secrets.schema,
        )
        .await
        .change_context(error::ConfigurationError::DatabaseError)?;

        let redis_conn = redis_interface::RedisConnectionPool::new(&global_config.redis)
            .await
            .expect("Failed to create Redis connection Pool");

        let pagos_client = if let Some(pagos_conf) = &tenant_config.pagos_api {
            Some(
                PagosApiClient::new(
                    pagos_conf.base_url.clone(),
                    pagos_conf.api_key.clone().expose().clone(),
                )
                .change_context(error::ConfigurationError::PagosClientSetupError)
                .attach_printable("Failed to initialize Pagos API client during TenantAppState creation")?,
            )
        } else {
            None
        };

        Ok(Self {
            db,
            redis_conn: Arc::new(RedisConnectionWrapper::new(redis_conn)),
            api_client,
            config: tenant_config,
            pagos_client,
        })
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
                                   // Wait for 60 seconds before shutting down
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        handle_clone.shutdown(); // Trigger axum_server shutdown
    });

    let router = axum::Router::new()
        // .layer(middleware::from_fn_with_state(
        //     global_app_state.clone(),
        //     custom_middleware::authenticate,
        // ))
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
            "/merchant-account/create",
            post(routes::merchant_account_config::create_merchant_config),
        )
        .route(
            "/merchant-account/:merchant-id",
            get(routes::merchant_account_config::get_merchant_config),
        )
        .route(
            "/merchant-account/:merchant-id",
            delete(routes::merchant_account_config::delete_merchant_config),
        );
    let router = router.route("/update-score", post(routes::update_score::update_score));
    let router = router.route(
        "/decide-gateway",
        post(routes::decide_gateway::decide_gateway),
    );
    let router = router.route(
        "/update-gateway-score",
        post(routes::update_gateway_score::update_gateway_score),
    );

    let router = router.layer(
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
        .with_state(global_app_state.clone());

    logger::info!(
        category = "SERVER",
        "OpenRouter started [{:?}] [{:?}]",
        global_app_state.global_config.server,
        global_app_state.global_config.log
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
