use crate::app::get_tenant_app_state;
use crate::storage::types::EmiBankCode as DBEmiBankCode;
use time::PrimitiveDateTime;
// use db::storageprelude::LocalTime;
// use eulerhs::language::MonadFlow;
// use eulerhs::extra::combinators::to_domain_all;
// use types::utils::dbconfig::get_euler_db_conf;
// use db::eulermeshimpl::mesh_config;
// use sequelize::{Clause, Term, Where};
use crate::storage::schema::emi_bank_code::dsl;
use diesel::associations::HasTable;
use diesel::*;
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::convert::From;
use std::fmt::Debug;
use std::option::Option;
use std::string::String;
use std::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EbcPId {
    pub ebcPId: i64,
}

pub fn to_ebc_pid(id: i64) -> EbcPId {
    EbcPId { ebcPId: id }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmiBankCode {
    #[serde(rename = "id")]
    pub id: EbcPId,
    #[serde(rename = "emiBank")]
    pub emiBank: String,
    #[serde(rename = "juspayBankCodeId")]
    pub juspayBankCodeId: i64,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: Option<PrimitiveDateTime>,
}

impl From<DBEmiBankCode> for EmiBankCode {
    fn from(value: DBEmiBankCode) -> Self {
        Self {
            id: to_ebc_pid(value.id),
            emiBank: value.emi_bank,
            juspayBankCodeId: value.juspay_bank_code_id,
            lastUpdated: value.last_updated,
        }
    }
}

pub async fn findEmiBankCodeByEMIBank(bank_name: &str) -> Vec<DBEmiBankCode> {
    // Try to find the EMI bank codes using diesel
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<<DBEmiBankCode as HasTable>::Table, _, _>(
        &app_state.db,
        dsl::emi_bank.eq(bank_name.to_owned()),
    )
    .await
    {
        Ok(db_results) => db_results
            .into_iter()
            .map(|db_emi_bank_code: DBEmiBankCode| db_emi_bank_code)
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD implement db calls --done

// pub async fn findEmiBankCodeDB(
//     where_clause: Where<DB::EmiBankCodeT>,
// ) -> Result<Vec<DB::EmiBankCode>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::EmiBankCodeT>().await?;
//     find_all_rows(db_conf, mesh_config(), where_clause).await
// }

// pub async fn findEmiBankCodeByEMIBank(
//     bank_name: String,
// ) -> Result<Vec<EmiBankCode>, MeshError> {
//     let where_clause = vec![Clause::Is(DB::emiBank, Term::Eq(bank_name))];
//     let either_emi_bank_code = findEmiBankCodeDB(where_clause).await;
//     to_domain_all(
//         either_emi_bank_code,
//         parseEmiBankCode,
//         "findEmiBankCodeByEMIBank",
//         "parseEmiBankCode",
//     )
// }
