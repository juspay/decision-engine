//! Everything the volume-commitment dashboard reads from analytics, gathered in one pass.
//!
//! The three metrics are independent queries, so they run concurrently: the caller waits for the
//! slowest rather than the sum, which is what makes a single composed request cheaper than the
//! separate polls it replaces.

use crate::analytics::models::{
    CommitmentAnalytics, CommitmentAnalyticsQuery, CommitmentImpactData,
};

use super::super::metrics::{commitment_audit, commitment_impact, commitment_series};

/// The pacing dashboard: where each PSP stands through its cycle, and what the controller did.
pub async fn load(
    client: &clickhouse::Client,
    query: &CommitmentAnalyticsQuery,
) -> CommitmentAnalytics {
    if query.windows.is_empty() {
        return CommitmentAnalytics::default();
    }

    let (series, audit) = futures::join!(
        commitment_series::load(client, query),
        commitment_audit::load(client, query),
    );

    CommitmentAnalytics { series, audit }
}

/// The impact view: each PSP's own cycle against the period immediately before it. Four
/// independent queries, run together.
pub async fn load_impact(
    client: &clickhouse::Client,
    query: &CommitmentAnalyticsQuery,
) -> CommitmentImpactData {
    if query.windows.is_empty() {
        return CommitmentImpactData::default();
    }

    // Every window steps back by its own length, so a document whose commitments open on different
    // days compares each PSP against its own history rather than against one shared span that
    // overlaps somebody's current cycle. Both halves are read a bucket per contract day, and both
    // number those buckets from a cycle start, so they line up per PSP bucket for bucket.
    let previous = query.previous_cycle();
    let (previous_daily, cycle_daily) = (previous.at_resolution(1), query.at_resolution(1));
    let (before, during, baseline_days, cycle_days) = futures::join!(
        commitment_impact::load(client, &previous),
        commitment_impact::load(client, query),
        commitment_series::load(client, &previous_daily),
        commitment_series::load(client, &cycle_daily),
    );

    CommitmentImpactData {
        before,
        during,
        baseline_days,
        cycle_days,
    }
}
