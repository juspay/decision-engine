//! Merchant-facing API for the highest-traffic fitted clusters and their per-cluster fee overrides.
//!
//! Powers both the "ingested data" view (what each of the merchant's biggest segments costs) and the
//! surgical per-cluster override (fix the fee on the top segments). A cluster override wins over a
//! connector override and the learned model at decide time — see
//! [`crate::cost_ingestion::serving::lookup`].

use std::collections::HashMap;

use axum::extract::{Path, Query, RawQuery};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Iso8601;

use crate::cost_ingestion::blended::{
    self, ClusterFilter, ClusterOrder, ClusterScope, ClusterSegment, TopCluster,
};
use crate::cost_ingestion::overrides::{self, ClusterDims, ClusterOverride};
use crate::routes::connector_fees::{clickhouse_config, refresh_serving};

/// How many top clusters to surface by default (top by GMV) when the caller asks for none.
const DEFAULT_LIMIT: u32 = 10;
/// Hard ceiling on one response. Raised well past the old 50 because the cluster list is long-tailed
/// — a single-account report fits ~1.6k clusters, and ranking by GMV put the ~1.4k low-traffic ones
/// permanently out of reach even though a THIN cluster is exactly what a merchant wants to override.
/// Combined with the per-column filters (see [`ClusterFilter`]) every cluster is now addressable.
const MAX_LIMIT: u32 = 5000;

/// The `connector|network|variant|funding|issuer|currency|ic_category` key used on the wire and to
/// key the stored override. Built lowercase so it round-trips through [`ClusterDims::from_key`] and
/// matches the serving `fine_key`.
#[allow(clippy::too_many_arguments)]
fn cluster_key(
    connector: &str,
    network: &str,
    variant: &str,
    funding: &str,
    issuer: &str,
    currency: &str,
    ic_category: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        connector.to_lowercase(),
        network.to_lowercase(),
        variant.to_lowercase(),
        funding.to_lowercase(),
        issuer.to_lowercase(),
        currency.to_lowercase(),
        ic_category.to_lowercase(),
    )
}

/// A fitted number only if it's actually a number. The regression yields NaN for a cluster with no
/// amount spread (e.g. a single transaction), and NaN would serialize as a bare JSON `null` from a
/// non-Option field — this makes the "no rate" case explicit instead of accidental.
fn finite(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

fn key_of_dims(d: &ClusterDims) -> String {
    cluster_key(
        &d.connector,
        &d.card_network,
        &d.variant,
        &d.funding,
        &d.issuer_country,
        &d.currency,
        &d.ic_category,
    )
}

/// One recovered piece of a capped/tiered cluster on the wire (from `cost_fee_model_segment`).
#[derive(Debug, Serialize)]
pub struct ApiClusterSegment {
    pub seg_idx: u8,
    /// Amount range this piece prices: `[lo, hi)`.
    pub lo: f64,
    pub hi: f64,
    pub pct_bps: Option<f64>,
    pub fixed: Option<f64>,
    pub bps_rmse: Option<f64>,
    pub n: u64,
    pub gross_sum: f64,
    /// `"GOOD"` | `"THIN"` | `"NON_LINEAR"`.
    pub verdict: String,
}

impl From<ClusterSegment> for ApiClusterSegment {
    fn from(s: ClusterSegment) -> Self {
        Self {
            seg_idx: s.seg_idx,
            lo: s.lo,
            hi: s.hi,
            pct_bps: s.pct_bps,
            fixed: s.fixed,
            bps_rmse: s.bps_rmse,
            n: s.n,
            gross_sum: s.gross_sum,
            verdict: s.verdict,
        }
    }
}

/// One cluster's fee picture for the dashboard.
#[derive(Debug, Serialize)]
pub struct ClusterFee {
    /// Opaque key identifying the cluster (used in the override PUT/DELETE path).
    pub key: String,
    pub connector: String,
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub issuer_country: String,
    pub currency: String,
    pub ic_category: String,
    /// Transaction count and settled GMV for the cluster (0 for an override-only cluster no longer
    /// in the top set).
    pub n: u64,
    pub gross_sum: f64,
    /// Learned fee (present when the cluster is in the fitted snapshot).
    pub model_pct_bps: Option<f64>,
    pub model_fixed: Option<f64>,
    /// Manual override, when set.
    pub override_pct_bps: Option<f64>,
    pub override_fixed: Option<f64>,
    pub override_updated_at: Option<String>,
    /// Fee actually used at decide time and where it comes from. `None` when the cluster has no
    /// usable rate at all — a single-transaction THIN cluster fits to NaN (no amount spread to
    /// regress on). Such a cluster is still listed, precisely so it can be given a contract rate.
    pub effective_pct_bps: Option<f64>,
    pub effective_fixed: Option<f64>,
    /// `"override"` | `"model"` | `"segmented"`.
    pub source: String,
    /// Fit grade of the learned model: `"GOOD"` | `"THIN"` | `"NON_LINEAR"`, or `""` for a cluster
    /// with no fitted row (an override-only cluster). A non-GOOD rate is shown so it can be
    /// corrected — it is not what the router trusts.
    pub verdict: String,
    /// Card-program proxy: the issuer BIN's dominant interchange rate, integer bps as a string
    /// (`"115"`), or `""` when the report carried no PAN. Within one network+funding+country the
    /// rate tier is what separates Classic from Platinum from commercial.
    pub card_product: String,
    /// Recovered per-segment rates for a capped/tiered cluster. Empty for an ordinary (GOOD) cluster
    /// priced by a single line. Display-only in Phase 1 — the decide path is not yet segment-aware.
    #[serde(default)]
    pub segments: Vec<ApiClusterSegment>,
}

#[derive(Debug, Deserialize)]
pub struct TopClustersQuery {
    pub limit: Option<u32>,
    /// Scope to one ingested snapshot (all three required together): the fitted segments of that
    /// specific report. Omit for the merchant-wide latest-snapshot view (the override targets).
    pub connector: Option<String>,
    pub account: Option<String>,
    pub report_date: Option<String>,
    /// Narrow to one ingestion's clusters. Required for a per-upload view: two uploads on the same
    /// day under the same account share a snapshot, so `(connector, account, report_date)` alone
    /// returns both reports' clusters.
    pub ingestion_id: Option<String>,
    /// Ranking for the top-N selection: `"txns"` ranks by transaction count; anything else (default)
    /// ranks by settled GMV.
    pub order: Option<String>,
}

/// Multi-valued column filters, parsed from REPEATED query keys
/// (`?card_network=visa&card_network=mc`). `serde_urlencoded` — what `Query<T>` uses — keeps only the
/// last occurrence of a key, so these cannot ride on [`TopClustersQuery`]; they are parsed straight
/// off the raw query string instead. Repeated keys are used rather than a delimited value because
/// real `ic_category` values contain commas.
#[derive(Debug, Default)]
pub struct ClusterFilterQuery {
    card_network: Vec<String>,
    variant: Vec<String>,
    funding: Vec<String>,
    issuer_country: Vec<String>,
    currency: Vec<String>,
    ic_category: Vec<String>,
    verdict: Vec<String>,
    q: Option<String>,
}

impl ClusterFilterQuery {
    /// Collect the filter keys out of a raw query string. Blank values are dropped: a browser submits
    /// an empty input as `?currency=`, and treating that as "match the empty string" would silently
    /// return nothing.
    pub fn from_raw(raw: Option<&str>) -> Self {
        let mut out = Self::default();
        let Some(raw) = raw else { return out };
        for (k, v) in form_urlencoded::parse(raw.as_bytes()) {
            let v = v.trim();
            if v.is_empty() {
                continue;
            }
            match k.as_ref() {
                "card_network" => out.card_network.push(v.to_string()),
                "variant" => out.variant.push(v.to_string()),
                "funding" => out.funding.push(v.to_string()),
                "issuer_country" => out.issuer_country.push(v.to_string()),
                "currency" => out.currency.push(v.to_string()),
                "ic_category" => out.ic_category.push(v.to_string()),
                "verdict" => out.verdict.push(v.to_string()),
                "q" => out.q = Some(v.to_string()),
                _ => {}
            }
        }
        out
    }

    fn filter(&self) -> ClusterFilter<'_> {
        ClusterFilter {
            card_network: &self.card_network,
            variant: &self.variant,
            funding: &self.funding,
            issuer_country: &self.issuer_country,
            currency: &self.currency,
            ic_category: &self.ic_category,
            verdict: &self.verdict,
            q: self.q.as_deref(),
        }
    }
}

/// One autosuggestion: a value that exists in some filterable column, and the traffic behind it.
#[derive(Debug, Serialize)]
pub struct ApiClusterFacet {
    pub dim: String,
    pub value: String,
    pub txns: u64,
}

/// `GET /merchant-account/:merchant_id/cost-cluster-facets[?connector&account&report_date]` — the
/// distinct value of every filterable column, highest-traffic first, for the filter bar's
/// autosuggestions. Scoped exactly like the cluster list, so a snapshot view only suggests values
/// that occur in that snapshot.
pub async fn list_cost_cluster_facets(
    Path(merchant_id): Path<String>,
    Query(q): Query<TopClustersQuery>,
) -> Result<Json<Vec<ApiClusterFacet>>, (StatusCode, String)> {
    let cfg = clickhouse_config()?;
    let scope = ClusterScope {
        connector: q.connector.as_deref(),
        account: q.account.as_deref(),
        report_date: q.report_date.as_deref(),
        ingestion_id: q.ingestion_id.as_deref(),
    };
    let facets = blended::cluster_facets(&cfg, &merchant_id, scope)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    Ok(Json(
        facets
            .into_iter()
            .map(|f| ApiClusterFacet {
                dim: f.dim,
                value: f.value,
                txns: f.txns,
            })
            .collect(),
    ))
}

/// `GET /merchant-account/:merchant_id/cost-clusters?limit=N[&connector&account&report_date]` — top
/// segments by GMV. Narrowed by any of `connector` / `account` / `report_date`: a connector (+account)
/// gives that connector's latest-snapshot segments (the override targets under a connector); adding
/// `report_date` pins one exact ingestion's segments. Unscoped is merchant-wide. Overrides are merged
/// in; in the unscoped view an overridden segment stays visible even if it drops out of the top set.
pub async fn list_cost_clusters(
    Path(merchant_id): Path<String>,
    Query(q): Query<TopClustersQuery>,
    RawQuery(raw): RawQuery,
) -> Result<Json<Vec<ClusterFee>>, (StatusCode, String)> {
    let filters = ClusterFilterQuery::from_raw(raw.as_deref());
    let cfg = clickhouse_config()?;
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let scope = ClusterScope {
        connector: q.connector.as_deref(),
        account: q.account.as_deref(),
        report_date: q.report_date.as_deref(),
        ingestion_id: q.ingestion_id.as_deref(),
    };
    let order = match q.order.as_deref() {
        Some("txns") => ClusterOrder::Txns,
        _ => ClusterOrder::Gross,
    };
    // "Scoped" = narrowed to a connector/account/snapshot; only then do we suppress the
    // append-overrides-outside-the-top-set behavior (that's a merchant-wide affordance).
    let scoped = q.connector.is_some() || q.account.is_some() || q.report_date.is_some();

    // Surface a ClickHouse failure as a 500 with its message — a swallowed error here is
    // indistinguishable from "no segments" and hides real query bugs.
    let filter = filters.filter();
    let top: Vec<TopCluster> =
        blended::top_clusters(&cfg, &merchant_id, limit, scope, order, filter)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    let overrides: HashMap<String, ClusterOverride> = overrides::list_clusters(&merchant_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|o| (key_of_dims(&o.dims), o))
        .collect();

    // Key → index into `out`, so the segment pass below can enrich a row the fit pass already
    // emitted instead of dropping it (both passes can now return the same non-GOOD cluster).
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<ClusterFee> = Vec::new();

    for c in top {
        let key = cluster_key(
            &c.connector,
            &c.card_network,
            &c.variant,
            &c.funding,
            &c.issuer_country,
            &c.currency,
            &c.ic_category,
        );
        seen.insert(key.clone(), out.len());
        let ov = overrides.get(&key);
        let (effective_pct_bps, effective_fixed, source) = match ov {
            Some(o) => (Some(o.pct_bps), Some(o.fixed), "override"),
            None => (finite(c.pct_bps), finite(c.fixed), "model"),
        };
        out.push(ClusterFee {
            key,
            connector: c.connector,
            card_network: c.card_network,
            variant: c.variant,
            funding: c.funding,
            issuer_country: c.issuer_country,
            currency: c.currency,
            ic_category: c.ic_category,
            n: c.n,
            gross_sum: c.gross_sum,
            model_pct_bps: finite(c.pct_bps),
            model_fixed: finite(c.fixed),
            override_pct_bps: ov.map(|o| o.pct_bps),
            override_fixed: ov.map(|o| o.fixed),
            override_updated_at: ov.map(|o| o.updated_at.clone()),
            effective_pct_bps,
            effective_fixed,
            source: source.to_string(),
            verdict: c.verdict,
            card_product: c.card_product,
            segments: Vec::new(),
        });
    }

    // Segmented (capped/tiered) clusters and their recovered per-segment rates. `top_clusters` now
    // returns non-GOOD clusters too, so most of these already have a row above — that row gets the
    // segments attached. One outside the top-N limit is appended fresh.
    // Where a cluster is segmented, `effective_*` is the volume-weighted blend of the pieces (what
    // the segments average to) rather than the whole-cluster line, which by definition didn't fit.
    // Display-only in Phase 1: the decide path is not yet segment-aware.
    // Best-effort, unlike the top_clusters fetch above: the segment table may not exist yet
    // (pre-migration) or a merchant may simply have none. A failure here must NOT break the core
    // cluster list — degrade to "no segments" and log, so the fitted clusters still render.
    let segmented = match blended::segmented_clusters(&cfg, &merchant_id, scope).await {
        Ok(s) => s,
        Err(e) => {
            crate::logger::warn!(
                tag = "cost_clusters",
                "segmented clusters unavailable: {:?}",
                e
            );
            Vec::new()
        }
    };
    for sc in segmented {
        let key = cluster_key(
            &sc.connector,
            &sc.card_network,
            &sc.variant,
            &sc.funding,
            &sc.issuer_country,
            &sc.currency,
            &sc.ic_category,
        );
        let ov = overrides.get(&key);
        // Volume-weighted blend of the fittable pieces, for a single headline number alongside the
        // per-segment detail.
        let (mut pnum, mut fnum, mut w) = (0.0f64, 0.0f64, 0.0f64);
        for s in &sc.segments {
            if let (Some(p), Some(fx)) = (s.pct_bps, s.fixed) {
                pnum += p * s.gross_sum;
                fnum += fx * s.gross_sum;
                w += s.gross_sum;
            }
        }
        let (blend_pct, blend_fixed) = if w > 0.0 {
            (finite(pnum / w), finite(fnum / w))
        } else {
            (None, None)
        };
        let (effective_pct_bps, effective_fixed, source) = match ov {
            Some(o) => (Some(o.pct_bps), Some(o.fixed), "override"),
            None => (blend_pct, blend_fixed, "segmented"),
        };
        // Already emitted by the fit pass: keep its identity/verdict/model rate and just attach the
        // pieces, re-pointing the effective fee at their blend (an override still wins).
        if let Some(&i) = seen.get(&key) {
            let row = &mut out[i];
            row.effective_pct_bps = effective_pct_bps;
            row.effective_fixed = effective_fixed;
            row.source = source.to_string();
            row.segments = sc
                .segments
                .into_iter()
                .map(ApiClusterSegment::from)
                .collect();
            continue;
        }
        // Not already listed, so appending it would bypass the column filter (the segment query is
        // scoped but not filtered). Attaching segments to a row the filter already admitted is fine;
        // introducing a NEW row is not.
        if !filter.is_empty() {
            continue;
        }
        seen.insert(key.clone(), out.len());
        out.push(ClusterFee {
            key,
            connector: sc.connector,
            card_network: sc.card_network,
            variant: sc.variant,
            funding: sc.funding,
            issuer_country: sc.issuer_country,
            currency: sc.currency,
            ic_category: sc.ic_category,
            n: sc.n,
            gross_sum: sc.gross_sum,
            model_pct_bps: None,
            model_fixed: None,
            override_pct_bps: ov.map(|o| o.pct_bps),
            override_fixed: ov.map(|o| o.fixed),
            override_updated_at: ov.map(|o| o.updated_at.clone()),
            effective_pct_bps,
            effective_fixed,
            source: source.to_string(),
            // Outside the fit pass's top-N, so no `cost_fee_model` row was read for it. Left blank
            // rather than assumed — the "N tiers" pill already says a single line didn't price it.
            verdict: String::new(),
            card_product: String::new(),
            segments: sc
                .segments
                .into_iter()
                .map(ApiClusterSegment::from)
                .collect(),
        });
    }

    // Merchant-wide view only: include any override whose cluster isn't in the current top set, so a
    // set override always stays visible and editable. A snapshot-scoped view shows only that
    // snapshot's segments, so we don't append unrelated overrides there.
    // Suppressed while a column filter is active as well: this append bypasses the WHERE clause, so
    // leaving it on would leak rows the user explicitly filtered out back into the result.
    let append_overrides = !scoped && filter.is_empty();
    for (key, o) in overrides.iter().filter(|_| append_overrides) {
        if seen.contains_key(key) {
            continue;
        }
        out.push(ClusterFee {
            key: key.clone(),
            connector: o.dims.connector.clone(),
            card_network: o.dims.card_network.clone(),
            variant: o.dims.variant.clone(),
            funding: o.dims.funding.clone(),
            issuer_country: o.dims.issuer_country.clone(),
            currency: o.dims.currency.clone(),
            ic_category: o.dims.ic_category.clone(),
            n: 0,
            gross_sum: 0.0,
            model_pct_bps: None,
            model_fixed: None,
            override_pct_bps: Some(o.pct_bps),
            override_fixed: Some(o.fixed),
            override_updated_at: Some(o.updated_at.clone()),
            effective_pct_bps: Some(o.pct_bps),
            effective_fixed: Some(o.fixed),
            source: "override".to_string(),
            // No fitted row behind this cluster at all — it exists only because it was overridden.
            verdict: String::new(),
            card_product: String::new(),
            segments: Vec::new(),
        });
    }

    Ok(Json(out))
}

#[derive(Debug, Deserialize)]
pub struct SetClusterOverrideRequest {
    pub pct_bps: f64,
    pub fixed: f64,
}

/// `PUT /merchant-account/:merchant_id/cost-clusters/:cluster_key/fee-override`
pub async fn set_cluster_override(
    Path((merchant_id, cluster_key)): Path<(String, String)>,
    Json(body): Json<SetClusterOverrideRequest>,
) -> Result<Json<ClusterOverride>, (StatusCode, String)> {
    if !body.pct_bps.is_finite()
        || !body.fixed.is_finite()
        || body.pct_bps < 0.0
        || body.fixed < 0.0
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "pct_bps and fixed must be finite and non-negative".to_string(),
        ));
    }
    let dims = ClusterDims::from_key(&cluster_key).ok_or((
        StatusCode::BAD_REQUEST,
        "cluster key must have 7 '|'-separated fields".to_string(),
    ))?;
    let ov = ClusterOverride {
        dims,
        pct_bps: body.pct_bps,
        fixed: body.fixed,
        updated_at: time::OffsetDateTime::now_utc()
            .format(&Iso8601::DEFAULT)
            .unwrap_or_default(),
    };
    overrides::put_cluster(&merchant_id, &ov)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    refresh_serving(&merchant_id).await;
    Ok(Json(ov))
}

/// `DELETE /merchant-account/:merchant_id/cost-clusters/:cluster_key/fee-override`
pub async fn delete_cluster_override(
    Path((merchant_id, cluster_key)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let dims = ClusterDims::from_key(&cluster_key).ok_or((
        StatusCode::BAD_REQUEST,
        "cluster key must have 7 '|'-separated fields".to_string(),
    ))?;
    overrides::delete_cluster(&merchant_id, &dims)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    refresh_serving(&merchant_id).await;
    Ok(StatusCode::NO_CONTENT)
}
