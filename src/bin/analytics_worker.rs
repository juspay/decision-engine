#![allow(clippy::unwrap_in_result)]

use open_router::{analytics::AnalyticsWorker, logger};

#[allow(clippy::expect_used)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut global_config =
        open_router::config::GlobalConfig::new().expect("Failed while parsing config");

    let _guard = logger::setup(
        &global_config.log,
        open_router::service_name!(),
        ["rdkafka", "tower_http"],
    );

    global_config
        .validate()
        .expect("Failed to validate application configuration");
    global_config
        .fetch_raw_secrets()
        .await
        .expect("Failed to fetch raw application secrets");

    let worker = AnalyticsWorker::new(global_config.analytics.clone())
        .await
        .expect("Failed to initialize analytics worker");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("Failed to create SIGTERM handler");

    let worker_handle = tokio::spawn(async move {
        worker
            .run_until_shutdown(async move {
                let _ = sigterm.recv().await;
                open_router::logger::info!("SIGTERM signal received, stopping analytics worker");
            })
            .await
            .expect("Analytics worker exited unexpectedly")
    });

    let metrics_server_handle = tokio::spawn(async move {
        open_router::metrics::metrics_server_builder(global_config.clone())
            .await
            .expect("Failed while building the metrics server")
    });

    tokio::try_join!(worker_handle, metrics_server_handle)?;

    Ok(())
}
