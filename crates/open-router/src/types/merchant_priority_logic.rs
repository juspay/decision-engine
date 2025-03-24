
use serde::{Serialize, Deserialize};
use serde_json::Value;
use time::PrimitiveDateTime;
use std::option::Option;
use std::vec::Vec;
use std::string::String;
use std::time::SystemTime;
use std::convert::TryFrom;
use std::result::Result;
use std::error::Error;
use crate::storage::schema::merchant_priority_logic::dsl;
use diesel::*;
use diesel::associations::HasTable;
use crate::app::get_tenant_app_state;
use crate::storage::types::MerchantPriorityLogic as DBMerchantPriorityLogic;
// use types::utils::dbconfig as DBConf;
use crate::types::merchant::id::{MerchantPId, to_merchant_pid};
// use eulerhs::language::MonadFlow;
// use eulerhs::extra::combinators::to_domain_all;
// use db::mesh::internal::{find_all_rows, find_one_row};
// use db::eulermeshimpl::mesh_config;
use std::marker::PhantomData;
// use named::Named;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantPriorityLogic {
    #[serde(rename = "version")]
    pub version: i64,
    #[serde(rename = "dateCreated")]
    pub dateCreated: PrimitiveDateTime,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: PrimitiveDateTime,
    #[serde(rename = "merchantAccountId")]
    pub merchantAccountId: MerchantPId,
    #[serde(rename = "status")]
    pub status: String,
    #[serde(rename = "priorityLogic")]
    pub priorityLogic: String,
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "name")]
    pub name: Option<String>,
    #[serde(rename = "description")]
    pub description: Option<String>,
    #[serde(rename = "priorityLogicRules")]
    pub priorityLogicRules: Option<String>,
    #[serde(rename = "isActiveLogic")]
    pub isActiveLogic: bool,
}

impl From<DBMerchantPriorityLogic> for MerchantPriorityLogic {
    fn from(db_type: DBMerchantPriorityLogic) -> Self {
        MerchantPriorityLogic {
            version: db_type.version,
            dateCreated: db_type.date_created,
            lastUpdated: db_type.last_updated,
            merchantAccountId: to_merchant_pid(db_type.merchant_account_id),
            status: db_type.status,
            priorityLogic: db_type.priority_logic,
            id: db_type.id,
            name: db_type.name,
            description: db_type.description,
            priorityLogicRules: db_type.priority_logic_rules,
            isActiveLogic: db_type.is_active_logic,
        }
    }
}

// #TOD implement db calls

// pub async fn find_all_priority_logic_by_merchant_pid_db<M: MonadFlow>(
//     mpid: MerchantPId,
// ) -> Result<Vec<DB::MerchantPriorityLogic>, MeshError> {
//     let db_conf = DBConf::get_euler_db_conf::<DB::MerchantPriorityLogicT>().await?;
//     find_all_rows(db_conf, mesh_config(), vec![DB::Clause::Is(DB::merchant_account_id, DB::Term::Eq(mpid))]).await
// }

// pub async fn find_all_priority_logic_by_merchant_pid<M: MonadFlow>(
//     mpid: MerchantPId,
// ) -> Result<Vec<MerchantPriorityLogic>, Box<dyn Error>> {
//     let res = find_all_priority_logic_by_merchant_pid_db(mpid).await?;
//     to_domain_all(res, parse_merchant_priority_logic, "findAllPriorityLogicByMerchantPId", "parseMerchantPriorityLogic")
// }

// pub async fn find_priority_logic_by_id_db<M: MonadFlow>(
//     mpl_id: String,
// ) -> Result<Option<DB::MerchantPriorityLogic>, MeshError> {
//     let db_conf = DBConf::get_euler_db_conf::<DB::MerchantPriorityLogicT>().await?;
//     find_one_row(db_conf, mesh_config(), vec![DB::Clause::Is(DB::id, DB::Term::Eq(mpl_id))]).await
// }

// pub async fn find_priority_logic_by_id<M: MonadFlow>(
//     mpl_id: String,
// ) -> Result<Option<MerchantPriorityLogic>, Box<dyn Error>> {
//     let res = find_priority_logic_by_id_db(mpl_id).await?;
//     to_domain_all(res, parse_merchant_priority_logic, "findPriorityLogicById", "parseMerchantPriorityLogic")
// }


pub async fn find_all_priority_logic_by_merchant_pid(
    
    mpid: i64,
) -> Vec<MerchantPriorityLogic> {
    // Call the DB using Diesel's generic find all function
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<
            <DBMerchantPriorityLogic as HasTable>::Table,
            _,
            DBMerchantPriorityLogic
        >(
            &app_state.db,
            dsl::merchant_account_id.eq(mpid),
        ).await {
        Ok(db_results) => db_results.into_iter()
                                   .filter_map(|db_record| MerchantPriorityLogic::try_from(db_record).ok())
                                   .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning an empty vector
    }
}

pub async fn find_priority_logic_by_id(
    
    mpl_id: i64,
) -> Option<MerchantPriorityLogic> {
    // Perform the database query using Diesel
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_one_optional::<
            <DBMerchantPriorityLogic as HasTable>::Table,
            _,
            DBMerchantPriorityLogic
        >(
            &app_state.db,
            dsl::merchant_account_id.eq(mpl_id),
        ).await {
            Ok(Some(db_record)) => Some(MerchantPriorityLogic::from(db_record)),
            Ok(None) => None,
            Err(_) => None, // Silently handle any errors by returning None
        }
}
