//! The scheduler's own axum listener (like the metrics server): `/health`, `/schedule`, and the loop.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use tokio::signal::unix::{signal, SignalKind};

use super::scheduler::{ScheduleEntry, Scheduler};
use super::Deps;
use crate::logger;
use crate::metrics::ConfigurationError;

/// Serve the scheduler's port, and run its loop, until SIGTERM. A no-op unless
/// `volume_commitment.enabled` — two schedulers would double every run.
pub async fn volume_commitment_server_builder(
    deps: Arc<Deps>,
    admin_secret: String,
) -> Result<(), ConfigurationError> {
    if !deps.config.enabled {
        logger::info!(
            tag = "volume_commitment",
            "volume commitment scheduler disabled; not binding its port"
        );
        return Ok(());
    }

    let bind_address = format!("{}:{}", deps.config.server.host, deps.config.server.port);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    logger::info!(
        tag = "volume_commitment",
        "volume commitment scheduler listening on {}",
        bind_address
    );

    let scheduler = Arc::new(Scheduler::new(deps, admin_secret));
    Arc::clone(&scheduler).spawn();

    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/schedule", get(schedule))
        .with_state(scheduler);

    let mut sigterm = signal(SignalKind::terminate())?;

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(async move {
            let _ = sigterm.recv().await;
            logger::info!(
                tag = "volume_commitment",
                "volume commitment scheduler shutting down gracefully"
            );
        })
        .await?;

    Ok(())
}

/// `GET /schedule` — every merchant's cadence, last run, and time until the next one fires.
async fn schedule(State(scheduler): State<Arc<Scheduler>>) -> Json<Vec<ScheduleEntry>> {
    Json(scheduler.schedule().await)
}
