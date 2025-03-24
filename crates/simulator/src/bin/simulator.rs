use dynamo::logger;
use simulator::{app, configs};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[allow(clippy::expect_used)]
    let config = configs::Config::new().expect("Failed while parsing config");
    let _guard = logger::setup(
        &config.log,
        dynamo::service_name!(),
        [dynamo::service_name!(), "tower_http"],
    );
    app::server_builder(config).await?;

    Ok(())
}
