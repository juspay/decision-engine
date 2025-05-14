use crate::app;
use crate::{
    decider::{gatewaydecider, network_decider::types, storage::utils::co_badged_card_info},
    error, logger,
    utils::CustomResult,
};
use error_stack::ResultExt;

pub struct CoBadgedCardInfoList(Vec<types::CoBadgedCardInfoDomainData>);

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

    pub fn is_only_one_global_network_present(&self) -> bool {
        let global_network_count = self
            .0
            .iter()
            .filter(|card| card.card_network.is_global_network())
            .count();

        if global_network_count == 1 {
            logger::debug!("Exactly one global network present");
            true
        } else {
            logger::debug!(
                "Invalid number of global networks: expected 1, found {}",
                global_network_count
            );
            false
        }
    }

    pub fn get_global_network_card(
        &self,
    ) -> CustomResult<&types::CoBadgedCardInfoDomainData, error::ApiError> {
        self.0
            .iter()
            .find(|card| card.card_network.is_global_network())
            .ok_or(error::ApiError::UnknownError)
            .attach_printable("Missing global network card in co-badged card info")
    }

    pub fn filter_cards(self) -> Self {
        logger::debug!(
            "Filtering co-badged cards, Total cards before filtering: {}",
            self.0.len()
        );

        let filtered_cards: Vec<types::CoBadgedCardInfoDomainData> = self
            .0
            .into_iter()
            .filter(|card| {
                card.card_type == Some(types::CardType::Debit)
                    && card.pan_or_token == types::PanOrToken::Pan
                    && card.prepaid == Some(false)
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
        acquirer_country: &types::CountryAlpha2,
    ) -> CustomResult<bool, error::ApiError> {
        logger::debug!("Validating if the transaction is local or international");

        let global_card = self.get_global_network_card()?;

        let issuer_country = global_card.get_country_code()?;

        Ok(*acquirer_country == issuer_country)
    }

    pub fn extract_networks(&self) -> Vec<gatewaydecider::types::NETWORK> {
        self.0
            .iter()
            .map(|card| card.card_network.clone())
            .collect()
    }

    pub fn get_co_badged_cards_info_response(
        &self,
    ) -> CustomResult<types::CoBadgedCardInfoResponse, error::ApiError> {
        logger::debug!("Constructing co-badged card info response");

        let global_card = self.get_global_network_card()?;

        Ok(types::CoBadgedCardInfoResponse {
            co_badged_card_networks: self.extract_networks(),
            issuer_country: global_card.get_country_code()?,
            is_regulated: global_card.get_is_regulated()?,
            regulated_name: global_card.regulated_name.clone(),
            card_type: global_card.get_card_type()?,
        })
    }
}

pub async fn get_co_badged_cards_info(
    app_state: &app::TenantAppState,
    card_isin: String,
    acquirer_country: &types::CountryAlpha2,
) -> CustomResult<Option<types::CoBadgedCardInfoResponse>, error::ApiError> {
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
            Err(error::ApiError::UnknownError)
                .attach_printable("Error while fetching co-badged card info record")
        }
        Ok(co_badged_card_infos) => {
            logger::debug!("Co-badged card info record retrieved successfully");

            // Parse the co-badged card info records into domain data
            let parsed_cards: Vec<types::CoBadgedCardInfoDomainData> = co_badged_card_infos
                .into_iter()
                .filter_map(|raw_co_badged_card_info| {
                    match raw_co_badged_card_info.clone().try_into() {
                        Ok(parsed) => Some(parsed),
                        Err(error) => {
                            logger::warn!(
                                "Skipping co-badged card with card_network = {:?} due to error: {}",
                                raw_co_badged_card_info.card_network,
                                error
                            );
                            None
                        }
                    }
                })
                .collect();

            let co_badged_card_infos_list = CoBadgedCardInfoList(parsed_cards);

            let filtered_list_optional = co_badged_card_infos_list
                .is_valid_length()
                .then(|| {
                    co_badged_card_infos_list
                        .is_only_one_global_network_present()
                        .then_some(co_badged_card_infos_list.filter_cards())
                })
                .flatten()
                .and_then(|filtered_list| filtered_list.is_valid_length().then_some(filtered_list));

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
        .map(|filtered_list| filtered_list.get_co_badged_cards_info_response())
        .transpose()
        .attach_printable("Failed to construct co-badged card info response")?;

    Ok(co_badged_cards_info_response)
}

pub fn calculate_interchange_fee(
    network: &gatewaydecider::types::NETWORK,
    co_badged_cards_info: &types::CoBadgedCardInfoResponse,
    merchant_category_code: &types::MerchantCategoryCode,
    amount: f64,
    debit_routing: &types::DebitRoutingConfig,
) -> CustomResult<f64, error::ApiError> {
    logger::debug!("Calculating interchange fee");
    let is_regulated = &co_badged_cards_info.is_regulated;
    let regulated_name_optional = &co_badged_cards_info.regulated_name;

    let fee_data = if *is_regulated {
        logger::debug!("Regulated bank");
        &debit_routing.interchange_fee.regulated
    } else {
        logger::debug!("Non regulated bank");
        debit_routing.get_non_regulated_interchange_fee(&merchant_category_code, network)?
    };

    let percentage = fee_data.percentage;

    let fixed_amount = fee_data.fixed_amount;

    let total_interchange_fee = (amount * percentage / 100.0) + fixed_amount;

    let total_fee = apply_fraud_check_fee_if_applicable(
        *is_regulated,
        regulated_name_optional,
        debit_routing.fraud_check_fee,
        total_interchange_fee,
    );

    Ok(total_fee)
}

pub fn apply_fraud_check_fee_if_applicable(
    is_regulated: bool,
    regulated_name_optional: &Option<types::RegulatedName>,
    fraud_check_fee: f64,
    total_interchange_fee: f64,
) -> f64 {
    if is_regulated {
        if let Some(regulated_name) = regulated_name_optional {
            match regulated_name {
                types::RegulatedName::Unknown(bank_name) => {
                    logger::debug!(
                        "Fraud check fee not applicable due to unknown regulated bank name: {}",
                        bank_name
                    );
                }
                types::RegulatedName::NonExemptWithFraud => {
                    logger::debug!("Regulated bank with non exemption for fraud");
                    return total_interchange_fee + fraud_check_fee;
                }
            }
        }
    }
    total_interchange_fee
}

pub fn calculate_network_fee(
    network: &gatewaydecider::types::NETWORK,
    amount: f64,
    debit_routing: &types::DebitRoutingConfig,
) -> CustomResult<f64, error::ApiError> {
    logger::debug!("Calculating network fee");
    let fee_data = debit_routing.get_network_fee(network)?;
    let percentage = fee_data.percentage;
    let fixed_amount = fee_data.fixed_amount;
    let total_network_fee = (amount * percentage / 100.0) + fixed_amount;
    Ok(total_network_fee)
}

pub fn calculate_total_fees_per_network(
    app_state: &crate::app::TenantAppState,
    co_badged_cards_info: &types::CoBadgedCardInfoResponse,
    merchant_category_code: &types::MerchantCategoryCode,
    amount: f64,
) -> CustomResult<Option<Vec<(gatewaydecider::types::NETWORK, f64)>>, error::ApiError> {
    logger::debug!("Calculating total fees per network");
    let debit_routing_config = &app_state.config.debit_routing_config.clone();

    co_badged_cards_info
        .co_badged_card_networks.clone()
        .into_iter()
        .map(|network| {
            let interchange_fee = calculate_interchange_fee(
                &network,
                &co_badged_cards_info,
                &merchant_category_code,
                amount,
                debit_routing_config,
            )
            .change_context(error::ApiError::UnknownError)
            .attach_printable("Failed to calculate debit routing interchange_fee")?;

            let network_fee = calculate_network_fee(&network, amount, debit_routing_config)
                .change_context(error::ApiError::UnknownError)
                .attach_printable("Failed to calculate debit routing network_fee")?;

            let total_fee = interchange_fee + network_fee;
            logger::debug!(
                "Total fee for network {} is {}",
                network.to_string(),
                total_fee
            );
            Ok(Some((network, total_fee)))
        })
        .collect::<CustomResult<Option<Vec<(gatewaydecider::types::NETWORK, f64)>>, error::ApiError>>()
}
