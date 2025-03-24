use dynamo::{app, configs, logger};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[allow(clippy::expect_used)]
    let config = configs::Config::new().expect("Failed while parsing config");
    let _guard = logger::setup(
        &config.log,
        dynamo::service_name!(),
        [dynamo::service_name!(), "tower_http"],
    );

    let metrics_server = app::metrics_server_builder(config.clone());
    let server = app::server_builder(config);

    #[allow(clippy::expect_used)]
    tokio::try_join!(metrics_server, server)?;

    Ok(())
}
