use error_stack::ResultExt as _;

use crate::decider::gatewaydecider::types as gateway_decider_types;
use crate::decider::network_decider::types;
use crate::error;
use crate::utils::CustomResult;

impl types::DebitRoutingConfig {
    pub fn get_non_regulated_interchange_fee(
        &self,
        merchant_category_code: &str,
        network: &gateway_decider_types::NETWORK,
    ) -> CustomResult<&types::NetworkProcessingData, error::ApiError> {
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
