use masking::{ExposeInterface, PeekInterface, Secret, StrongSecret};

use crate::{
    app::TenantAppState,
    crypto::{
        encryption_manager::{encryption_interface::Encryption, managers::aes::GcmAes256},
        keymanager::CryptoOperationsManager,
    },
    error::{self, ContainerError},
};

pub struct InternalKeyManager;

#[async_trait::async_trait]
impl super::KeyProvider for InternalKeyManager {
    async fn find_by_entity_id(
        &self,
        _tenant_app_state: &TenantAppState,
        _entity_id: String,
    ) -> Result<Box<dyn CryptoOperationsManager>, ContainerError<error::ApiError>> {
        todo!()
    }

    async fn find_or_create_entity(
        &self,
        _tenant_app_state: &TenantAppState,
        _entity_id: String,
    ) -> Result<Box<dyn CryptoOperationsManager>, ContainerError<error::ApiError>> {
        todo!()
    }
}

pub struct InternalCryptoManager(GcmAes256);

impl InternalCryptoManager {
    // fn from_secret_key(key: Secret<Vec<u8>>) -> Self {
    //     Self(GcmAes256::new(key.expose()))
    // }

    fn get_inner(&self) -> &GcmAes256 {
        &self.0
    }
}

#[async_trait::async_trait]
impl CryptoOperationsManager for InternalCryptoManager {
    async fn encrypt_data(
        &self,
        _tenant_app_state: &TenantAppState,
        decryted_data: StrongSecret<Vec<u8>>,
    ) -> Result<Secret<Vec<u8>>, ContainerError<error::ApiError>> {
        Ok(self
            .get_inner()
            .encrypt(decryted_data.peek().clone())?
            .into())
    }
    async fn decrypt_data(
        &self,
        _tenant_app_state: &TenantAppState,
        encrypted_data: Secret<Vec<u8>>,
    ) -> Result<StrongSecret<Vec<u8>>, ContainerError<error::ApiError>> {
        Ok(self.get_inner().decrypt(encrypted_data.expose())?.into())
    }
}
