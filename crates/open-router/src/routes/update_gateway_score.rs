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
use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};
use crate::feedback::gateway_scoring_service::check_and_update_gateway_score_;
use crate::feedback::types::UpdateScorePayload;


#[axum::debug_handler]
pub async fn update_gateway_score(req: axum::http::Request<axum::body::Body>) -> Result<&'static str, ErrorResponse>
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

    let update_score_request: Result<UpdateScorePayload, _> = serde_json::from_slice(&body);
    match update_score_request {
        Ok(payload) => {
            check_and_update_gateway_score_(payload).await;
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
