//! Volume commitment routing: merchants promise PSPs volume for rebates, and normal routing may
//! not deliver it. Routing stays in charge; payments are only nudged toward a behind-pace PSP
//! where it approves almost as well, so approvals barely move.
//!
//! `forecast_interval_secs` is the only cadence: each run re-measures delivery and republishes the
//! share of eligible payments each behind-pace PSP should take.
//!
//! Three parts: `controller` writes the plan (never on a timer), `server`+`scheduler` own the
//! clock on their own port, `nudge` reads the plan per payment. No plan means normal routing.

pub mod controller;
pub mod dsl;
pub mod inputs;
pub mod math;
pub mod nudge;
pub mod plan;
pub mod scheduler;
pub mod server;
pub mod state;
pub mod volume;

use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::OnceCell;

pub use dsl::DslInputSource;
pub use inputs::{Commitment, CommitmentInputs, InputSource, MeasuredVolume};
pub use nudge::{NudgeOutcome, VolumeSteerInfo, VolumeSteerOutcome};
pub use plan::{DroppedPsp, PspPlan, SteeringPlan};
pub use state::{RedisStateStore, StateStore};
pub use volume::{ClickHouseVolumeSource, FixtureVolumeSource, VolumeSource};

/// What this feature needs from outside: the promise, the delivery, and somewhere to keep state.
pub struct Deps {
    /// Deployment settings — cadence defaults, where the main server is.
    pub config: crate::config::VolumeCommitmentConfig,
    /// What the merchant committed to. From the contract DSL.
    pub inputs: Arc<dyn InputSource>,
    /// Where the plan and the steered-today counters live.
    pub state: Arc<dyn StateStore>,
    /// What each PSP was actually sent. From the traffic.
    pub volume: Arc<dyn VolumeSource>,
}

/// Build the shared dependencies at startup, before any server binds. Commitments come from the
/// merchant's active volume-contract document; a merchant without one is simply not steered.
pub async fn build_deps(
    config: &crate::config::VolumeCommitmentConfig,
    clickhouse: &crate::config::ClickHouseAnalyticsConfig,
) -> Deps {

    // Real routed traffic when analytics is on. Without ClickHouse there is nowhere to read
    // delivered volume from, so every PSP measures zero and nothing is ever judged behind.
    let volume: Arc<dyn VolumeSource> = if clickhouse.enabled {
        crate::logger::info!(
            tag = "volume_commitment",
            "measuring delivered volume from clickhouse"
        );
        Arc::new(ClickHouseVolumeSource::new(clickhouse))
    } else {
        crate::logger::warn!(
            tag = "volume_commitment",
            "clickhouse analytics is disabled; no delivered volume can be measured, so no \
             commitment will be paced"
        );
        Arc::new(FixtureVolumeSource::new(HashMap::new()))
    };

    Deps {
        config: config.clone(),
        inputs: Arc::new(DslInputSource),
        // Redis, always. Redis is already a hard dependency of the process, and a plan held only
        // in local memory is invisible to every other replica and gone on restart — there is no
        // deployment, single-process included, where that is the better answer.
        state: Arc::new(RedisStateStore),
        volume,
    }
}

/// Set once at startup, so the routing path can reach these without being passed them.
static DEPS: OnceCell<Arc<Deps>> = OnceCell::new();

/// Store the dependencies. Calling it twice does nothing.
pub fn init_deps(deps: Arc<Deps>) {
    let _ = DEPS.set(deps);
}

/// The shared dependencies, or None before startup has run — routing treats that as "feature off".
pub fn deps() -> Option<&'static Arc<Deps>> {
    DEPS.get()
}
