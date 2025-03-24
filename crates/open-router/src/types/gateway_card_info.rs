
use serde::{Serialize, Deserialize};
use crate::app::get_tenant_app_state;
use crate::storage::db;
// use db::euler_mesh_impl::mesh_config;
// use db::mesh::internal;
use crate::storage::types::GatewayCardInfo as DBGatewayCardInfo;
use crate::types::gateway::{Gateway, GatewayAny, text_to_gateway};
use crate::types::bank_code::{BankCodeId, to_bank_code_id};
// use juspay::extra::parsing::{
//     Parsed, Step, ParsingErrorType, ParsingErrorType::UnexpectedTextValue, around, lift_either,
//     lift_pure, mandated, non_negative, parse_field, project,
// };
use crate::types::payment::payment_method::{PaymentMethodType, text_to_payment_method_type};
// use eulerhs::extra::combinators::to_domain_all;
// use types::utils::dbconfig::get_euler_db_conf;
// use eulerhs::language::MonadFlow;
use serde_json::Value;
use std::string::String;
use std::vec::Vec;
use std::option::Option;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::cmp::PartialEq;
use crate::error::ApiError;

use crate::storage::schema::gateway_card_info::dsl;
use diesel::associations::HasTable;
use diesel::*;

use super::gateway::gateway_to_text;

// use super::payment::payment_method::text_to_payment_method_type;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GciPId {
    pub gciPId: i64,
}

pub fn to_gci_pid(id: i64) -> GciPId {
    GciPId {
        gciPId: id,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayCardInfo {
    #[serde(rename = "id")]
    pub id: GciPId,
    #[serde(rename = "isin")]
    pub isin: Option<String>,
    #[serde(rename = "gateway")]
    pub gateway: Option<Gateway>,
    #[serde(rename = "cardIssuerBankName")]
    pub cardIssuerBankName: Option<String>,
    #[serde(rename = "authType")]
    pub authType: Option<String>,
    #[serde(rename = "juspayBankCodeId")]
    pub juspayBankCodeId: Option<BankCodeId>,
    #[serde(rename = "disabled")]
    pub disabled: Option<bool>,
    #[serde(rename = "validationType")]
    pub validationType: Option<ValidationType>,
    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: Option<PaymentMethodType>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationType {
    #[serde(rename = "CARD_MANDATE")]
    CardMandate,
    #[serde(rename = "EMANDATE")]
    Emandate,
    #[serde(rename = "TPV")]
    Tpv,
    #[serde(rename = "TPV_EMANDATE")]
    TpvEmandate,
    #[serde(rename = "REWARD")]
    Reward,
}

pub fn text_to_validation_type(validation_type: String) -> Result<ValidationType, ApiError> {
    match validation_type.as_str() {
        "CARD_MANDATE" => Ok(ValidationType::CardMandate),
        "EMANDATE" => Ok(ValidationType::Emandate),
        "TPV" => Ok(ValidationType::Tpv),
        "TPV_EMANDATE" => Ok(ValidationType::TpvEmandate),
        "REWARD" => Ok(ValidationType::Reward),
        _ => Err(ApiError::ParsingError("Invalid Validation Type")),
    }
}

pub fn validation_type_to_text(validation_type: ValidationType) -> String {
    match validation_type {
        ValidationType::CardMandate => "CARD_MANDATE".to_string(),
        ValidationType::Emandate => "EMANDATE".to_string(),
        ValidationType::Tpv => "TPV".to_string(),
        ValidationType::TpvEmandate => "TPV_EMANDATE".to_string(),
        ValidationType::Reward => "REWARD".to_string(),
    }
}

impl TryFrom<DBGatewayCardInfo> for GatewayCardInfo {
    type Error = ApiError;

    fn try_from(db_gci: DBGatewayCardInfo) -> Result<Self, ApiError> {
        Ok(GatewayCardInfo {
            id: to_gci_pid(db_gci.id),
            isin: db_gci.isin,
            gateway: db_gci.gateway.map(|gw| text_to_gateway(gw.as_str())).transpose()?,
            cardIssuerBankName: db_gci.card_issuer_bank_name,
            authType: db_gci.auth_type,
            juspayBankCodeId: db_gci.juspay_bank_code_id.map(|id| to_bank_code_id(id)),
            disabled: db_gci.disabled,
            validationType: db_gci.validation_type.map(|validation_type| text_to_validation_type(validation_type)).transpose()?,
            paymentMethodType: db_gci.payment_method_type.map(|payment_method_type| text_to_payment_method_type(payment_method_type)).transpose()?,
        })
    }
}

pub async fn get_all_by_mgci_ids(
    
    ids: Vec<GciPId>
) -> Vec<GatewayCardInfo> {
    // Extract i64 values from GciPId objects
    let id_values: Vec<i64> = ids.into_iter().map(|id| id.gciPId).collect();
    let app_state = get_tenant_app_state().await;
    // Execute the database query using Diesel
    match crate::generics::generic_find_all::<
            <DBGatewayCardInfo as HasTable>::Table,
            _,
            DBGatewayCardInfo
        >(
            &app_state.db,
            dsl::id.eq_any(id_values)
        )
        .await
        {
            Ok(db_results) => db_results.into_iter()
                                    .filter_map(|db_record: DBGatewayCardInfo  | GatewayCardInfo::try_from(db_record).ok())
                                    .collect(),
            Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
        }
}

pub async fn get_enabled_gateway_card_info_for_gateways(
    
    card_bins: Vec<Option<String>>,
    gateways: Vec<Gateway>,
) -> Vec<GatewayCardInfo> {
    // Early return if both input lists are empty
    if card_bins.is_empty() && gateways.is_empty() {
        return Vec::new();
    }
    let app_state = get_tenant_app_state().await;

    // Convert gateways to strings
    let gateway_strings: Vec<Option<String>> = gateways.into_iter()
        .map(|g| Some(gateway_to_text(&g)))
        .collect();
    
    // Execute database query with three conditions
    match crate::generics::generic_find_all::<
            <DBGatewayCardInfo as HasTable>::Table,
            _,
            DBGatewayCardInfo
        >(
            &app_state.db,
            dsl::isin.eq_any(card_bins)
                .and(dsl::gateway.eq_any(gateway_strings))
                .and(dsl::disabled.is_null().or(dsl::disabled.ne(true))),
        )
        .await
        {
            Ok(db_results) => db_results.into_iter()
                                    .filter_map(|db_record| GatewayCardInfo::try_from(db_record).ok())
                                    .collect(),
            Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
        }
}

// #TOD implement db calls

// pub async fn get_enabled_gateway_card_info_for_gateways(
//     card_bins: Vec<Option<String>>,
//     gateways: Vec<Gateway>,
// ) -> Result<Vec<GatewayCardInfo>, Box<dyn Error>> {
//     if card_bins.is_empty() && gateways.is_empty() {
//         return Ok(Vec::new());
//     }

//     let db_conf = get_euler_db_conf::<DB::GatewayCardInfoT>().await?;
//     let db_res = find_all_rows(
//         db_conf,
//         mesh_config(),
//         vec![
//             DB::Clause::Is(DB::isin, DB::Term::In(card_bins)),
//             DB::Clause::Is(DB::gateway, DB::Term::In(gateways.into_iter().map(|g| Some(g.to_string())).collect())),
//             DB::Clause::Is(DB::disabled, DB::Term::Not(DB::Term::Eq(Some(true)))),
//         ],
//     )
//     .await?;

//     to_domain_all(db_res, parse_gateway_card_info)
// }

// pub async fn get_all_by_mgci_ids(ids: Vec<GciPId>) -> Result<Vec<GatewayCardInfo>, Box<dyn Error>> {
//     let ids: Vec<i64> = ids.into_iter().map(|id| id.unGciPId).collect();
//     let db_conf = get_euler_db_conf::<DB::GatewayCardInfoT>().await?;
//     let db_res = find_all_rows(
//         db_conf,
//         mesh_config(),
//         vec![DB::Clause::Is(DB::id, DB::Term::In(ids.into_iter().map(Some).collect()))],
//     )
//     .await?;

//     to_domain_all(db_res, parse_gateway_card_info)
// }
