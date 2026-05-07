use axum::Json;
use serde::Serialize;

use crate::gsm::{options, GsmOptionRow};

#[derive(Serialize)]
pub struct GsmOptionsResponse {
    pub rules: Vec<GsmOptionRow>,
}

pub async fn gsm_options() -> Json<GsmOptionsResponse> {
    Json(GsmOptionsResponse { rules: options() })
}
