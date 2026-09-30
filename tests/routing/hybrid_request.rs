#![allow(clippy::panic_in_result_fn)]

use super::prepare_hybrid_request;
use crate::{
    decider::gatewaydecider::types::DomainDeciderRequestForApiCallV2,
    types::hybrid_routing::HybridRoutingRequest,
};
use serde_json::{json, Value};

fn hybrid_payload() -> Value {
    json!({
        "static_routing_request": {
            "created_by": "pro_static",
            "payment_id": "pay_static",
            "parameters": {
                "amount": {"type": "number", "value": 6540},
                "currency": {"type": "enum_variant", "value": "USD"},
                "payment_method": {"type": "enum_variant", "value": "card"},
                "payment_method_type": {"type": "enum_variant", "value": "credit"}
            },
            "fallback_output": [
                {"gateway_name": "pretendpay", "gateway_id": "mca_one"},
                {"gateway_name": "fauxpay", "gateway_id": "mca_two"}
            ]
        },
        "dynamic_routing_request": {
            "merchantId": "pro_dynamic",
            "eligibleGatewayList": ["stripe:mca_other"],
            "rankingAlgorithm": "NTW_SR_HYBRID_ROUTING",
            "eliminationEnabled": true,
            "enableMultiObjective": true,
            "paymentInfo": {
                "paymentId": "pay_dynamic",
                "amount": 100,
                "currency": "CAD",
                "paymentType": "ORDER_PAYMENT",
                "paymentMethod": "card",
                "paymentMethodType": "UPI"
            }
        }
    })
}

#[test]
fn hybrid_rebuild_preserves_only_preferred_connectors() -> Result<(), serde_json::Error> {
    let mut payload = hybrid_payload();
    let preferences = json!(["pretendpay:mca_one", "fauxpay:mca_two"]);
    payload["dynamic_routing_request"]["paymentInfo"]["preferredConnectors"] = preferences.clone();
    let payload: HybridRoutingRequest = serde_json::from_value(payload)?;
    let mut expected = serde_json::to_value(
        payload
            .static_routing_request
            .as_ref()
            .map(DomainDeciderRequestForApiCallV2::from),
    )?;
    expected["paymentInfo"]["preferredConnectors"] = preferences;

    let prepared = prepare_hybrid_request(payload.clone());
    assert_eq!(
        serde_json::to_value(&prepared.dynamic_routing_request)?,
        expected
    );
    assert_eq!(
        serde_json::to_value(prepared.static_routing_request)?,
        serde_json::to_value(payload.static_routing_request)?
    );
    Ok(())
}

#[test]
fn hybrid_rebuild_handles_absent_null_and_empty_preferences() -> Result<(), serde_json::Error> {
    for preferences in [None, Some(Value::Null), Some(json!([]))] {
        let mut payload = hybrid_payload();
        if let Some(value) = &preferences {
            payload["dynamic_routing_request"]["paymentInfo"]["preferredConnectors"] =
                value.clone();
        }
        let payload: HybridRoutingRequest = serde_json::from_value(payload)?;
        let prepared = prepare_hybrid_request(payload);
        let serialized = serde_json::to_value(prepared.dynamic_routing_request)?;
        assert_eq!(
            serialized
                .get("paymentInfo")
                .and_then(|info| info.get("preferredConnectors")),
            preferences.as_ref().filter(|value| !value.is_null())
        );
    }
    Ok(())
}

#[test]
fn hybrid_rebuild_keeps_static_only_behavior() -> Result<(), serde_json::Error> {
    let mut payload = hybrid_payload();
    payload["dynamic_routing_request"] = Value::Null;
    let payload: HybridRoutingRequest = serde_json::from_value(payload)?;
    let expected = payload
        .static_routing_request
        .as_ref()
        .map(DomainDeciderRequestForApiCallV2::from);
    let prepared = prepare_hybrid_request(payload);
    assert_eq!(
        serde_json::to_value(prepared.dynamic_routing_request)?,
        serde_json::to_value(expected)?
    );
    Ok(())
}

#[test]
fn hybrid_rebuild_requires_static_input() -> Result<(), serde_json::Error> {
    for dynamic_request in [
        Value::Null,
        hybrid_payload()["dynamic_routing_request"].clone(),
    ] {
        let payload: HybridRoutingRequest = serde_json::from_value(json!({
            "static_routing_request": null,
            "dynamic_routing_request": dynamic_request
        }))?;
        let prepared = prepare_hybrid_request(payload);
        assert!(prepared.static_routing_request.is_none());
        assert!(prepared.dynamic_routing_request.is_none());
    }
    Ok(())
}
