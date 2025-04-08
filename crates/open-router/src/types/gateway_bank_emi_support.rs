// use db::euler_mesh_impl::meshConfig;
// use db::mesh::internal::*;
// use control::category::compose as compose; // Equivalent to Haskell's >>> operator
use crate::app::get_tenant_app_state;
use crate::error::ApiError;
use crate::storage::types::GatewayBankEmiSupport as DBGatewayBankEmiSupport;
use serde::{Deserialize, Serialize};
use std::option::Option;
use std::string::String;
use std::vec::Vec;
// use types::utils::dbconfig::getEulerDbConf;
use crate::types::gateway::{gateway_to_text, text_to_gateway, Gateway};
// use juspay::extra::parsing::{Parsed, Step, liftPure, mandated, nonNegative, parseField, project};
// use eulerhs::extra::combinators::toDomainAll;
// use eulerhs::language::MonadFlow;
// use ghc::generics::Generic;
// use ghc::typelits::KnownSymbol;
// use named::named_macro as named; // Equivalent to Named (!)
// use prelude::*;
// use sequelize::{Clause::{And, Is}, Term::{Eq, In, Null}};
// use test::quickcheck::Arbitrary;

use crate::storage::schema::gateway_bank_emi_support::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct GbesPId {
    pub gbesPId: i64,
}

pub fn to_gbes_pid(id: i64) -> GbesPId {
    GbesPId { gbesPId: id }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayBankEmiSupport {
    #[serde(rename = "id")]
    pub id: GbesPId,
    #[serde(rename = "gateway")]
    pub gateway: Gateway,
    #[serde(rename = "bank")]
    pub bank: String,
    #[serde(rename = "juspayBankCodeId")]
    pub juspayBankCodeId: Option<i64>,
    #[serde(rename = "scope")]
    pub scope: Option<String>,
}

impl TryFrom<DBGatewayBankEmiSupport> for GatewayBankEmiSupport {
    type Error = ApiError;

    fn try_from(db_gbes: DBGatewayBankEmiSupport) -> Result<Self, ApiError> {
        Ok(Self {
            id: to_gbes_pid(db_gbes.id),
            gateway: text_to_gateway(db_gbes.gateway.as_str())
                .map_err(|_| ApiError::ParsingError("Invalid gateway"))?,
            bank: db_gbes.bank,
            juspayBankCodeId: db_gbes.juspay_bank_code_id,
            scope: db_gbes.scope,
        })
    }
}

pub async fn getGatewayBankEmiSupportDB(
    emi_bank: String,
    gws: Vec<Gateway>,
    scp: String,
) -> Result<Vec<DBGatewayBankEmiSupport>, crate::generics::MeshError> {
    // Convert Gateway enum values to strings
    let app_state = get_tenant_app_state().await;
    let t_gws: Vec<String> = gws.iter().map(gateway_to_text).collect();

    // Build the query based on scp value
    let query = if scp == "NULL" {
        crate::generics::generic_find_all::<
            <DBGatewayBankEmiSupport as HasTable>::Table,
            _,
            DBGatewayBankEmiSupport,
        >(
            &app_state.db,
            dsl::bank
                .eq(emi_bank)
                .and(dsl::gateway.eq_any(t_gws))
                .and(dsl::scope.is_null()),
        )
        .await
    } else {
        crate::generics::generic_find_all::<
            <DBGatewayBankEmiSupport as HasTable>::Table,
            _,
            DBGatewayBankEmiSupport,
        >(
            &app_state.db,
            dsl::bank
                .eq(emi_bank)
                .and(dsl::gateway.eq_any(t_gws))
                .and(dsl::scope.eq(scp)),
        )
        .await
    };

    // Execute the query
    query
}

pub async fn getGatewayBankEmiSupport(
    emi_bank: String,
    gws: Vec<Gateway>,
    scp: String,
) -> Vec<GatewayBankEmiSupport> {
    // Call the DB function and handle results
    match getGatewayBankEmiSupportDB(emi_bank, gws, scp).await {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record| GatewayBankEmiSupport::try_from(db_record).ok())
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD implement db calls

// pub async fn getGatewayBankEmiSupportDB(
//     emi_bank: String,
//     gws: Vec<Gateway>,
//     scp: String,
// ) -> Result<Vec<DB::GatewayBankEmiSupport>, MeshError> {
//     let db_conf = getEulerDbConf::<DB::GatewayBankEmiSupportT>().await?;
//     let t_gws: Vec<String> = gws.iter().map(|gw| gatewayToText(gw)).collect();
//     findAllRows(
//         db_conf,
//         meshConfig,
//         vec![And(vec![
//             Is(DB::bank, Eq(emi_bank)),
//             Is(DB::gateway, In(t_gws)),
//             if scp == "NULL" {
//                 Is(DB::scope, Null)
//             } else {
//                 Is(DB::scope, Eq(Some(scp)))
//             },
//         ])],
//     )
//     .await
// }

// pub async fn getGatewayBankEmiSupport(
//     emi_bank: String,
//     gws: Vec<Gateway>,
//     scp: String,
// ) -> Result<Vec<GatewayBankEmiSupport>, MeshError> {
//     let res = getGatewayBankEmiSupportDB(emi_bank, gws, scp).await?;
//     toDomainAll(
//         res,
//         parseGatewayBankEmiSupport,
//         named! { function_name = "getGatewayBankEmiSupport" },
//         named! { parser_name = "parseGatewayBankEmiSupport" },
//     )
//     .await
// }
