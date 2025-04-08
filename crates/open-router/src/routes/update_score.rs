// use std::sync::Arc;
//
// use axum::{routing::post, Json};
//
// #[cfg(feature = "limit")]
// use axum::{error_handling::HandleErrorLayer, response::IntoResponse};
//
// use crate::{
//     crypto::{
//         hash_manager::managers::sha::Sha512,
//         keymanager::{self, KeyProvider},
//     },
//     custom_extractors::TenantStateResolver,
//     error::{self, ContainerError, ResultContainerExt},
//     logger,
//     storage::{FingerprintInterface, HashInterface, OpenRouterInterface},
//     tenant::GlobalAppState,
//     utils,
// };

use axum::body::to_bytes;
use serde::{Deserialize, Serialize};
use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};
use crate::feedback::gateway_scoring_service::check_and_update_gateway_score;
use crate::types::txn_details::types::TxnDetail;
use crate::types::card::txn_card_info::TxnCardInfo;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct UpdateScoreRequest {
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    log_message: String,
    enforce_dynaic_routing_failure: Option<bool>,
    gateway_reference_id : Option<String>,
}


#[axum::debug_handler]
pub async fn update_score(req: axum::http::Request<axum::body::Body>) -> Result<&'static str, ErrorResponse>
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

    let update_score_request: Result<UpdateScoreRequest, _> = serde_json::from_slice(&body);
    match update_score_request {
        Ok(payload) => {


            if payload.txn_detail.gateway.is_none() {
                return Err(ErrorResponse {
                    status: "400".to_string(),
                    error_code: "400".to_string(),
                    error_message: "Gateway is empty".to_string(),
                    priority_logic_tag: None,
                    routing_approach: None,
                    filter_wise_gateways: None,
                    error_info: UnifiedError{
                        code: "GATEWAY_NOT_FOUND".to_string(),
                        user_message: "Request params does not have gateway, please provide the gateway to update score.".to_string(),
                        developer_message: "Gateway field is empty. Not able to update score.".to_string(),
                    },
                    priority_logic_output: None,
                    is_dynamic_mga_enabled: false,
                });
            }


            let txn_detail = payload.txn_detail;
            let txn_card_info = payload.txn_card_info;
            let log_message = payload.log_message;
            let enforce_failure = payload.enforce_dynaic_routing_failure.unwrap_or(false);
            let gateway_reference_id = payload.gateway_reference_id;
            check_and_update_gateway_score(txn_detail, txn_card_info, log_message.as_str(), enforce_failure, gateway_reference_id).await;
            return Ok("Success");
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
    }

    
}
