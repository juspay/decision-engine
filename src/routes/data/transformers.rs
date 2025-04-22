use masking::{ExposeInterface, PeekInterface};

use crate::{
    crypto::hash_manager::hash_interface::Encode,
    error::{self, ContainerError, ResultContainerExt},
    storage,
};

use super::types::{self, DataDuplicationCheck};

impl From<(Option<DataDuplicationCheck>, storage::types::OpenRouter)>
    for super::types::StoreCardResponse
{
    fn from(
        (duplication_check, value): (Option<DataDuplicationCheck>, storage::types::OpenRouter),
    ) -> Self {
        Self {
            status: types::Status::Ok,
            payload: Some(super::types::StoreCardRespPayload {
                card_reference: value.open_router_id.expose(),
                duplication_check,
                dedup: None,
            }),
        }
    }
}

impl TryFrom<storage::types::OpenRouter> for super::types::RetrieveCardResponse {
    type Error = ContainerError<error::ApiError>;
    fn try_from(value: storage::types::OpenRouter) -> Result<Self, Self::Error> {
        let decrypted_data = value
            .data
            .get_decrypted_inner_value()
            .ok_or::<ContainerError<_>>(error::ApiError::DecodingError.into())?;
        let (card, enc_card_data) =
            match serde_json::from_slice::<types::StoredData>(decrypted_data.peek())
                .change_error(error::ApiError::DecodingError)?
            {
                types::StoredData::EncData(data) => (None, Some(data)),
                types::StoredData::CardData(card) => (Some(card), None),
            };

        Ok(Self {
            status: types::Status::Ok,
            payload: Some(super::types::RetrieveCardRespPayload {
                card,
                enc_card_data,
            }),
        })
    }
}

impl From<storage::types::Fingerprint> for super::types::FingerprintResponse {
    fn from(value: storage::types::Fingerprint) -> Self {
        Self {
            fingerprint_id: value.fingerprint_id,
        }
    }
}

impl std::cmp::PartialEq<types::Data> for super::types::StoredData {
    fn eq(&self, other: &types::Data) -> bool {
        match (self, other) {
            (Self::EncData(request_enc_card_data), types::Data::EncData { enc_card_data }) => {
                request_enc_card_data == enc_card_data
            }
            (Self::CardData(request_card), types::Data::Card { card }) => request_card == card,
            _ => false,
        }
    }
}

pub fn get_hash<T>(
    request: &types::Data,
    hash_algorithm: T,
) -> Result<Vec<u8>, ContainerError<error::ApiError>>
where
    T: Encode<
        Vec<u8>,
        Vec<u8>,
        ReturnType<Vec<u8>> = Result<Vec<u8>, error::ContainerError<error::CryptoError>>,
    >,
{
    let data = match request {
        types::Data::EncData { enc_card_data } => enc_card_data,
        types::Data::Card { card } => card.card_number.peek(),
    };

    let json_data = serde_json::to_vec(data).change_error(error::ApiError::EncodingError)?;

    let hash_data = hash_algorithm.encode(json_data)?;

    Ok(hash_data)
}

pub fn get_data_duplication_status(
    stored_payload: &storage::types::OpenRouter,
    request_data: &types::Data,
) -> Result<DataDuplicationCheck, ContainerError<error::ApiError>> {
    let decrypted_data = stored_payload
        .data
        .get_decrypted_inner_value()
        .ok_or::<ContainerError<_>>(error::ApiError::DecodingError.into())?;
    let stored_data = serde_json::from_slice::<types::StoredData>(decrypted_data.peek())
        .change_error(error::ApiError::DecodingError)?;

    let is_metadata_duplicated = stored_data.eq(request_data);

    Ok(match is_metadata_duplicated {
        true => DataDuplicationCheck::Duplicated,
        false => DataDuplicationCheck::MetaDataChanged,
    })
}
