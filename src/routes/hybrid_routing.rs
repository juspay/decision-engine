use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::flow_new::decider_full_payload_hs_function;
use crate::error::ContainerError;
use crate::euclid::errors::EuclidErrors;
use crate::euclid::handlers::routing_rules::routing_evaluate;
use crate::types::hybrid_routing::HybridRoutingRequest;
use axum::{Json, response::IntoResponse};
use std::time::Instant;

#[axum::debug_handler]
pub async fn hybrid_routing_evaluate(
    Json(payload): Json<HybridRoutingRequest>,
) -> Result<axum::response::Response, ContainerError<EuclidErrors>> {
    let mut eligible_gateways: Option<Vec<String>> = None;
    let mut static_routing_response = None;

    if let Some(static_req) = payload.static_routing_request {
        let response_result = routing_evaluate(Json(static_req)).await?;
        let response = response_result.0;
        
        let connectors: Vec<String> = response
            .eligible_connectors
            .iter()
            .cloned()
            .map(|c| c.gateway_name)
            .collect();
        
        eligible_gateways = Some(connectors);
        static_routing_response = Some(response);
    }

    if let Some(mut dynamic_req) = payload.dynamic_routing_request {
        if static_routing_response.is_some() {
            dynamic_req.eligible_gateway_list = eligible_gateways;
        }

        let cpu_start = Instant::now();
        let result = decider_full_payload_hs_function(dynamic_req, cpu_start).await;
        
        match result {
            Ok(decided) => {
                let mut res = serde_json::Map::new();
                if let Some(static_res) = static_routing_response {
                    res.insert(
                        "static_routing".to_string(),
                        serde_json::to_value(static_res).unwrap_or_default(),
                    );
                }
                res.insert(
                    "dynamic_routing".to_string(),
                    serde_json::to_value(decided).unwrap_or_default(),
                );
                Ok(Json(serde_json::Value::Object(res)).into_response())
            }
            Err(err) => Ok(err.into_response()),
        }
    } else if let Some(static_res) = static_routing_response {
        let mut res = serde_json::Map::new();
        res.insert(
            "static_routing".to_string(),
            serde_json::to_value(static_res).unwrap_or_default(),
        );
        Ok(Json(serde_json::Value::Object(res)).into_response())
    } else {
        Err(EuclidErrors::InvalidRequest(
            "At least one of static_routing_request or dynamic_routing_request must be provided.".to_string(),
        )
        .into())
    }
}
