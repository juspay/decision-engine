use crate::decider::gatewaydecider::types::DomainDeciderRequestForApiCallV2;
use crate::euclid::types::RoutingRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridRoutingRequest {
    pub static_routing_request: Option<RoutingRequest>,
    pub dynamic_routing_request: Option<DomainDeciderRequestForApiCallV2>,
}

impl crate::auth::MerchantScoped for HybridRoutingRequest {
    fn merchant_ids(&self) -> Vec<&str> {
        self.static_routing_request
            .iter()
            .map(|request| request.created_by.as_str())
            .chain(
                self.dynamic_routing_request
                    .iter()
                    .map(|request| request.merchant_id.as_str()),
            )
            .collect()
    }
}
