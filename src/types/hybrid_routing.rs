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

/// The payment both halves evaluate. Each half names it; the static one is read first.
impl crate::analytics::payload::ApiEventPayload for HybridRoutingRequest {
    fn payment_id(&self) -> Option<&str> {
        self.static_routing_request
            .as_ref()
            .and_then(|request| request.payment_id.as_deref())
            .or_else(|| {
                self.dynamic_routing_request
                    .as_ref()
                    .map(|request| request.payment_id())
            })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::HybridRoutingRequest;
    use crate::analytics::payload::ApiEventPayload;

    /// The documented `/routing/hybrid` request, with each half naming its own payment.
    fn documented_request() -> serde_json::Value {
        let docs = include_str!("../../docs/api-reference/endpoint/hybridRouting.mdx");
        let start = docs.find("--data '").unwrap() + "--data '".len();
        let end = start + docs[start..].find("\n}'").unwrap() + 2;
        let mut request: serde_json::Value = serde_json::from_str(&docs[start..end]).unwrap();
        request["static_routing_request"]["payment_id"] = "pay_static".into();
        request["dynamic_routing_request"]["paymentInfo"]["paymentId"] = "pay_dynamic".into();
        request
    }

    #[test]
    fn the_payment_is_read_from_the_static_half_then_the_dynamic_one() {
        let mut request = documented_request();
        let both: HybridRoutingRequest = serde_json::from_value(request.clone()).unwrap();
        assert_eq!(both.payment_id(), Some("pay_static"));

        request["static_routing_request"] = serde_json::Value::Null;
        let dynamic_only: HybridRoutingRequest = serde_json::from_value(request).unwrap();
        assert_eq!(dynamic_only.payment_id(), Some("pay_dynamic"));
    }
}
