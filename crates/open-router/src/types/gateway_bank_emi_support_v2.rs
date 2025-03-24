use serde::{Serialize, Deserialize};
// use db::eulermeshimpl::meshConfig;
// use db::mesh::internal;
// use control::category;
use std::vec::Vec;
use std::string::String;
use std::option::Option;
use std::i64;
use crate::app::get_tenant_app_state;
use crate::error::ApiError;
use crate::storage::types::GatewayBankEmiSupportV2 as DBGatewayBankEmiSupportV2;
// use types::utils::dbconfig::getEulerDbConf;
use crate::types::gateway::{gateway_to_text, Gateway, text_to_gateway};
// use juspay::extra::parsing::{Parsed, Step, liftPure, mandated, nonNegative, parseField, project};
// use eulerhs::extra::combinators::toDomainAll;
// use eulerhs::language::MonadFlow;
// use named::*;
// use sequelize::{Clause, Term};
// use test::quickcheck::Arbitrary;

use crate::storage::schema::gateway_bank_emi_support_v2::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GbesPId {
    pub gbesPId: i64,
}

pub fn to_gbes_pid(id: i64) -> GbesPId {
    GbesPId {
        gbesPId: id,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayBankEmiSupportV2 {
    pub id: GbesPId,
    pub gateway: Gateway,
    pub juspayBankCodeId: i64,
    pub scope: String,
    pub cardType: String,
    pub tenure: i32,
}

impl TryFrom<DBGatewayBankEmiSupportV2> for GatewayBankEmiSupportV2 {
    type Error = ApiError;

    fn try_from(db_gbes: DBGatewayBankEmiSupportV2) -> Result<Self, ApiError> {
        Ok(GatewayBankEmiSupportV2 {
            id: to_gbes_pid(db_gbes.id),
            gateway: text_to_gateway(db_gbes.gateway.as_str()).map_err(|_| ApiError::ParsingError("Invalid gateway"))?,
            juspayBankCodeId: db_gbes.juspay_bank_code_id,
            scope: db_gbes.scope,
            cardType: db_gbes.card_type,
            tenure: db_gbes.tenure,
        })
    }
}

pub async fn getGatewayBankEmiSupportV2DB(
    jbc_id: i64,
    gws: &[Gateway],
    scp: String,
    ct: String,
    ten: i32,
) -> Result<Vec<DBGatewayBankEmiSupportV2>, crate::generics::MeshError> {
    // Convert Gateway enum values to strings
    let gateway_strings: Vec<String> = gws.iter().map(|gw| gateway_to_text(gw)).collect();
    let app_state = get_tenant_app_state().await;
    
    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_all::<
            <DBGatewayBankEmiSupportV2 as HasTable>::Table,
            _,
            DBGatewayBankEmiSupportV2
        >(
            &app_state.db,
            dsl::juspay_bank_code_id.eq(jbc_id)
                .and(dsl::gateway.eq_any(gateway_strings))
                .and(dsl::scope.eq(scp))
                .and(dsl::card_type.eq(ct))
                .and(dsl::tenure.eq(ten)),
        )
        .await
}

// Domain-level function with error handling and conversion
pub async fn getGatewayBankEmiSupportV2(
    
    jbc_id: i64,
    gws: &[Gateway],
    scp: String,
    ct: String,
    ten: i32,
) -> Vec<GatewayBankEmiSupportV2> {
    // Call the DB function and handle results
    match getGatewayBankEmiSupportV2DB( jbc_id, gws, scp, ct, ten).await {
        Ok(db_results) => db_results.into_iter()
                                   .filter_map(|db_record| GatewayBankEmiSupportV2::try_from(db_record).ok())
                                   .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}



// #TOD implement db calls

// pub async fn getGatewayBankEmiSupportV2DB(
//     jbc_id: i64,
//     gws: Vec<Gateway>,
//     scp: String,
//     ct: String,
//     ten: i32,
// ) -> Result<Vec<db::storage::types::gatewaybankemisupportv2::GatewayBankEmiSupportV2>, MeshError> {
//     let db_conf = getEulerDbConf::<db::storage::types::gatewaybankemisupportv2::GatewayBankEmiSupportV2T>().await?;
//     let t_gws: Vec<String> = gws.into_iter().map(gateway_to_text).collect();
//     findAllRows(
//         db_conf,
//         meshConfig,
//         vec![Clause::And(vec![
//             Clause::Is("juspayBankCodeId", Term::Eq(jbc_id)),
//             Clause::Is("gateway", Term::In(t_gws)),
//             Clause::Is("scope", Term::Eq(scp)),
//             Clause::Is("cardType", Term::Eq(ct)),
//             Clause::Is("tenure", Term::Eq(ten)),
//         ])],
//     )
//     .await
// }

// pub async fn getGatewayBankEmiSupportV2(
//     jbc_id: i64,
//     gws: Vec<Gateway>,
//     scp: String,
//     ct: String,
//     ten: i32,
// ) -> Result<Vec<GatewayBankEmiSupportV2>, MeshError> {
//     let res = getGatewayBankEmiSupportV2DB(jbc_id, gws, scp, ct, ten).await?;
//     toDomainAll(
//         res,
//         parseGatewayBankEmiSupportV2,
//         named::named! {
//             function_name: "getGatewayBankEmiSupportV2",
//             parser_name: "parseGatewayBankEmiSupportV2",
//         },
//     )
// }
