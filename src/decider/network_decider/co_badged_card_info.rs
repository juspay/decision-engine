use error_stack::ResultExt;

use crate::{
    decider::{
        gatewaydecider, network_decider::types,
        storage::utils::co_badged_card_info::find_co_badged_cards_info_by_card_bin,
    },
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
    ) -> CustomResult<types::CoBadgedCardInfoResponse, error::ApiError> {
        logger::debug!("Constructing co-badged card info response");

        let first_element = self
            .0
            .first()
            .ok_or(error::ApiError::UnknownError)
            .attach_printable("The filtered co-badged card info list is empty")?;

        Ok(types::CoBadgedCardInfoResponse {
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
        find_co_badged_cards_info_by_card_bin(app_state, parsed_number).await;

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

pub fn calculate_interchange_fee(
    network: &gatewaydecider::types::NETWORK,
    is_regulated: &bool,
    regulated_name_optional: Option<&types::RegulatedName>,
    amount: f64,
    debit_routing: &types::DebitRoutingConfig,
) -> CustomResult<f64, error::ApiError> {
    logger::debug!("Calculating interchange fee");
    let fee_data = if *is_regulated {
        logger::debug!("Regulated bank");
        &debit_routing.interchange_fee.regulated
    } else {
        logger::debug!("Non regulated bank");
        debit_routing
            .interchange_fee
            .non_regulated
            .0
            .get("merchant_category_code_0001")
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for merchant category code",
            ))?
            .get(network)
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for non regulated",
            ))
            .attach_printable(
                "Failed to fetch interchange fee for non regulated banks in debit routing",
            )?
    };

    let percentage = fee_data.percentage;

    let fixed_amount = fee_data.fixed_amount;

    let mut total_interchange_fee = (amount * percentage / 100.0) + fixed_amount;

    if *is_regulated {
        if let Some(regulated_name) = regulated_name_optional {
            match regulated_name {
                types::RegulatedName::ExemptFraud => {
                    logger::debug!("Regulated bank with exemption for fraud");
                }
                types::RegulatedName::NonExemptWithFraud => {
                    logger::debug!("Regulated bank with non exemption for fraud");
                    let fraud_check_fee = debit_routing.fraud_check_fee;

                    total_interchange_fee += fraud_check_fee
                }
            };
        }
    };

    Ok(total_interchange_fee)
}

pub fn calculate_network_fee(
    network: &gatewaydecider::types::NETWORK,
    amount: f64,
    debit_routing: &types::DebitRoutingConfig,
) -> CustomResult<f64, error::ApiError> {
    logger::debug!("Calculating network fee");
    let fee_data = debit_routing
        .network_fee
        .get(network)
        .ok_or(error::ApiError::MissingRequiredField(
            "interchange fee for non regulated",
        ))
        .attach_printable(
            "Failed to fetch interchange fee for non regulated banks in debit routing",
        )?;
    let percentage = fee_data.percentage;
    let fixed_amount = fee_data.fixed_amount;
    let total_network_fee = (amount * percentage / 100.0) + fixed_amount;
    Ok(total_network_fee)
}

pub fn calculate_total_fees_per_network(
    app_state: &crate::app::TenantAppState,
    co_badged_cards_info: types::CoBadgedCardInfoResponse,
    amount: f64,
) -> CustomResult<Option<Vec<(gatewaydecider::types::NETWORK, f64)>>, error::ApiError> {
    logger::debug!("Calculating total fees per network");
    let routing_config = &app_state
        .config
        .routing_config
        .clone()
        .ok_or(error::ApiError::UnknownError)
        .attach_printable("Missing routing config for debit routing")?;

    let debit_routing_config = &routing_config.debit_routing_config;

    co_badged_cards_info
        .co_badged_card_networks
        .into_iter()
        .map(|network| {
            let interchange_fee = calculate_interchange_fee(
                &network,
                &co_badged_cards_info.is_regulated,
                co_badged_cards_info.regulated_name.as_ref(),
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

pub fn sort_networks_by_fee(
    network_fees: Vec<(gatewaydecider::types::NETWORK, f64)>,
) -> Vec<gatewaydecider::types::NETWORK> {
    logger::debug!("Sorting networks by fee");
    let mut sorted_fees = network_fees;
    sorted_fees.sort_by(|(_network1, fee1), (_network2, fee2)| fee1.total_cmp(fee2));

    sorted_fees
        .into_iter()
        .map(|(network, _fee)| network)
        .collect()
}
