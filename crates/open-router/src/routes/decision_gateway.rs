// use std::sync::Arc;

// use axum::{routing::post, Json};

// #[cfg(feature = "limit")]
// use axum::{error_handling::HandleErrorLayer, response::IntoResponse};

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
//     decider::gatewaydecider::flows::runDeciderFlow,
//     decider::gatewaydecider::types::DeciderParams,
// };

use crate::{app::APP_STATE, tenant::GlobalAppState};



#[axum::debug_handler]
pub async fn decision_gateway() -> &'static str {
    // let state = GlobalAppState::get_app_state_of_tenant(&global_app_state, "tenant_id").await.unwrap();
    // let connection = state.db.get_conn().await.unwrap();
    println!("Starting Decision Gateway");
    let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "jungleerummy".to_string(), "".to_string()).await;
    println!("Decision Gateway expect true: {:?}", res);

    let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "ClassicRummy".to_string(), "".to_string()).await;
    println!("Decision Gateway expect false: {:?}", res);

    let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "zeptomarketplace".to_string(), "".to_string()).await;
    println!("Decision Gateway expect false: {:?}", res);
    
    "Decision Gateway"
}
