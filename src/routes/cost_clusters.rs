//! Merchant-facing API for the highest-traffic fitted clusters and their per-cluster fee overrides.
//!
//! Powers both the "ingested data" view (what each of the merchant's biggest segments costs) and the
//! surgical per-cluster override (fix the fee on the top segments). A cluster override wins over a
//! connector override and the learned model at decide time — see
//! [`crate::cost_ingestion::serving::lookup`].

use std::collections::HashMap;

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Iso8601;

use crate::cost_ingestion::blended::{self, ClusterScope, TopCluster};
use crate::cost_ingestion::overrides::{self, key_of_dims, ClusterDims, ClusterOverride};
use crate::routes::connector_fees::{clickhouse_config, refresh_serving};

/// How many top clusters to surface by default (top by GMV). Capped so the response and the
/// override target list stay small and scannable.
const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 50;

fn dims_from_cluster(c: &TopCluster) -> ClusterDims {
    let has_segment_key = !c.interchange_bps.is_empty() || c.segment_idx != 0;
    ClusterDims {
        connector: c.connector.to_lowercase(),
        card_network: c.card_network.to_lowercase(),
        variant: c.variant.to_lowercase(),
        funding: c.funding.to_lowercase(),
        issuer_country: c.issuer_country.to_lowercase(),
        currency: c.currency.to_lowercase(),
        ic_category: c.ic_category.to_lowercase(),
        interchange_bps: (!c.interchange_bps.is_empty()).then(|| c.interchange_bps.to_lowercase()),
        segment_idx: has_segment_key.then_some(c.segment_idx),
    }
}

fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn nonzero_segment_value(v: f64, segment_idx: Option<u16>) -> Option<f64> {
    segment_idx.filter(|idx| *idx > 0).map(|_| v)
}

fn nonzero_quality(v: f64) -> Option<f64> {
    (v != 0.0).then_some(v)
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
    pub interchange_bps: Option<String>,
    pub segment_idx: Option<u16>,
    pub amount_lo: Option<f64>,
    pub amount_hi: Option<f64>,
    /// Transaction count and settled GMV for the cluster (0 for an override-only cluster no longer
    /// in the top set).
    pub n: u64,
    pub gross_sum: f64,
    /// Learned fee (present when the cluster is in the fitted snapshot).
    pub model_pct_bps: Option<f64>,
    pub model_fixed: Option<f64>,
    pub grade_bps: Option<f64>,
    pub pct_ci95_bps: Option<f64>,
    pub astar: Option<f64>,
    pub prop_bps: Option<f64>,
    pub fix_abs: Option<f64>,
    pub fix_bps: Option<f64>,
    pub below_gross_frac: Option<f64>,
    pub fan_frac: Option<f64>,
    pub fan_money_bps: Option<f64>,
    /// Manual override, when set.
    pub override_pct_bps: Option<f64>,
    pub override_fixed: Option<f64>,
    pub override_updated_at: Option<String>,
    /// Fee actually used at decide time and where it comes from.
    pub effective_pct_bps: f64,
    pub effective_fixed: f64,
    /// `"override"` | `"model"`.
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct TopClustersQuery {
    pub limit: Option<u32>,
    /// Scope to one ingested snapshot (all three required together): the fitted segments of that
    /// specific report. Omit for the merchant-wide latest-snapshot view (the override targets).
    pub connector: Option<String>,
    pub account: Option<String>,
    pub report_date: Option<String>,
}

/// `GET /merchant-account/:merchant_id/cost-clusters?limit=N[&connector&account&report_date]` — top
/// segments by GMV. Narrowed by any of `connector` / `account` / `report_date`: a connector (+account)
/// gives that connector's latest-snapshot segments (the override targets under a connector); adding
/// `report_date` pins one exact ingestion's segments. Unscoped is merchant-wide. Overrides are merged
/// in; in the unscoped view an overridden segment stays visible even if it drops out of the top set.
pub async fn list_cost_clusters(
    Path(merchant_id): Path<String>,
    Query(q): Query<TopClustersQuery>,
) -> Result<Json<Vec<ClusterFee>>, (StatusCode, String)> {
    let cfg = clickhouse_config()?;
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let scope = ClusterScope {
        connector: q.connector.as_deref(),
        account: q.account.as_deref(),
        report_date: q.report_date.as_deref(),
    };
    // "Scoped" = narrowed to a connector/account/snapshot; only then do we suppress the
    // append-overrides-outside-the-top-set behavior (that's a merchant-wide affordance).
    let scoped = q.connector.is_some() || q.account.is_some() || q.report_date.is_some();

    // Surface a ClickHouse failure as a 500 with its message — a swallowed error here is
    // indistinguishable from "no segments" and hides real query bugs.
    let top: Vec<TopCluster> = blended::top_clusters(&cfg, &merchant_id, limit, scope)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    let overrides: HashMap<String, ClusterOverride> = overrides::list_clusters(&merchant_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|o| (key_of_dims(&o.dims), o))
        .collect();

    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out: Vec<ClusterFee> = Vec::new();

    for c in top {
        let dims = dims_from_cluster(&c);
        let key = key_of_dims(&dims);
        seen.insert(key.clone());
        let ov = overrides.get(&key);
        let (effective_pct_bps, effective_fixed, source) = match ov {
            Some(o) => (o.pct_bps, o.fixed, "override"),
            None => (c.pct_bps, c.fixed, "model"),
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
            interchange_bps: empty_to_none(c.interchange_bps),
            segment_idx: dims.segment_idx,
            amount_lo: nonzero_segment_value(c.amount_lo, dims.segment_idx),
            amount_hi: nonzero_segment_value(c.amount_hi, dims.segment_idx),
            n: c.n,
            gross_sum: c.gross_sum,
            model_pct_bps: Some(c.pct_bps),
            model_fixed: Some(c.fixed),
            grade_bps: Some(c.grade_bps),
            pct_ci95_bps: Some(c.pct_ci95_bps),
            astar: nonzero_quality(c.astar),
            prop_bps: nonzero_quality(c.prop_bps),
            fix_abs: nonzero_quality(c.fix_abs),
            fix_bps: nonzero_quality(c.fix_bps),
            below_gross_frac: Some(c.below_gross_frac),
            fan_frac: Some(c.fan_frac),
            fan_money_bps: Some(c.fan_money_bps),
            override_pct_bps: ov.map(|o| o.pct_bps),
            override_fixed: ov.map(|o| o.fixed),
            override_updated_at: ov.map(|o| o.updated_at.clone()),
            effective_pct_bps,
            effective_fixed,
            source: source.to_string(),
        });
    }

    // Merchant-wide view only: include any override whose cluster isn't in the current top set, so a
    // set override always stays visible and editable. A snapshot-scoped view shows only that
    // snapshot's segments, so we don't append unrelated overrides there.
    for (key, o) in overrides.iter().filter(|_| !scoped) {
        if seen.contains(key) {
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
            interchange_bps: o.dims.interchange_bps.clone(),
            segment_idx: o.dims.segment_idx,
            amount_lo: None,
            amount_hi: None,
            n: 0,
            gross_sum: 0.0,
            model_pct_bps: None,
            model_fixed: None,
            grade_bps: None,
            pct_ci95_bps: None,
            astar: None,
            prop_bps: None,
            fix_abs: None,
            fix_bps: None,
            below_gross_frac: None,
            fan_frac: None,
            fan_money_bps: None,
            override_pct_bps: Some(o.pct_bps),
            override_fixed: Some(o.fixed),
            override_updated_at: Some(o.updated_at.clone()),
            effective_pct_bps: o.pct_bps,
            effective_fixed: o.fixed,
            source: "override".to_string(),
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
        "cluster key must have 7, 8, or 9 '|'-separated fields".to_string(),
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
        "cluster key must have 7, 8, or 9 '|'-separated fields".to_string(),
    ))?;
    overrides::delete_cluster(&merchant_id, &dims)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    refresh_serving(&merchant_id).await;
    Ok(StatusCode::NO_CONTENT)
}
