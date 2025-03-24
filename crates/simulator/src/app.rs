use std::sync::Arc;

use crate::{
    configs::{self, Config},
    errors,
    file_storage::FileStorageInterface,
    handlers::{self, simulate::types::AlgorithmType},
    utils,
};

use axum::extract::{DefaultBodyLimit, Request};
use redis_interface::RedisConnectionPool;
use tower_http::trace as tower_trace;

use dynamo::{
    authentication,
    configs::GlobalSrConfig,
    ephemeral_store::InMemoryEphemeralStore,
    logger,
    success_rate::{
        error::SuccessRateError,
        types::{SuccessRate, SuccessRateConfig},
    },
    DynamicRouting,
};
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::oneshot,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub redis_conn: Arc<RedisConnectionPool>,
    pub file_storage_client: Arc<dyn FileStorageInterface>,
    pub sr_algorithm: Algorithms,
    pub jwt_secret: masking::Secret<String>,
}

#[derive(Clone)]
pub struct Algorithms {
    pub window_based: SuccessRate,
}

type DynamicRoutingBox = Box<
    dyn DynamicRouting<
            UpdateWindowReport = Vec<(String, bool)>,
            RoutingResponse = Vec<(f64, String)>,
            Error = SuccessRateError,
        > + Send
        + Sync,
>;

impl Algorithms {
    fn new(window_based: SuccessRate) -> Self {
        Self { window_based }
    }

    pub fn get_algo(&self, algo_type: &AlgorithmType) -> DynamicRoutingBox {
        match algo_type {
            AlgorithmType::WindowBased => Box::new(self.window_based.clone()),
        }
    }
}

impl AppState {
    async fn new(config: Config) -> error_stack::Result<Self, errors::ConfigurationError> {
        let secrets = config.secret_config.create_client().await;
        let storage = authentication::sql::SqlStorage::new(
            config.database.clone(),
            secrets
                .get_database_password(&config.database)
                .await
                .expect("Failed to decrypt database password"),
        )
        .await
        .expect("Unable to create SqlStorage");

        let hash_key = secrets
            .get_hash_key(&config.secrets)
            .await
            .expect("Failed to decrypt hashkey");

        let jwt_secret = secrets
            .get_jwt_secret(&config.secrets)
            .await
            .expect("Failed to decrypt jwt_secret");

        let file_storage_client = config.file_storage.get_file_storage_client().await;

        let redis_conn = Arc::new(
            RedisConnectionPool::new(&config.redis)
                .await
                .map_err(errors::ConfigurationError::RedisConnectionError)?,
        );

        let ephemeral_store = InMemoryEphemeralStore::default();
        let cache_config = config.cache.clone();

        let sr_algorithm = Algorithms::new(
            SuccessRate::new(
                SuccessRateConfig::new(
                    config.ttl_for_keys,
                    config.multi_tenancy.enabled,
                    GlobalSrConfig::default(),
                ),
                Box::new(ephemeral_store.clone()),
                hash_key.clone(),
                Some(Box::new(authentication::caching::CachingStorage::new(
                    storage,
                    cache_config.clone(),
                ))),
            )
            .await,
        );

        Ok(Self {
            config,
            redis_conn,
            file_storage_client,
            sr_algorithm,
            jwt_secret,
        })
    }
}

pub async fn server_builder(
    config: configs::Config,
) -> error_stack::Result<(), errors::ConfigurationError> {
    let listener = config.server.tcp_listener().await?;

    // Signal handler
    let (tx, rx) = oneshot::channel();

    #[allow(clippy::expect_used)]
    tokio::spawn(async move {
        let mut sig_int =
            signal(SignalKind::interrupt()).expect("Failed to initialize SIGINT signal handler");
        let mut sig_term =
            signal(SignalKind::terminate()).expect("Failed to initialize SIGTERM signal handler");
        let mut sig_quit =
            signal(SignalKind::quit()).expect("Failed to initialize QUIT signal handler");
        let mut sig_hup =
            signal(SignalKind::hangup()).expect("Failed to initialize SIGHUP signal handler");

        tokio::select! {
            _ = sig_int.recv() => {
                logger::info!("Received SIGINT");
                tx.send(()).expect("Failed to send SIGINT signal");
            }
            _ = sig_term.recv() => {
                logger::info!("Received SIGTERM");
                tx.send(()).expect("Failed to send SIGTERM signal");
            }
            _ = sig_quit.recv() => {
                logger::info!("Received QUIT");
                tx.send(()).expect("Failed to send QUIT signal");
            }
            _ = sig_hup.recv() => {
                logger::info!("Received SIGHUP");
                tx.send(()).expect("Failed to send SIGHUP signal");
            }
        }
    });

    #[allow(clippy::expect_used)]
    let shutdown_signal = async {
        rx.await.expect("Failed to receive shutdown signal");
        logger::info!("Shutdown signal received");
    };

    let app_state = AppState::new(config).await?;

    let router = axum::Router::new()
        .route(
            "/simulate/{merchant_id}",
            axum::routing::post(handlers::simulate::simulate),
        )
        .route(
            "/simulate/{merchant-id}/get-statistics",
            axum::routing::get(handlers::simulate::fetch_simulated_summary),
        )
        .route(
            "/simulate/{merchant-id}/get-records",
            axum::routing::get(handlers::simulate::fetch_simulated_report),
        )
        .layer(DefaultBodyLimit::max(
            app_state.config.parameters.max_default_body_limit_in_bytes,
        ))
        .with_state(Arc::new(app_state));

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

    let router = router.route("/health", axum::routing::get(handlers::health::health));

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal)
        .await
        .map_err(|err| errors::ConfigurationError::ServerError {
            msg: err.to_string(),
        })?;

    Ok(())
}
