use error_stack::ResultExt;

use crate::{
    decider::{gatewaydecider, network_decider::types, storage::utils::co_badged_card_info},
    error, logger,
    storage::types::CoBadgedCardInfo,
    utils::CustomResult,
};

pub struct CoBadgedCardInfoList(Vec<CoBadgedCardInfo>);

impl CoBadgedCardInfoList {
    fn pad_card_number_to_19_digit(card_isin: String) -> String {
        format!("{:0<19}", card_isin)
    }
    pub fn is_valid_length(&self) -> bool {
        if self.0.len() < 2 {
            logger::debug!("Invalid co-badged network list length");
            false
        } else {
            logger::debug!("Valid co-badged network list length");
            true
        }
    }

    pub fn filter_cards(self) -> Self {
        logger::debug!(
            "Filtering co-badged cards, Total cards before filtering: {}",
            self.0.len()
        );

        let filtered_cards: Vec<CoBadgedCardInfo> = self
            .0
            .into_iter()
            .filter(|card| {
                card.card_type == types::CardType::Debit
                    && card.pan_or_token == types::PanOrToken::Pan
                    && !card.prepaid
            })
            .collect();

        logger::debug!(
            "Filtering complete. Total cards after filtering: {}",
            filtered_cards.len()
        );

        Self(filtered_cards)
    }

    pub fn has_same_issuer(&self) -> CustomResult<bool, error::ApiError> {
        let first_element = self
            .0
            .first()
            .ok_or(error::ApiError::UnknownError)
            .attach_printable("The filtered co-badged card info list is empty")?;

        let first_issuer = &first_element.issuing_bank_name;
        let has_same_issuer = self
            .0
            .iter()
            .all(|card| &card.issuing_bank_name == first_issuer);
        Ok(has_same_issuer)
    }

    pub fn is_local_transaction(
        &self,
        acquirer_country: types::CountryAlpha2,
    ) -> CustomResult<bool, error::ApiError> {
        logger::debug!("Validating if the transaction is local or international");

        let first_element = self
            .0
            .first()
            .ok_or(error::ApiError::UnknownError)
            .attach_printable("The filtered co-badged card info list is empty")?;

        let issuer_country = first_element.country_code;
        Ok(acquirer_country == issuer_country)
    }

    pub fn extract_networks(&self) -> Vec<gatewaydecider::types::NETWORK> {
        self.0
            .iter()
            .map(|card| card.card_network.clone())
            .collect()
    }

    pub fn get_co_badged_cards_info_response(
        &self,
    ) -> CustomResult<types::DebitRoutingOutput, error::ApiError> {
        logger::debug!("Constructing co-badged card info response");

        let first_element = self
            .0
            .first()
            .ok_or(error::ApiError::UnknownError)
            .attach_printable("The filtered co-badged card info list is empty")?;

        Ok(types::DebitRoutingOutput {
            co_badged_card_networks: self.extract_networks(),
            issuer_country: first_element.country_code,
            is_regulated: first_element.regulated,
            regulated_name: first_element.regulated_name.clone(),
            card_type: first_element.card_type,
        })
    }
}

pub async fn get_co_badged_cards_info(
    app_state: &crate::app::TenantAppState,
    card_isin: String,
    acquirer_country: types::CountryAlpha2,
) -> CustomResult<Option<types::DebitRoutingOutput>, error::ApiError> {
    // pad the card number to 19 digits to match the co-badged card bin length
    let card_number_str = CoBadgedCardInfoList::pad_card_number_to_19_digit(card_isin);

    let parsed_number: i64 = card_number_str
        .parse::<i64>()
        .change_context(error::ApiError::UnknownError)
        .attach_printable(
            "Failed to convert card number to integer in co-badged cards info flow",
        )?;

    let co_badged_card_infos_record =
        co_badged_card_info::find_co_badged_cards_info_by_card_bin(app_state, parsed_number).await;

    let filtered_co_badged_card_info_list_optional = match co_badged_card_infos_record {
        Err(error) => {
            logger::error!(
                "Error while fetching co-badged card info record: {:?}",
                error
            );

            // We need to handle db not found error here
            Err(error::ApiError::UnknownError)
                .attach_printable("error while fetching co-badged card info record")
        }
        Ok(co_badged_card_infos) => {
            logger::debug!("co-badged card info record retrieved successfully");
            let co_badged_card_infos_list = CoBadgedCardInfoList(co_badged_card_infos);

            let filtered_list_optional = co_badged_card_infos_list
                .is_valid_length()
                .then(|| co_badged_card_infos_list.filter_cards())
                .and_then(|filtered_co_badged_card_infos_list| {
                    filtered_co_badged_card_infos_list
                        .is_valid_length()
                        .then_some(filtered_co_badged_card_infos_list)
                });

            filtered_list_optional
                .and_then(|filtered_list| {
                    filtered_list
                        .is_local_transaction(acquirer_country)
                        .change_context(error::ApiError::UnknownError)
                        .attach_printable(
                            "Failed to check if the transaction is local or international",
                        )
                        .map(|is_local_transaction| is_local_transaction.then_some(filtered_list))
                        .transpose()
                })
                .transpose()
        }
    }?;

    let co_badged_cards_info_response = filtered_co_badged_card_info_list_optional
        .map(|filtered_co_badged_card_info_lis| {
            filtered_co_badged_card_info_lis.get_co_badged_cards_info_response()
        })
        .transpose()
        .attach_printable("Failed to construct co-badged card info response")?;

    Ok(co_badged_cards_info_response)
}
