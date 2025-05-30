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
// use juspay::extra::parsing::{Parsed, Step, liftPure, mandated, nonNegative, parseField, project};
// use eulerhs::extra::combinators::toDomainAll;
// use eulerhs::language::MonadFlow;
// use ghc::generics::Generic;
// use ghc::typelits::KnownSymbol;
// use named::named_macro as named; // Equivalent to Named (!)
// use prelude::*;
// use sequelize::{Clause::{And, Is}, Term::{Eq, In, Null}};
// use test::quickcheck::Arbitrary;

#[cfg(feature = "mysql")]
use crate::storage::schema::gateway_bank_emi_support::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::gateway_bank_emi_support::dsl;
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
    pub gateway: String,
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
            gateway: db_gbes.gateway,
            bank: db_gbes.bank,
            juspayBankCodeId: db_gbes.juspay_bank_code_id,
            scope: db_gbes.scope,
        })
    }
}

pub async fn getGatewayBankEmiSupportDB(
    emi_bank: String,
    t_gws: Vec<String>,
    scp: String,
) -> Result<Vec<DBGatewayBankEmiSupport>, crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;

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
    gws: Vec<String>,
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
