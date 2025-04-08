use crate::{decider::gatewaydecider::{
    flows::deciderFullPayloadHSFunction,
    types::{DecidedGateway, DomainDeciderRequest, ErrorResponse, UnifiedError},
}, types::gateway::Gateway};
use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

impl IntoResponse for DecidedGatewayResponse {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        let body = serde_json::to_string(&self).unwrap();
        axum::http::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        let body = serde_json::to_string(&self).unwrap();
        axum::http::Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DecidedGatewayResponse {
    pub decided_gateway: DecidedGateway,
    pub filter_list: Vec<(String, Vec<Gateway>)>,
}

#[axum::debug_handler]
pub async fn decision_gateway(
    req: axum::http::Request<axum::body::Body>,
) -> Result<DecidedGatewayResponse, ErrorResponse>
where   
    DecidedGatewayResponse: IntoResponse, // Ensure it's a valid response type
    ErrorResponse: IntoResponse,  //
    
{
    let headers = req.headers();
    for (name, value) in headers.iter() {
        println!("Header: {}: {:?}", name, value);
    }
    let body = match to_bytes(req.into_body(), usize::MAX).await {
        Ok(body) => {
            println!("Body: {:?}", body);
            body
        }
        Err(e) => {
            println!("Error: {:?}", e);
            return Err(ErrorResponse {
                status: "400".to_string(),
                error_code: "400".to_string(),
                error_message: "Error parsing request".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError{
                    code: "INVALID_INPUT".to_string(),
                    user_message: "Invalid request params. Please verify your input.".to_string(),
                    developer_message: e.to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            });
        }
    };
    let api_decider_request: Result<DomainDeciderRequest, _> = serde_json::from_slice(&body);
    match api_decider_request {
        Ok(payload) => match deciderFullPayloadHSFunction(payload).await {
            Ok((decided_gateway, filter_list)) => {

                let response: DecidedGatewayResponse = DecidedGatewayResponse {
                    decided_gateway: decided_gateway,
                    filter_list,
                };


                Ok(response)
            }
            Err(e) => {
                println!("Error: {:?}", e);
                Err(e)
            }
        },
        Err(e) => {
            println!("Error: {:?}", e);
            Err(ErrorResponse {
                status: "400".to_string(),
                error_code: "400".to_string(),
                error_message: "Error parsing request".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError{
                    code: "INVALID_INPUT".to_string(),
                    user_message: "Invalid request params. Please verify your input.".to_string(),
                    developer_message: e.to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            })
        }
    }

    // let connection = state.db.get_conn().await.unwrap();
    // println!("Starting Decision Gateway");
    // let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "jungleerummy".to_string(), "".to_string()).await;
    // println!("Decision Gateway expect true: {:?}", res);

    // let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "ClassicRummy".to_string(), "".to_string()).await;
    // println!("Decision Gateway expect false: {:?}", res);

    // let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "zeptomarketplace".to_string(), "".to_string()).await;
    // println!("Decision Gateway expect false: {:?}", res);
}

// SELECT `merchant_iframe_preferences`.`id`, `merchant_iframe_preferences`.`merchant_id`, `merchant_iframe_preferences`.`dynamic_switching_enabled`, `merchant_iframe_preferences`.`isin_routing_enabled`, `merchant_iframe_preferences`.`issuer_routing_enabled`, `merchant_iframe_preferences`.`txn_failure_gateway_penality`, `merchant_iframe_preferences`.`card_brand_routing_enabled` FROM `merchant_iframe_preferences` WHERE (`merchant_iframe_preferences`.`merchant_id` = ?)
