use crate::app;
use crate::{
    types::pagos,
    decider::{gatewaydecider, network_decider::types, storage::utils::co_badged_card_info},
    error, logger, pagos_client,
    utils::CustomResult,
};
use error_stack::ResultExt;

fn get_parsed_bin_range_from_pagos(
    pagos_card_details: &pagos::PagosCardDetails,
    additional_brand_info: Option<&pagos::PagosAdditionalCardBrand>,
    card_brand_to_parse: &str,
) -> CustomResult<(i64, i64), error::ApiError> {
    let (bin_min_str_opt, bin_max_str_opt) = if let Some(brand_info_ref) = additional_brand_info {
        (
            brand_info_ref.bin_min.as_ref(),
            brand_info_ref.bin_max.as_ref(),
        )
    } else {
        (
            pagos_card_details.bin_min.as_ref(),
            pagos_card_details.bin_max.as_ref(),
        )
    };

    let final_bin_min: i64 = bin_min_str_opt
        .ok_or_else(|| error::ApiError::ParsingError("Missing bin_min from Pagos response"))
        .attach_printable_lazy(|| {
            format!("Missing bin_min for card_brand: {}", card_brand_to_parse)
        })?
        .parse::<i64>()
        .change_context(error::ApiError::ParsingError(
            "Failed to parse bin_min from Pagos response to i64",
        ))
        .attach_printable_lazy(|| {
            format!(
                "Failed to parse bin_min for card_brand: {}",
                card_brand_to_parse
            )
        })?;

    let final_bin_max: i64 = bin_max_str_opt
        .ok_or_else(|| error::ApiError::ParsingError("Missing bin_max from Pagos response"))
        .attach_printable_lazy(|| {
            format!("Missing bin_max for card_brand: {}", card_brand_to_parse)
        })?
        .parse::<i64>()
        .change_context(error::ApiError::ParsingError(
            "Failed to parse bin_max from Pagos response to i64",
        ))
        .attach_printable_lazy(|| {
            format!(
                "Failed to parse bin_max for card_brand: {}",
                card_brand_to_parse
            )
        })?;
    Ok((final_bin_min, final_bin_max))
}

fn try_convert_pagos_card_to_domain_data(
    pagos_card_details: &pagos::PagosCardDetails,
    additional_brand_info: Option<&pagos::PagosAdditionalCardBrand>,
    card_brand_to_parse: &str,
) -> CustomResult<types::CoBadgedCardInfoDomainData, error::ApiError> {
    let parsed_network = card_brand_to_parse
        .parse::<gatewaydecider::types::NETWORK>()
        .change_context(error::ApiError::ParsingError("NETWORK"))
        .attach_printable_lazy(|| {
            format!(
                "Failed to parse card_brand from Pagos: {}",
                card_brand_to_parse
            )
        })?;

    let card_brand_is_additional = additional_brand_info.is_some();

    let (final_bin_min, final_bin_max) = get_parsed_bin_range_from_pagos(
        pagos_card_details,
        additional_brand_info,
        card_brand_to_parse,
    )?;

    let pan_or_token = pagos_card_details
        .pan_or_token
        .clone()
        .ok_or_else(|| error::ApiError::ParsingError("Missing pan_or_token from Pagos response"))?;

    Ok(types::CoBadgedCardInfoDomainData {
        card_bin_min: final_bin_min,
        card_bin_max: final_bin_max,
        issuing_bank_name: pagos_card_details
            .bank
            .as_ref()
            .and_then(|bank_details| bank_details.name.clone()),
        card_network: parsed_network,
        country_code: pagos_card_details
            .country
            .as_ref()
            .and_then(|country_details| country_details.alpha2.clone()),
        card_type: pagos_card_details
            .card_type
            .as_ref()
            .and_then(|pagos_card_type| pagos_card_type.to_domain_card_type()),
        regulated: pagos_card_details.cost.as_ref().and_then(|cost| {
            cost.interchange
                .as_ref()
                .and_then(|interchange_details| interchange_details.regulated)
        }),
        regulated_name: pagos_card_details.cost.as_ref().and_then(|cost| {
            cost.interchange
                .as_ref()
                .and_then(|interchange_details| interchange_details.regulated_name.clone())
        }),
        prepaid: pagos_card_details.prepaid,
        reloadable: pagos_card_details.reloadable,
        pan_or_token,
        // Card bin length will be present for a successful Pagos response
        // If the Pagos response does not provide bin_length, default to 0
        // Setting bin_length to 0 is safe as it will not be used in the domain logic
        card_bin_length: pagos_card_details.bin_length.unwrap_or(0),

        // Bin provider bin length will be present for a successful Pagos response
        // If the Pagos response does not provide pagos_bin_length, default to 0
        // Setting pagos_bin_length to 0 is safe as it will not be used in the domain logic
        bin_provider_bin_length: pagos_card_details.pagos_bin_length.unwrap_or(0),

        card_brand_is_additional,
        domestic_only: pagos_card_details.domestic_only,
    })
}

async fn fetch_co_badged_info_from_db(
    app_state: &app::TenantAppState,
    parsed_card_number: i64,
) -> CustomResult<Vec<types::CoBadgedCardInfoDomainData>, error::ApiError> {
    let co_badged_card_infos_record =
        co_badged_card_info::find_co_badged_cards_info_by_card_bin(app_state, parsed_card_number)
            .await;

    match co_badged_card_infos_record {
        Err(error) => {
            logger::error!(
                "Error while fetching co-badged card info record from DB: {:?}",
                error
            );
            Err(error::ApiError::UnknownError)
                .attach_printable("Error while fetching co-badged card info record from DB")
        }
        Ok(co_badged_card_infos) => {
            logger::debug!("Co-badged card info record retrieved successfully from DB");

            let parsed_cards: Vec<types::CoBadgedCardInfoDomainData> = co_badged_card_infos
                .into_iter()
                .filter_map(|raw_co_badged_card_info| {
                    match raw_co_badged_card_info.clone().try_into() {
                        Ok(parsed) => Some(parsed),
                        Err(error) => {
                            logger::warn!(
                                "Skipping co-badged card from DB with card_network = {:?} due to error: {}",
                                raw_co_badged_card_info.card_network,
                                error
                            );
                            None
                        }
                    }
                })
                .collect();
            Ok(parsed_cards)
        }
    }
}

async fn fetch_co_badged_info_from_pagos_api(
    app_state: &app::TenantAppState,
    card_isin: &str,
) -> CustomResult<Vec<types::CoBadgedCardInfoDomainData>, error::ApiError> {
    match pagos_client::fetch_pan_details_internal(app_state, card_isin).await {
        Ok(pagos_response) => {
            logger::debug!(
                ?pagos_response,
                "Pagos PAN details fetched successfully internally"
            );
            let mut domain_data_list = Vec::new();

            if let Some(primary_card_brand_str) = &pagos_response.card.card_brand {
                match try_convert_pagos_card_to_domain_data(
                    &pagos_response.card,
                    None,
                    primary_card_brand_str,
                ) {
                    Ok(primary_data) => domain_data_list.push(primary_data),
                    Err(error) => {
                        logger::error!("Error converting primary Pagos card details: {:?}", error);
                        return Err(error);
                    }
                }
            }

            if let Some(additional_brands) = &pagos_response.card.additional_card_brands {
                for brand_info in additional_brands {
                    if let Some(additional_card_brand_str) = &brand_info.card_brand {
                        match try_convert_pagos_card_to_domain_data(
                            &pagos_response.card,
                            Some(brand_info),
                            additional_card_brand_str,
                        ) {
                            Ok(additional_data) => domain_data_list.push(additional_data),
                            Err(error) => {
                                logger::error!("Error converting additional Pagos card details for brand {}: {:?}", additional_card_brand_str, error);
                                return Err(error);
                            }
                        }
                    }
                }
            }
            Ok(domain_data_list)
        }
        Err(error) => Err(error),
    }
}

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
) -> CustomResult<Option<types::CoBadgedCardInfoResponse>, error::ApiError> {
    let use_api_lookup = app_state
        .config
        .pagos_api
        .as_ref()
        .map_or(false, |pc| pc.use_api_for_co_badged_lookup);

    let co_badged_cards_data_result = if use_api_lookup {
        logger::debug!("Fetching co-badged card info from Pagos API");
        fetch_co_badged_info_from_pagos_api(app_state, &card_isin).await
    } else {
        logger::debug!("Fetching co-badged card info from DB");
        let card_number_str = CoBadgedCardInfoList::pad_card_number_to_19_digit(card_isin.clone());
        let parsed_number: i64 = card_number_str
            .parse::<i64>()
            .change_context(error::ApiError::UnknownError)
            .attach_printable(
                "Failed to convert card number to integer in co-badged cards info flow (DB path)",
            )?;
        fetch_co_badged_info_from_db(app_state, parsed_number).await
    };

    let co_badged_cards_data = match co_badged_cards_data_result {
        Ok(data) => data,
        Err(error) => {
            logger::error!("Failed to fetch co-badged card info: {:?}", error);
            return Err(error);
        }
    };

    if co_badged_cards_data.is_empty() {
        logger::debug!("No co-badged card data found from the selected source.");
        return Ok(None);
    }

    let co_badged_card_infos_list = CoBadgedCardInfoList(co_badged_cards_data);

    let filtered_list_optional = co_badged_card_infos_list
        .is_valid_length()
        .then(|| {
            co_badged_card_infos_list
                .is_only_one_global_network_present()
                .then_some(co_badged_card_infos_list.filter_cards())
        })
        .flatten()
        .and_then(|filtered_list| filtered_list.is_valid_length().then_some(filtered_list));

    let co_badged_cards_info_response = filtered_list_optional
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
