//! Shared HTTP client builder for the cost pipeline's direct ClickHouse calls.
//!
//! Unlike the analytics read path (which uses the pooled `clickhouse` crate), the cost pipeline
//! talks to ClickHouse over hand-rolled `reqwest` requests: `app → ALB → ClickHouse`. These calls
//! fire infrequently — the serving refresh every 5 min, dashboards on demand — so a pooled keep-alive
//! connection sits idle between uses. An AWS ALB silently closes idle connections at its idle timeout
//! (~60s); a connection left idle past that is reaped, and reqwest then hands the *reaped* socket back
//! from its pool for the next request, which fails at the transport layer with the opaque
//! `error sending request for url (...)` — the symptom that looked like a ClickHouse outage but was a
//! stale connection. (See `scratch/repro-ch-stale-conn` for a local reproduction, and note the analytics
//! `payment-audit` path never hit this because it is used often enough to keep its pool warm.)
//!
//! Every cost ClickHouse client is built here so the pool settings can't drift between call sites:
//! bound the idle-pool lifetime well under the ALB idle timeout and enable TCP keepalive, so a
//! connection reqwest reuses is either fresh or provably alive — never one the ALB already dropped.

use std::time::Duration;

/// Evict idle pooled connections after this long. Must stay comfortably below the ALB idle timeout
/// (~60s) so reqwest opens a fresh connection rather than reusing one the load balancer has reaped.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(15);

/// TCP keepalive so a connection kept between the infrequent cost queries is actively probed rather
/// than silently going stale.
const TCP_KEEPALIVE: Duration = Duration::from_secs(15);

/// Build a cost-pipeline ClickHouse HTTP client. Callers pass their own total-request `timeout`
/// (inserts/fits get a longer budget than reads); the pool/keepalive hardening is shared.
pub fn client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
        .tcp_keepalive(TCP_KEEPALIVE)
        .build()
        .expect("failed to build clickhouse cost http client")
}
