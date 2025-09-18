use crate::decider::gatewaydecider::types as gateway_decider_types;
use crate::decider::network_decider::{co_badged_card_info, types};
use crate::utils::CustomResult;
use crate::{app, error, logger};
use error_stack::ResultExt as _;

impl types::DebitRoutingConfig {
    pub fn get_non_regulated_interchange_fee(
        &self,
        merchant_category_code: &types::MerchantCategoryCode,
        network: &gateway_decider_types::NETWORK,
    ) -> CustomResult<&types::NetworkProcessingData, error::ApiError> {
        logger::debug!(
            "Fetching interchange fee for non regulated banks in debit routing {:?}",
            merchant_category_code
        );
        self.interchange_fee
            .non_regulated
            .0
            .get(merchant_category_code)
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for merchant category code",
            ))?
            .get(network)
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for non regulated",
            ))
            .attach_printable(
                "Failed to fetch interchange fee for non regulated banks in debit routing",
            )
    }

    pub fn get_network_fee(
        &self,
        network: &gateway_decider_types::NETWORK,
    ) -> CustomResult<&types::NetworkProcessingData, error::ApiError> {
        Ok(self
            .network_fee
            .get(network)
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for non regulated",
            ))
            .attach_printable(
                "Failed to fetch interchange fee for non regulated banks in debit routing",
            )?)
    }
}

pub fn return_debit_routing_application_error() -> gateway_decider_types::ErrorResponse {
    gateway_decider_types::ErrorResponse {
        status: "Invalid Request".to_string(),
        error_code: "invalid_request_error".to_string(),
        error_message: "Can't find the co-badged card network".to_string(),
        priority_logic_tag: None,
        routing_approach: None,
        filter_wise_gateways: None,
        error_info: gateway_decider_types::UnifiedError {
            code: "CO_BADGED_NETWORK_NOT_FOUND".to_string(),
            user_message: "Co-badged card network not found to process the transaction request."
                .to_string(),
            developer_message:
                "Co-badged card network not found to process the transaction request.".to_string(),
        },
        priority_logic_output: None,
        is_dynamic_mga_enabled: false,
    }
}

impl types::CoBadgedCardRequest {
    pub async fn sorted_networks_by_fee(
        self,
        app_state: &app::TenantAppState,
        card_isin_optional: Option<String>,
        amount: f64,
    ) -> Option<types::DebitRoutingOutput> {
        logger::debug!("Fetching sorted card networks based on their respective network fees");

        let co_badged_card_info = self
            .fetch_co_badged_card_info(app_state, card_isin_optional)
            .await?;

        let mut network_costs = self
            .calculate_network_fees(app_state, &co_badged_card_info, amount)
            .await?;

        logger::debug!("Total fees per debit network: {:?}", network_costs);
        network_costs.sort_by(|(_, fee1), (_, fee2)| fee1.total_cmp(fee2));

        let network_saving_infos = Self::calculate_network_saving_infos(network_costs, amount)?;

        Some(types::DebitRoutingOutput {
            co_badged_card_networks_info: network_saving_infos,
            issuer_country: co_badged_card_info.issuer_country,
            is_regulated: co_badged_card_info.is_regulated,
            regulated_name: co_badged_card_info.regulated_name,
            card_type: co_badged_card_info.card_type,
        })
    }

    pub async fn sorted_networks_by_absolute_fee(
        self,
        app_state: &app::TenantAppState,
        card_isin_optional: Option<String>,
        amount: f64,
    ) -> Option<types::DebitRoutingOutput> {
        logger::debug!("Fetching sorted card networks based on their respective network fees");

        let co_badged_card_info = self
            .fetch_co_badged_card_info(app_state, card_isin_optional)
            .await?;

        let mut network_costs = self
            .calculate_network_fees(app_state, &co_badged_card_info, amount)
            .await?;

        logger::debug!("Total fees per debit network: {:?}", network_costs);
        network_costs.sort_by(|(_, fee1), (_, fee2)| fee1.total_cmp(fee2));

        // Initialize network_saving_infos vector
        let mut network_saving_infos: Vec<types::NetworkSavingInfo> = Vec::new();

        // Find min and max fee values for min-max normalization
        let min_fee = network_costs.first().map(|(_, fee)| *fee).unwrap_or(0.0);
        let max_fee = network_costs.last().map(|(_, fee)| *fee).unwrap_or(0.0);

        for (network, fee) in &network_costs {
            // Apply standard min-max normalization
            let normalized_value = if max_fee > min_fee {
                (*fee - min_fee) / (max_fee - min_fee)
            } else {
                0.0
            };

            network_saving_infos.push(types::NetworkSavingInfo {
                network: network.clone(),
                saving_percentage: normalized_value,
            });
        }

        Some(types::DebitRoutingOutput {
            co_badged_card_networks_info: network_saving_infos,
            issuer_country: co_badged_card_info.issuer_country,
            is_regulated: co_badged_card_info.is_regulated,
            regulated_name: co_badged_card_info.regulated_name,
            card_type: co_badged_card_info.card_type,
        })
    }

    async fn fetch_co_badged_card_info(
        &self,
        app_state: &app::TenantAppState,
        card_isin_optional: Option<String>,
    ) -> Option<types::CoBadgedCardInfoResponse> {
        if let Some(co_badged_card_data) = self.co_badged_card_data.clone() {
            logger::debug!("Co-badged card data found in request");
            return Some(co_badged_card_data.into());
        }

        let card_isin = card_isin_optional?;
        co_badged_card_info::get_co_badged_cards_info(app_state, card_isin)
            .await
            .map_err(|error| {
                logger::warn!(?error, "Failed to fetch co-badged card info");
            })
            .ok()
            .flatten()
    }

    async fn calculate_network_fees(
        &self,
        app_state: &app::TenantAppState,
        co_badged_card_info: &types::CoBadgedCardInfoResponse,
        amount: f64,
    ) -> Option<Vec<(gateway_decider_types::NETWORK, f64)>> {
        co_badged_card_info::calculate_total_fees_per_network(
            app_state,
            co_badged_card_info,
            &self.merchant_category_code,
            amount,
        )
        .map_err(|error| {
            logger::warn!(?error, "Failed to calculate total fees per network");
        })
        .ok()
        .flatten()
    }

    fn calculate_savings<F>(
        costs: Vec<(gateway_decider_types::NETWORK, f64)>,
        calc_savings_percentage: F,
    ) -> Vec<types::NetworkSavingInfo>
    where
        F: Fn(f64) -> f64,
    {
        costs
            .into_iter()
            .map(|(network, fee)| types::NetworkSavingInfo {
                network,
                saving_percentage: calc_savings_percentage(fee),
            })
            .collect()
    }

    fn calculate_network_saving_infos(
        sorted_network_costs: Vec<(gateway_decider_types::NETWORK, f64)>,
        transaction_amount: f64,
    ) -> Option<Vec<types::NetworkSavingInfo>> {
        let zero_savings_fn = |_fee: f64| 0.0;
        let Some((first_network, _)) = sorted_network_costs.first() else {
            logger::debug!("No network costs found, returning empty vector.");
            return None;
        };

        if first_network.is_global_network() {
            return Some(Self::calculate_savings(
                sorted_network_costs,
                zero_savings_fn,
            ));
        };

        let baseline_fee_optional = sorted_network_costs
            .iter()
            .find(|(network, _)| network.is_global_network())
            .map(|(_, fee)| *fee);

        if let Some(baseline_fee) = baseline_fee_optional {
            let calc_savings_fn = |fee: f64| {
                let saving = baseline_fee - fee;
                if saving > 0.0 && transaction_amount > 0.0 {
                    ((saving / transaction_amount) * 10000.0).round() / 100.0
                } else {
                    0.0
                }
            };
            Some(Self::calculate_savings(
                sorted_network_costs,
                calc_savings_fn,
            ))
        } else {
            Some(Self::calculate_savings(
                sorted_network_costs,
                zero_savings_fn,
            ))
        }
    }
}

impl gateway_decider_types::NETWORK {
    pub fn is_global_network(&self) -> bool {
        match self {
            gateway_decider_types::NETWORK::VISA
            | gateway_decider_types::NETWORK::AMEX
            | gateway_decider_types::NETWORK::DINERS
            | gateway_decider_types::NETWORK::RUPAY
            | gateway_decider_types::NETWORK::MASTERCARD => true,
            gateway_decider_types::NETWORK::STAR
            | gateway_decider_types::NETWORK::PULSE
            | gateway_decider_types::NETWORK::ACCEL
            | gateway_decider_types::NETWORK::NYCE => false,
        }
    }
}
