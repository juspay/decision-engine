use open_router::{logger, tenant::GlobalAppState};

#[allow(clippy::expect_used)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut global_config =
        open_router::config::GlobalConfig::new().expect("Failed while parsing config");

    let _guard = logger::setup(
        &global_config.log,
        open_router::service_name!(),
        [open_router::service_name!(), "tower_http"],
    );

    #[allow(clippy::expect_used)]
    global_config
        .validate()
        .expect("Failed to validate application configuration");
    global_config
        .fetch_raw_secrets()
        .await
        .expect("Failed to fetch raw application secrets");

    let global_app_state = GlobalAppState::new(global_config.clone()).await;

    // Run all three threads concurrently using tokio::spawn
    let main_server_handle = tokio::spawn(async move {
        open_router::app::server_builder(global_app_state)
            .await
            .expect("Failed while building the main server")
    });

    let metrics_server_handle = tokio::spawn(async move {
        open_router::metrics::metrics_server_builder(global_config.clone())
            .await
            .expect("Failed while building the metrics server")
    });

    let shard_queue_handle = tokio::spawn(async move {
        open_router::shard_queue::GLOBAL_SHARD_QUEUE_HANDLER.spawn()
            .await
            .expect("Failed while running the shard queue handler")
    });

    // Wait for all three threads to complete (they should run indefinitely)
    tokio::try_join!(main_server_handle, metrics_server_handle, shard_queue_handle)?;

    Ok(())
}
