use crate::decider::gatewaydecider::{self, types as gateway_decider_types};
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

        let networks = self
            .calculate_network_fees(app_state, &co_badged_card_info, amount)
            .await?;

        logger::debug!("Total fees per debit network: {:?}", networks);

        let sorted_networks = sort_networks(networks.clone());

        let saving_percentage =
            calculate_and_round_saving_percentage(&sorted_networks, &networks, amount);

        Some(types::DebitRoutingOutput {
            co_badged_card_networks: sorted_networks.clone(),
            issuer_country: co_badged_card_info.issuer_country,
            is_regulated: co_badged_card_info.is_regulated,
            regulated_name: co_badged_card_info.regulated_name,
            card_type: co_badged_card_info.card_type.clone(),
            saving_percentage,
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
}

pub fn sort_networks(
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

// Helper function to calculate and round the saving percentage
fn calculate_and_round_saving_percentage(
    sorted_network_types: &[gateway_decider_types::NETWORK],
    network_costs: &[(gateway_decider_types::NETWORK, f64)],
    transaction_amount: f64,
) -> f64 {
    let mut saving_percentage_value: f64 = 0.0;

    if !sorted_network_types.is_empty() {
        let first_chosen_network = sorted_network_types[0].clone();

        if first_chosen_network.is_global_network() {
            // If the first network is already global, savings are 0.
            // saving_percentage_value is already 0.0
        } else {
            // The first network is not global, try to find a global one for comparison.
            let cost_first_opt = network_costs
                .iter()
                .find(|(n, _)| *n == first_chosen_network)
                .map(|(_, cost)| *cost);

            let global_network_for_comparison_opt = sorted_network_types
                .iter()
                .find(|n_type| n_type.is_global_network());

            if let (Some(cost_first), Some(global_network_type)) =
                (cost_first_opt, global_network_for_comparison_opt)
            {
                let cost_global_for_comparison_opt = network_costs
                    .iter()
                    .find(|(n, _)| *n == *global_network_type)
                    .map(|(_, cost)| *cost);

                if let Some(cost_global) = cost_global_for_comparison_opt {
                    let difference = cost_global - cost_first;
                    if transaction_amount > 0.0 {
                        let raw_percentage = (difference / transaction_amount) * 100.0;

                        // Round to 2 decimal places
                        saving_percentage_value = (raw_percentage * 100.0).round() / 100.0;
                    }
                }
            }
        }
    }

    // sorted_network_types is empty, percentage remains 0.0
    saving_percentage_value
}

impl gateway_decider_types::NETWORK {
    pub fn is_global_network(&self) -> bool {
        match self {
            gateway_decider_types::NETWORK::VISA
            | gateway_decider_types::NETWORK::AMEX
            | gateway_decider_types::NETWORK::DINERS
            | gateway_decider_types::NETWORK::RUPAY
            | gateway_decider_types::NETWORK::MASTERCARD
            | gateway_decider_types::NETWORK::DISCOVER => true,
            gateway_decider_types::NETWORK::STAR
            | gateway_decider_types::NETWORK::PULSE
            | gateway_decider_types::NETWORK::ACCEL
            | gateway_decider_types::NETWORK::NYCE => false,
        }
    }
}
