use crate::euclid::types::RoutingRequest;
use crate::decider::gatewaydecider::types::DomainDeciderRequestForApiCallV2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinedRoutingRequest {
    pub static_routing_request: Option<RoutingRequest>,
    pub dynamic_routing_request: Option<DomainDeciderRequestForApiCallV2>,
}
