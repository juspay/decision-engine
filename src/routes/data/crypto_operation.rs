use masking::ExposeInterface;

use crate::{
    app::TenantAppState,
    crypto::keymanager::CryptoOperationsManager,
    error::{self, ContainerError, ResultContainerExt},
    routes::data::types,
    storage::{
        storage_v2::{types::VaultNew, VaultInterface},
        types::{OpenRouter, OpenRouterNew},
        OpenRouterInterface,
    },
};

pub async fn encrypt_data_and_insert_into_db<'a>(
    tenant_app_state: &'a TenantAppState,
    crypto_operator: Box<dyn CryptoOperationsManager>,
    request: types::StoreCardRequest,
    hash_id: &'a str,
) -> Result<OpenRouter, ContainerError<error::ApiError>> {
    let data_to_be_encrypted = match request.data.clone() {
        types::Data::Card { card } => Ok(types::StoredData::CardData(card)),
        types::Data::EncData { enc_card_data } => Ok(types::StoredData::EncData(enc_card_data)),
    }
    .and_then(|inner| serde_json::to_vec(&inner).change_error(error::ApiError::EncodingError))?;

    let encrypted_data = crypto_operator
        .encrypt_data(tenant_app_state, data_to_be_encrypted.into())
        .await?;

    let open_router_new = OpenRouterNew::new(request, hash_id, encrypted_data.into());

    let open_router = tenant_app_state
        .db
        .insert_or_get_from_open_router(open_router_new)
        .await?;

    Ok(open_router)
}

pub async fn decrypt_data<T>(
    tenant_app_state: &TenantAppState,
    crypto_operator: Box<dyn CryptoOperationsManager>,
    mut data: T,
) -> Result<T, ContainerError<error::ApiError>>
where
    T: types::SecretDataManager,
{
    if let Some(encrypted_data) = data.get_encrypted_inner_value() {
        let decrypted_data = crypto_operator
            .decrypt_data(tenant_app_state, encrypted_data)
            .await?;

        data = data.set_decrypted_data(decrypted_data);
    }
    Ok(data)
}

