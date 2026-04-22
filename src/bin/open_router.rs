#![allow(clippy::unwrap_in_result)]

use open_router::{logger, tenant::GlobalAppState};

#[allow(clippy::expect_used)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut global_config =
        open_router::config::GlobalConfig::new().expect("Failed while parsing config");

    let _guard = logger::setup(
        &global_config.log,
        open_router::service_name!(),
        ["tower_http"],
    );

    #[allow(clippy::expect_used)]
    global_config
        .validate()
        .expect("Failed to validate application configuration");
    global_config
        .fetch_raw_secrets()
        .await
        .expect("Failed to fetch raw application secrets");

    log_startup_configuration(&global_config);

    let global_app_state = GlobalAppState::new(global_config.clone())
        .await
        .expect("Failed while configuring global application state");

    // Run both servers concurrently using tokio::spawn
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

    // Wait for both servers to complete (they should run indefinitely)
    tokio::try_join!(main_server_handle, metrics_server_handle)?;

    Ok(())
}

fn log_startup_configuration(global_config: &open_router::config::GlobalConfig) {
    logger::info!("Decision engine started [{:?}]", global_config);

    if global_config.admin_secret.is_default() {
        logger::warn!(
            "SECURITY WARNING: admin_secret is set to the default value. \
             Set `admin_secret.secret` in your config to a strong secret before exposing this server."
        );
    }
    if global_config.user_auth.jwt_secret == "change_me_in_production_use_32chars!!" {
        logger::warn!(
            "SECURITY WARNING: user_auth.jwt_secret is set to the default value. \
             Set it to a strong random secret in production."
        );
    }
}
