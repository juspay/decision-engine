//! `/routing/hybrid`, kept for callers that have not moved to `/decide`. It runs the same
//! evaluation and answers with its audit record.

use crate::custom_extractors::{RequestIds, ScopedJson};
use crate::error::ContainerError;
use crate::euclid::errors::EuclidErrors;
use crate::types::hybrid_routing::HybridRoutingRequest;

#[axum::debug_handler]
pub async fn hybrid_routing_evaluate(
    ids: RequestIds,
    ScopedJson { payload, .. }: ScopedJson<HybridRoutingRequest>,
) -> Result<axum::response::Response, ContainerError<EuclidErrors>> {
    super::decide::evaluation::evaluate(ids, payload, None, |_, _| Ok(None)).await
}
