use std::sync::Arc;

use crate::app::APP_STATE;
use crate::tenant::GlobalAppState;

use axum::{routing::get, Json};
use hyper::StatusCode;

use crate::storage::TestInterface;
use crate::{custom_extractors::TenantStateResolver, error};

///
/// Function for registering routes that is specifically handling the health apis
///
pub fn serve() -> axum::Router<Arc<GlobalAppState>> {
    axum::Router::new()
        .route("/", get(health))
        .route("/diagnostics", get(diagnostics))
        .route("/ready", get(ready))
}

#[derive(serde::Serialize, Debug)]
pub struct HealthRespPayload {
    pub message: String,
}

/// '/health/ready` API handler`
pub async fn ready() -> (StatusCode, Json<HealthRespPayload>) {
    let app_state = APP_STATE.get().expect("GlobalAppState not set");
    let app_ready = app_state.is_ready();
    crate::logger::debug!("Readiness check was called");

    if app_ready {
        (
            StatusCode::OK,
            Json(HealthRespPayload {
                message: "Up".into(),
            }),
        )
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(HealthRespPayload {
                message: "Down".into(),
            }),
        )
    }
}

/// '/health` API handler`
pub async fn health() -> Json<HealthRespPayload> {
    crate::logger::debug!("Health was called");
    Json(HealthRespPayload {
        message: "Health is good".into(),
    })
}

#[derive(Debug, serde::Serialize, Default)]
pub struct Diagnostics {
    key_custodian_locked: bool,
    database: DatabaseHealth,
}

#[derive(Debug, serde::Serialize, Default)]
pub struct DatabaseHealth {
    database_connection: HealthState,
    database_read: HealthState,
    database_write: HealthState,
    database_delete: HealthState,
}

#[derive(Debug, serde::Serialize, Default)]
pub enum HealthState {
    Working,
    #[default]
    Failing,
}

/// '/health/diagnostics` API handler`
pub async fn diagnostics(TenantStateResolver(state): TenantStateResolver) -> Json<Diagnostics> {
    crate::logger::info!("Health diagnostics was called");

    let db_test_output = state.db.test().await;
    let db_test_output_case_match = db_test_output.as_ref().map_err(|err| err.get_inner());

    let db_health = match db_test_output_case_match {
        Ok(()) => DatabaseHealth {
            database_connection: HealthState::Working,
            database_read: HealthState::Working,
            database_write: HealthState::Working,
            database_delete: HealthState::Working,
        },

        Err(&error::TestDBError::DBReadError) => DatabaseHealth {
            database_connection: HealthState::Working,
            ..Default::default()
        },

        Err(&error::TestDBError::DBWriteError) => DatabaseHealth {
            database_connection: HealthState::Working,
            database_read: HealthState::Working,
            ..Default::default()
        },

        Err(&error::TestDBError::DBDeleteError) => DatabaseHealth {
            database_connection: HealthState::Working,
            database_write: HealthState::Working,
            database_read: HealthState::Working,
            ..Default::default()
        },

        Err(_) => DatabaseHealth {
            ..Default::default()
        },
    };

    axum::Json(Diagnostics {
        key_custodian_locked: false,
        database: db_health,
    })
}
