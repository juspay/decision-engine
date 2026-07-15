//! Shared HTTP client builder for the cost pipeline's direct ClickHouse calls.
//!
//! Unlike the analytics read path (which uses the pooled `clickhouse` crate over hyper), the cost
//! pipeline talks to ClickHouse over hand-rolled `reqwest` requests: `app → internal ALB → ClickHouse`.
//! Every cost ClickHouse client is built here so this configuration can't drift between call sites.
//!
//! ## `no_proxy` — why the cost path was failing with `error sending request`
//! The sandbox runs in a private subnet with a Squid egress proxy, so the pod sets `HTTP_PROXY`/
//! `http_proxy`. `reqwest` honors those env vars by DEFAULT, so without `.no_proxy()` the cost client
//! sent requests for the *internal* ClickHouse ALB *through the external egress proxy*, which cannot
//! route to an internal ELB — the request hung until the request timeout and surfaced as the opaque
//! `error sending request for url (...)`. The analytics `clickhouse` crate never hit this because it
//! uses hyper directly and ignores the proxy env vars, connecting straight to ClickHouse (which is
//! why analytics reads succeeded against the same URL while every cost call failed, even on a fresh
//! connection). ClickHouse is an internal endpoint that must always be reached directly, so we force
//! `.no_proxy()` here.
//!
//! ## pool_idle_timeout / tcp_keepalive — defense-in-depth for LB-reaped connections
//! These calls fire infrequently (serving refresh every 5 min, dashboards on demand), so a pooled
//! keep-alive connection can sit idle long enough for the ALB to silently reap it (half-open, no
//! FIN/RST). reqwest cannot detect that and would reuse the dead socket → the same transport error.
//! We bound the idle-pool lifetime below the ALB idle timeout and enable TCP keepalive so a reused
//! connection is either fresh or provably alive. (See `scratch/repro-ch-stale-conn` for a local
//! reproduction of that failure mode.)

use std::time::Duration;

/// Evict idle pooled connections after this long. Must stay comfortably below the ALB idle timeout
/// (~60s) so reqwest opens a fresh connection rather than reusing one the load balancer has reaped.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(15);

/// TCP keepalive so a connection kept between the infrequent cost queries is actively probed rather
/// than silently going stale.
const TCP_KEEPALIVE: Duration = Duration::from_secs(15);

/// Build a cost-pipeline ClickHouse HTTP client. Callers pass their own total-request `timeout`
/// (inserts/fits get a longer budget than reads); the proxy/pool/keepalive hardening is shared.
pub fn client(timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(timeout)
        // ClickHouse is an internal endpoint: never route it through the egress proxy that reqwest
        // would otherwise pick up from HTTP_PROXY/http_proxy. This is the fix for the cost path
        // hanging while the (proxy-ignoring) analytics `clickhouse` crate succeeded on the same URL.
        .no_proxy()
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
        .tcp_keepalive(TCP_KEEPALIVE)
        .build()
        .expect("failed to build clickhouse cost http client")
}
