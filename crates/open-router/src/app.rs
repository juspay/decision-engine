use axum::{extract::Request, middleware, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use error_stack::ResultExt;
use tower_http::trace as tower_trace;
use redis_interface::RedisConnectionPool;
use crate::redis::commands::RedisConnectionWrapper;

use crate::middleware as custom_middleware;

use std::sync::Arc;

use crate::{
    api_client::ApiClient,
    config::{self, GlobalConfig, TenantConfig},
    error, logger,
    routes,
    storage,
    tenant::GlobalAppState,
    utils,
};

use once_cell::sync::OnceCell;

pub static APP_STATE: OnceCell<Arc<GlobalAppState>> = OnceCell::new();


pub async fn get_tenant_app_state() -> Arc<TenantAppState> {
    let app_state = APP_STATE.get().expect("GlobalAppState not set");
    let tenant_app_state = GlobalAppState::get_app_state_of_tenant(app_state, "public").await.unwrap();
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

        Ok(Self {
            db,
            redis_conn: Arc::new(RedisConnectionWrapper::new(redis_conn)),
            api_client,
            config: tenant_config,
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


    let router = axum::Router::new()
        .layer(middleware::from_fn_with_state(
            global_app_state.clone(),
            custom_middleware::authenticate,
        ))
        .route(
            "/decision_gateway",
            post(routes::decision_gateway::decision_gateway)
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
        // .nest("/health", routes::health::serve())
        .with_state(global_app_state.clone());

    logger::info!(
        "OpenRouter started [{:?}] [{:?}]",
        global_app_state.global_config.server,
        global_app_state.global_config.log
    );

    logger::debug!(startup_config=?global_app_state.global_config);

    if let Some(tls_config) = &global_app_state.global_config.tls {
        let tcp_listener = std::net::TcpListener::bind(socket_addr)?;
        let rusttls_config =
            RustlsConfig::from_pem_file(&tls_config.certificate, &tls_config.private_key).await?;

        axum_server::from_tcp_rustls(tcp_listener, rusttls_config)
            .serve(router.into_make_service())
            .await?;
    } else {
        let tcp_listener = tokio::net::TcpListener::bind(socket_addr).await?;

        axum::serve(tcp_listener, router.into_make_service()).await?;
    }

    Ok(())
}
