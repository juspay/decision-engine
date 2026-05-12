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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GsmScenarioView {
    key: String,
    label: String,
    penalise: bool,
    error_category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<String>,
}

impl From<&crate::config::GsmSimulationScenario> for GsmScenarioView {
    fn from(s: &crate::config::GsmSimulationScenario) -> Self {
        Self {
            key: s.key.clone(),
            label: s.label.clone(),
            penalise: s.penalise,
            error_category: s.error_category.clone(),
            decision: s.decision.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct GsmScenariosResponse {
    scenarios: Vec<GsmScenarioView>,
}

pub async fn gsm_scenarios() -> Json<GsmScenariosResponse> {
    let app_state = crate::app::APP_STATE
        .get()
        .expect("GlobalAppState not initialised");
    Json(GsmScenariosResponse {
        scenarios: app_state
            .global_config
            .gsm_scenarios
            .scenarios
            .iter()
            .map(GsmScenarioView::from)
            .collect(),
    })
}
