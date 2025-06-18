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

        if network_costs.is_empty() {
            logger::debug!("No network costs found or calculated, returning None.");
            return None;
        }

        logger::debug!("Total fees per debit network: {:?}", network_costs);
        network_costs.sort_by(|(_, fee1), (_, fee2)| fee1.total_cmp(fee2));

        let network_saving_infos = Self::calculate_network_saving_infos(network_costs, amount);

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

    // Helper function to calculate per-network savings
    fn calculate_network_saving_infos(
        // Takes sorted network_costs by value as it will be consumed by into_iter
        sorted_network_costs: Vec<(gateway_decider_types::NETWORK, f64)>,
        transaction_amount: f64,
    ) -> Vec<types::NetworkSavingInfo> {
        // This logic assumes sorted_network_costs is not empty,
        // as the calling function (sorted_networks_by_fee) checks for emptiness.
        let first_chosen_network_is_global = sorted_network_costs[0].0.is_global_network();

        if first_chosen_network_is_global {
            // If the first chosen (cheapest) network is global, all savings are 0.
            sorted_network_costs
                .into_iter()
                .map(|(network, _fee)| types::NetworkSavingInfo {
                    network,
                    saving_percentage: 0.0,
                })
                .collect()
        } else {
            // First network is local. Find the global network for comparison.
            let global_network_fee_opt: Option<f64> = sorted_network_costs
                .iter()
                .find(|(network, _)| network.is_global_network())
                .map(|(_, fee)| *fee);

            if let Some(baseline_fee_for_comparison) = global_network_fee_opt {
                // A global network exists for comparison
                sorted_network_costs
                    .into_iter()
                    .map(|(network, fee)| {
                        let saving = baseline_fee_for_comparison - fee;
                        let mut current_saving_percentage = 0.0;
                        // Only calculate positive savings; if local is more expensive than global, saving is 0 or negative.
                        // The problem implies savings are benefits, so negative savings are treated as 0.
                        if saving > 0.0 && transaction_amount > 0.0 {
                            current_saving_percentage = (saving / transaction_amount) * 100.0;
                        }
                        let rounded_percentage =
                            (current_saving_percentage * 100.0).round() / 100.0;
                        types::NetworkSavingInfo {
                            network,
                            saving_percentage: rounded_percentage,
                        }
                    })
                    .collect()
            } else {
                // No global network found for comparison, so all savings are 0.
                sorted_network_costs
                    .into_iter()
                    .map(|(network, _fee)| types::NetworkSavingInfo {
                        network,
                        saving_percentage: 0.0,
                    })
                    .collect()
            }
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
