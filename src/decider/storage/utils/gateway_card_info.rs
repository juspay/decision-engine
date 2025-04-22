// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal::*;
use crate::storage::types::{BitBool, GatewayCardInfo as DBGatewayCardInfo, MerchantGatewayCardInfo as DBMerchantGatewayCardInfo};
use crate::types::gateway_card_info::GatewayCardInfo;
// use types::utils::dbconfig::get_euler_db_conf;
use crate::types::merchant::id::merchant_pid_to_text;
// use juspay::extra::parsing::{Parsed, Step, around, lift_pure, mandated, parse_field, project};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
use crate::types::merchant::merchant_account::MerchantAccount;

use crate::storage::schema::gateway_card_info::dsl;
use crate::storage::schema::merchant_gateway_card_info::dsl as m_dsl;
use diesel::associations::HasTable;
use diesel::*;
use std::clone::Clone;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
//use crate::errors::{ApiError, UnifiedError}; // Import the ApiError and UnifiedError types

use crate::{error::{ApiError}};
use crate::{decider::gatewaydecider::{
    types::{ ErrorResponse, UnifiedError},
}, logger};

pub async fn getSupportedGatewayCardInfoForBins(
    app_state: &crate::app::TenantAppState,
    input_merchant_account: MerchantAccount,
    card_bins: Vec<Option<String>>,
) -> Result<Vec<GatewayCardInfo>, ErrorResponse> {
    // Step 1: Query GatewayCardInfo with diesel
    let gci_records: Vec<DBGatewayCardInfo> = match crate::generics::generic_find_all::<
        <DBGatewayCardInfo as HasTable>::Table,
        _,
        DBGatewayCardInfo,
    >(
        &app_state.db,
        dsl::isin.eq_any(card_bins.clone())
            .and(dsl::disabled.eq(Some(BitBool(false)))),
    ).await {
        Ok(records) => records,
        Err(_) => Vec::new(),
    };

    let gcis: Vec<i64> = gci_records.iter().map(|r| r.id).collect();
    if gcis.is_empty() {
        return Ok(Vec::new());
    }

    // Step 2: Query MerchantGatewayCardInfo using diesel
    let mgci_records: Vec<DBMerchantGatewayCardInfo> = match crate::generics::generic_find_all::<
        <DBMerchantGatewayCardInfo as HasTable>::Table,
        _,
        DBMerchantGatewayCardInfo,
    >(
        &app_state.db,
        m_dsl::merchant_account_id.eq(merchant_pid_to_text(input_merchant_account.id))
            .and(m_dsl::disabled.eq(BitBool(false)))
            .and(m_dsl::gateway_card_info_id.eq_any(gcis)),
    )
    .await
    {
        Ok(records) => records,
        Err(_) => Vec::new(),
    };

    // Step 3: Filter GatewayCardInfo records
    let gcis_filtered: Vec<i64> = mgci_records
        .iter()
        .map(|r| r.gateway_card_info_id)
        .collect();
    let gci_records_filtered: Vec<DBGatewayCardInfo> = gci_records
        .into_iter()
        .filter(|gci| gcis_filtered.contains(&gci.id.clone()))
        .collect();

    // Step 4: Convert using TryFrom and handle errors

    let gci_records: Result<Vec<GatewayCardInfo>, ApiError> = gci_records_filtered
        .into_iter()
        .map(GatewayCardInfo::try_from)
        .collect();
    
    let parsed_gci_records = match gci_records {
        Ok(records) => records,
        Err(err) => {
            logger::error!( "parseGatewayCardInfo: {:?}", err);
            return Err(ErrorResponse {
                status: "500".to_string(),
                error_code: "INTERNAL_SERVER_ERROR".to_string(),
                error_message: "Internal Server Error".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "INTERNAL_SERVER_ERROR".to_string(),
                    user_message: "Internal Server Error.".to_string(),
                    developer_message: "record parsing failed.".to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            });
        }
    };
    Ok(parsed_gci_records)
    
}
