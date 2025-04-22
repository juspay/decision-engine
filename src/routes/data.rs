use std::sync::Arc;

use axum::{routing::post, Json};

#[cfg(feature = "limit")]
use axum::{error_handling::HandleErrorLayer, response::IntoResponse};

use crate::{
    crypto::{
        hash_manager::managers::sha::Sha512,
        keymanager::{self, KeyProvider},
    },
    custom_extractors::TenantStateResolver,
    error::{self, ContainerError, ResultContainerExt},
    logger,
    storage::{FingerprintInterface, HashInterface, OpenRouterInterface},
    tenant::GlobalAppState,
    utils,
};

use self::types::Validation;

pub mod crypto_operation;
mod transformers;
pub mod types;

#[cfg(feature = "limit")]
const BUFFER_LIMIT: usize = 1024;
#[cfg(feature = "limit")]
async fn ratelimit_err_handler(_: axum::BoxError) -> impl IntoResponse {
    (hyper::StatusCode::TOO_MANY_REQUESTS, "Rate Limit Applied")
}

///
/// Function for registering routes that is specifically handling the main open_router apis
///
#[allow(clippy::let_and_return)]
pub fn serve(
    #[cfg(feature = "limit")] global_app_state: Arc<GlobalAppState>,
) -> axum::Router<Arc<GlobalAppState>> {
    #[cfg(feature = "limit")]
    let ratelimit_middleware = tower::ServiceBuilder::new()
        .layer(HandleErrorLayer::new(ratelimit_err_handler))
        .buffer(
            global_app_state
                .global_config
                .limit
                .buffer_size
                .unwrap_or(BUFFER_LIMIT),
        )
        .load_shed()
        .rate_limit(
            global_app_state.global_config.limit.request_count,
            std::time::Duration::from_secs(global_app_state.global_config.limit.duration),
        )
        .into_inner();

    #[cfg(feature = "limit")]
    let delete_route = post(delete_card).layer(ratelimit_middleware);

    #[cfg(not(feature = "limit"))]
    let delete_route = post(delete_card);

    let router = axum::Router::new()
        .route("/delete", delete_route)
        .route("/add", post(add_card))
        .route("/retrieve", post(retrieve_card))
        .route("/fingerprint", post(get_or_insert_fingerprint));

    router
}

/// `/data/add` handling the requirement of storing data
pub async fn add_card(
    TenantStateResolver(tenant_app_state): TenantStateResolver,
    Json(request): Json<types::StoreCardRequest>,
) -> Result<Json<types::StoreCardResponse>, ContainerError<error::ApiError>> {
    request.validate()?;

    let hash_data = transformers::get_hash(&request.data, Sha512)
        .change_error(error::ApiError::EncodingError)?;

    let optional_hash_table = tenant_app_state.db.find_by_data_hash(&hash_data).await?;

    let crypto_manager = keymanager::get_dek_manager()
        .find_or_create_entity(&tenant_app_state, request.merchant_id.clone())
        .await?;

    let (duplication_check, output) = match optional_hash_table {
        Some(hash_table) => {
            let stored_data = tenant_app_state
                .db
                .find_by_hash_id_merchant_id_customer_id(
                    &hash_table.hash_id,
                    &request.merchant_id,
                    &request.merchant_customer_id,
                )
                .await?;

            let (duplication_check, output) = match stored_data {
                Some(open_router) => {
                    let decrypted_open_router_data =
                        crypto_operation::decrypt_data(&tenant_app_state, crypto_manager, open_router)
                            .await?;

                    let duplication_check = transformers::get_data_duplication_status(
                        &decrypted_open_router_data,
                        &request.data,
                    )?;

                    (Some(duplication_check), decrypted_open_router_data)
                }
                None => {
                    let encrypted_open_router_data = crypto_operation::encrypt_data_and_insert_into_db(
                        &tenant_app_state,
                        crypto_manager,
                        request,
                        &hash_table.hash_id,
                    )
                    .await?;

                    (None, encrypted_open_router_data)
                }
            };

            (duplication_check, output)
        }
        None => {
            let hash_table = tenant_app_state.db.insert_hash(hash_data).await?;

            let encrypted_open_router_data = crypto_operation::encrypt_data_and_insert_into_db(
                &tenant_app_state,
                crypto_manager,
                request,
                &hash_table.hash_id,
            )
            .await?;

            (None, encrypted_open_router_data)
        }
    };

    let response = Json(types::StoreCardResponse::from((duplication_check, output)));
    logger::info!(add_card_response=?response);

    Ok(response)
}
