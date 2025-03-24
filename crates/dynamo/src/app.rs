use crate::{
    configs,
    contract_routing::{proto::types as contract_proto_types, types as contract_types},
    elimination::{
        proto::types as elimination_proto_types,
        types::{Elimination, EliminationConfig},
    },
    ephemeral_store::RedisEphemeralStore,
    error::ConfigurationError,
    health_check::{proto::types as health_proto, types as health_types},
    logger, metrics, proto_build,
    success_rate::{
        proto::types as sr_proto_types,
        types::{SuccessRate, SuccessRateConfig},
    },
    utils,
};
use axum::http;
use redis_interface::RedisConnectionPool;
use std::{future::Future, net, sync::Arc};
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::oneshot,
};
use tonic::transport::Server;
use tower_http::trace as tower_trace;

/// # Panics
///
/// Will panic if redis connection establishment fails or signal handling fails
pub async fn server_builder(config: configs::Config) -> Result<(), ConfigurationError> {
    let server_config = config.server.clone();
    let socket_addr = net::SocketAddr::new(server_config.host.parse()?, server_config.port);

    let redis_conn = Arc::new(
        RedisConnectionPool::new(&config.redis)
            .await
            .map_err(ConfigurationError::RedisConnectionError)?,
    );

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

    let service = Service::new(config.clone(), Arc::clone(&redis_conn));

    logger::info!(host = %server_config.host, port = %server_config.port, r#type = ?server_config.type_, "starting dynamo");

    match server_config.type_ {
        configs::ServiceType::Grpc => {
            service
                .await
                .grpc_server(socket_addr, shutdown_signal)
                .await?
        }
        configs::ServiceType::Http => {
            service
                .await
                .http_server(socket_addr, shutdown_signal)
                .await?
        }
    }

    Ok(())
}

pub struct Service {
    success_rate_service: SuccessRate,
    elimination_service: Elimination,
    health_check_service: health_types::HealthCheck,
    contract_routing_service: contract_types::ContractRouting,
}

impl Service {
    /// # Panics
    ///
    /// Will panic either if database password, hash key isn't present in configs or unable to
    /// deserialize any of the above keys
    #[allow(clippy::expect_used)]
    pub async fn new(config: configs::Config, redis_conn: Arc<RedisConnectionPool>) -> Self {
        let secrets = config.secret_config.create_client().await;
        let storage = crate::authentication::sql::SqlStorage::new(
            config.database.clone(),
            secrets
                .get_database_password(&config.database)
                .await
                .expect("Failed to decrypt database password"),
        )
        .await
        .expect("Unable to create SqlStorage");
        let ephemeral_store =
            RedisEphemeralStore::new(Arc::clone(&redis_conn), config.ttl_for_keys);

        Self {
            success_rate_service: SuccessRate::new(
                SuccessRateConfig::new(
                    config.ttl_for_keys,
                    config.multi_tenancy.enabled,
                    config.global_routing_configs.success_rate,
                ),
                Box::new(ephemeral_store),
                secrets
                    .get_hash_key(&config.secrets)
                    .await
                    .expect("Failed to decrypt hashkey"),
                Some(Box::new(
                    crate::authentication::caching::CachingStorage::new(storage, config.cache),
                )),
            )
            .await,
            elimination_service: Elimination::new(
                EliminationConfig::new(
                    config.ttl_for_keys,
                    config.multi_tenancy.enabled,
                    config.global_routing_configs.elimination_rate,
                ),
                Arc::clone(&redis_conn),
            ),
            health_check_service: health_types::HealthCheck,
            contract_routing_service: contract_types::ContractRouting::new(
                contract_types::ContractRoutingConfig {
                    keys_ttl: config.ttl_for_keys,
                    is_multi_tenancy_enabled: config.multi_tenancy.enabled,
                },
                Arc::clone(&redis_conn),
            ),
        }
    }

    pub async fn http_server(
        self,
        socket: net::SocketAddr,
        shutdown_signal: impl Future<Output = ()> + Send + 'static,
    ) -> Result<(), ConfigurationError> {
        let logging_layer = tower_trace::TraceLayer::new_for_http()
            .make_span_with(|request: &axum::extract::Request<_>| {
                utils::record_fields_from_header(request)
            })
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
            );

        let router = axum::Router::new()
            .layer(logging_layer)
            .merge(health_proto::proto_items::health_handler(
                self.health_check_service,
            ))
            .merge(
                sr_proto_types::proto_items::success_rate_calculator_handler(
                    self.success_rate_service,
                ),
            )
            .merge(
                contract_proto_types::proto_items::contract_score_calculator_handler(
                    self.contract_routing_service,
                ),
            )
            .merge(
                elimination_proto_types::proto_items::elimination_analyser_handler(
                    self.elimination_service,
                ),
            );

        let listener = tokio::net::TcpListener::bind(socket).await?;

        axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(shutdown_signal)
            .await?;

        Ok(())
    }

    pub async fn grpc_server(
        self,
        socket: net::SocketAddr,
        shutdown_signal: impl Future<Output = ()>,
    ) -> Result<(), ConfigurationError> {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(proto_build::FILE_DESCRIPTOR_SET)
            .build_v1()?;

        let logging_layer = tower_trace::TraceLayer::new_for_http()
            .make_span_with(|request: &http::request::Request<_>| {
                utils::record_fields_from_header(request)
            })
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
            );

        Server::builder()
            .layer(logging_layer)
            .add_service(reflection_service)
            .add_service(health_proto::HealthServer::new(self.health_check_service))
            .add_service(sr_proto_types::SuccessRateCalculatorServer::new(
                self.success_rate_service,
            ))
            .add_service(contract_proto_types::ContractScoreCalculatorServer::new(
                self.contract_routing_service,
            ))
            .add_service(elimination_proto_types::EliminationAnalyserServer::new(
                self.elimination_service,
            ))
            .serve_with_shutdown(socket, shutdown_signal)
            .await?;

        Ok(())
    }
}

pub async fn metrics_server_builder(config: configs::Config) -> Result<(), ConfigurationError> {
    let listener = config.metrics.tcp_listener().await?;

    let router = axum::Router::new().route(
        "/metrics",
        axum::routing::get(|| async {
            let output = metrics::metrics_handler().await;
            match output {
                Ok(metrics) => Ok(metrics),
                Err(error) => {
                    tracing::error!(?error, "Error fetching metrics");

                    Err((
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Error fetching metrics".to_string(),
                    ))
                }
            }
        }),
    );

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(async {
            let output = tokio::signal::ctrl_c().await;
            tracing::error!("shutting down: {:?}", output);
        })
        .await?;

    Ok(())
}
