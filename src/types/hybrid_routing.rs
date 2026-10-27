use crate::decider::gatewaydecider::types::DomainDeciderRequestForApiCallV2;
use crate::euclid::types::RoutingRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, crate::auth::MerchantScoped)]
pub struct HybridRoutingRequest {
    #[merchant]
    pub static_routing_request: Option<RoutingRequest>,
    #[merchant]
    pub dynamic_routing_request: Option<DomainDeciderRequestForApiCallV2>,
}
