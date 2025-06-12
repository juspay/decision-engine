use serde::{Deserialize, Serialize};
use std::option::Option;
use std::string::String;
use std::vec::Vec;
// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal::*;
use crate::app::get_tenant_app_state;
use crate::storage::types::{BitBool, UserEligibilityInfo as DBUserEligibilityInfo};
// use types::utils::dbconfig::get_euler_db_conf;
// use eulerhs::language::MonadFlow;
// use eulerhs::extra::combinators::to_domain_all;
// use juspay::extra::parsing::{Parsed, Step, ParsingErrorType, lift_either, parse_field, project};
// use sequelize::{Clause::{Is, And, Or}, Term::{Eq, In}};
// use named::Named;
use crate::error::ApiError;
use crate::types::payment_flow::PaymentFlow;

#[cfg(feature = "mysql")]
use crate::storage::schema::user_eligibility_info::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::user_eligibility_info::dsl;
use diesel::associations::HasTable;
use diesel::*;

use super::payment_flow::text_to_payment_flows;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FlowSubtype {
    #[serde(rename = "DCEMI")]
    DCEMI,
    #[serde(rename = "CCEMI")]
    CCEMI,
    #[serde(rename = "CARDLESS")]
    CARDLESS,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderType {
    #[serde(rename = "ISSUER")]
    ISSUER,
    #[serde(rename = "GATEWAY")]
    GATEWAY,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IdentifierName {
    #[serde(rename = "PAN")]
    PAN,
    #[serde(rename = "MOBILE")]
    MOBILE,
    #[serde(rename = "BIN")]
    BIN,
}

pub fn identifier_name_to_text(identifier_name: IdentifierName) -> String {
    match identifier_name {
        IdentifierName::PAN => "PAN".to_string(),
        IdentifierName::MOBILE => "MOBILE".to_string(),
        IdentifierName::BIN => "BIN".to_string(),
    }
}

pub fn text_to_identifier_name(text: String) -> Result<IdentifierName, ApiError> {
    match text.as_str() {
        "PAN" => Ok(IdentifierName::PAN),
        "MOBILE" => Ok(IdentifierName::MOBILE),
        "BIN" => Ok(IdentifierName::BIN),
        _ => Err(ApiError::ParsingError("Invalid Identifier Name")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserEligibilityInfo {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "flowType")]
    pub flowType: PaymentFlow,
    #[serde(rename = "identifierName")]
    pub identifierName: IdentifierName,
    #[serde(rename = "identifierValue")]
    pub identifierValue: String,
    #[serde(rename = "providerName")]
    pub providerName: String,
    #[serde(rename = "disabled")]
    pub disabled: Option<bool>,
}

impl TryFrom<DBUserEligibilityInfo> for UserEligibilityInfo {
    type Error = ApiError;

    fn try_from(db_type: DBUserEligibilityInfo) -> Result<Self, ApiError> {
        Ok(Self {
            id: db_type.id,
            flowType: text_to_payment_flows(db_type.flow_type)
                .map_err(|_| ApiError::ParsingError("Invalid Payment Flow"))?,
            identifierName: text_to_identifier_name(db_type.identifier_name)
                .map_err(|_| ApiError::ParsingError("Invalid Identifier Name"))?,
            identifierValue: db_type.identifier_value,
            providerName: db_type.provider_name,
            disabled: db_type.disabled.map(|f| f.0),
        })
    }
}

// #TOD implement db calls

// pub async fn get_eligibility_info(
//     identifiers: Vec<String>,
//     identifier_name: IdentifierName,
//     provider_name: String,
//     flow_type: PaymentFlow,
// ) -> Result<Vec<UserEligibilityInfo>, Box<dyn Error>> {
//     let db_conf = get_euler_db_conf::<DB::UserEligibilityInfoT>().await?;
//     let res = find_all_rows(
//         db_conf,
//         mesh_config(),
//         vec![
//             And(vec![
//                 Is(DB::identifierValue, In(identifiers)),
//                 Is(DB::identifierName, Eq(identifier_name_to_text(identifier_name))),
//                 Is(DB::providerName, Eq(provider_name)),
//                 Is(DB::flowType, Eq(flow_type_to_text(flow_type))),
//                 Or(vec![
//                     Is(DB::disabled, Eq(Some(false))),
//                     Is(DB::disabled, Eq(None)),
//                 ]),
//             ]),
//         ],
//     ).await?;
//     to_domain_all(res, parse_user_eligibility_info, "getEligibilityInfo", "getEligibilityInfo").await
// }

pub async fn get_eligibility_info(
    identifiers: Vec<String>,
    identifier_name: String,
    provider_name: String,
    flow_type: String,
) -> Vec<UserEligibilityInfo> {
    // Use Diesel's query builder with multiple conditions
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<
        <DBUserEligibilityInfo as HasTable>::Table,
        _,
        DBUserEligibilityInfo,
    >(
        &app_state.db,
        dsl::identifier_value
            .eq_any(identifiers)
            .and(dsl::identifier_name.eq(identifier_name))
            .and(dsl::provider_name.eq(provider_name))
            .and(dsl::flow_type.eq(flow_type))
            .and(
                dsl::disabled.eq(Some(BitBool(false))).or(dsl::disabled.is_null())
            ),
    ).await {
        Ok(db_results) => db_results.into_iter()
                                   .filter_map(|db_record| UserEligibilityInfo::try_from(db_record).ok())
                                   .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}
