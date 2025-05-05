use error_stack::ResultExt;

use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::{
    types as gateway_decider_types, utils as gateway_decider_utils,
};

use crate::decider::network_decider::{helpers, types};
use crate::logger;

pub async fn perform_debit_routing(
    decider_request: gateway_decider_types::DomainDeciderRequestForApiCallV2,
) -> Result<gateway_decider_types::DecidedGateway, gateway_decider_types::ErrorResponse> {
    let app_state = get_tenant_app_state().await;
    let card_isin_optional = decider_request.paymentInfo.cardIsin;
    let amount = decider_request.paymentInfo.amount;
    let first_connector_from_request = decider_request
        .eligibleGatewayList
        .as_ref()
        .and_then(|connector| connector.first().cloned());

    if let Some(metadata_value) = decider_request
        .paymentInfo
        .metadata
        .map(|metadata_string| gateway_decider_utils::parse_json_from_string(&metadata_string))
        .flatten()
    {
        logger::debug!("Parsed debit routing metadata to json");
        match TryInto::<types::CoBadgedCardRequest>::try_into(metadata_value) {
            Ok(co_badged_card_request) => {
                logger::debug!("Parsed debit routing metadata to co_badged_card_request");
                if let Some(debit_routing_output) = co_badged_card_request
                    .sorted_networks_by_fee(&app_state, card_isin_optional, amount)
                    .await
                {
                    return Ok(gateway_decider_types::DecidedGateway {
                        // This field should not be consumed when the request is made to /decide-gateway with the rankingAlgorithm set to NTW_BASED_ROUTING.
                        decided_gateway: first_connector_from_request.unwrap_or("".to_string()),
                        gateway_priority_map: None,
                        filter_wise_gateways: None,
                        priority_logic_tag: None,
                        routing_approach: gateway_decider_types::GatewayDeciderApproach::NONE,
                        gateway_before_evaluation: None,
                        priority_logic_output: None,
                        debit_routing_output: Some(debit_routing_output),
                        reset_approach: gateway_decider_types::ResetApproach::NO_RESET,
                        routing_dimension: None,
                        routing_dimension_level: None,
                        is_scheduled_outage: false,
                        is_dynamic_mga_enabled: false,
                        gateway_mga_id_map: None,
                    });
                }
            }
            Err(error) => {
                logger::error!("Failed to parse debit routing metadata: {:?}", error);
            }
        }
    }

    Err(helpers::return_debit_routing_application_error())
}
