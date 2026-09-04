//! Volume-commitment routing: nudge payments toward a behind-pace PSP within an approval
//! tolerance. `controller` writes the plan, `scheduler` owns the clock, `nudge` reads it per payment.

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
pub use volume::{ClickHouseVolumeSource, FixtureVolumeSource, VolumeError, VolumeSource};

/// Per-merchant feature flag; the routing path, the forecaster and the dashboard toggle all read it.
pub const FEATURE_FLAG: &str = "volume_commitment_routing_enabled";

/// The `routing_approach` a diverted decision is stamped with. Analytics needs the value to tell
/// steered volume from unaided, and takes it from here rather than depending on the decider.
pub fn steered_approach() -> String {
    crate::decider::gatewaydecider::types::GatewayDeciderApproach::SrSelectionVolumeCommitment
        .to_string()
}

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

/// Build the shared dependencies at startup, before any server binds.
pub async fn build_deps(
    config: &crate::config::VolumeCommitmentConfig,
    clickhouse: &crate::config::ClickHouseAnalyticsConfig,
) -> Deps {
    // Without ClickHouse nothing can be measured, so no plan is ever built and nothing is steered.
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
        // Redis, always: a process-local plan would be invisible to other replicas and lost on restart.
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
