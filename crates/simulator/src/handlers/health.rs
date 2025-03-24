use axum::Json;
use dynamo::logger;

#[derive(serde::Serialize, Debug)]
pub struct HealthRespPayload {
    pub message: String,
}

/// '/health` API handler`
pub async fn health() -> Json<HealthRespPayload> {
    logger::debug!("Health was called");
    Json(HealthRespPayload {
        message: "Health is good".into(),
    })
}
