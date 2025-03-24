
use serde::{Serialize, Deserialize};
// use db::eulermeshimpl::{mesh_config, throw_missing_tenant_error, throw_tenant_mismatch_error, default_tenant_account_id};
// use db::mesh::internal::*;
use crate::storage::types::MerchantAccount as DBMerchantAccount;
use crate::error::ApiError;
// use types::utils::dbconfig::get_euler_db_conf;
// use types::locker::id::{LockerId, to_locker_id};
use crate::app::get_tenant_app_state;
use crate::types::merchant::id::{MerchantId, MerchantPId, to_merchant_id, to_merchant_pid};
// use juspay::extra::parsing::{Parsed, Step, around, defaulting, lift_pure, mandated, non_negative, parse_field, project};
// use juspay::extra::secret::SecretContext;
// use juspay::extra::nonemptytext::non_empty;
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::{MonadFlow, log_error, throw_exception, get_option_local, TenantConfigObj, TenantConfig};
// use eulerhs::prelude::{bool, from_maybe, when};
use serde_json::Value as AValue;
use std::option::Option;
use std::vec::Vec;
use std::string::String;
use std::collections::HashMap;
use std::sync::Arc;
use std::convert::From;
use std::cmp::PartialEq;
use std::fmt::Debug;
use crate::storage::schema::merchant_account::dsl;
use diesel::*;
use diesel::associations::HasTable;


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnableTokenization {
    #[serde(rename = "enable_network_tokenization")]
    pub enable_network_tokenization: bool,
    #[serde(rename = "enable_issuer_tokenization")]
    pub enable_issuer_tokenization: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantAccount {
    // #[serde(rename = "id")]
    pub id: MerchantPId,
    // #[serde(rename = "merchantId")]
    pub merchantId: MerchantId,
    // #[serde(rename = "country")]
    pub country: Option<String>,
    // #[serde(rename = "gatewayDecidedByHealthEnabled")]
    pub gatewayDecidedByHealthEnabled: Option<bool>,
    // #[serde(rename = "gatewayPriority")]
    pub gatewayPriority: Option<String>,
    // #[serde(rename = "gatewayPriorityLogic")]
    pub gatewayPriorityLogic: String,
    // #[serde(rename = "useCodeForGatewayPriority")]
    pub useCodeForGatewayPriority: bool,
    // #[serde(rename = "internalHashKey")]
    pub internalHashKey: Option<String>,
    // #[serde(rename = "lockerId")]
    // pub lockerId: Option<LockerId>,
    // #[serde(rename = "tokenLockerId")]
    // pub tokenLockerId: Option<String>,
    // #[serde(rename = "userId")]
    pub userId: Option<i64>,
    // #[serde(rename = "secondaryMerchantAccountId")]
    pub secondaryMerchantAccountId: Option<MerchantPId>,
    // #[serde(rename = "enableGatewayReferenceIdBasedRouting")]
    pub enableGatewayReferenceIdBasedRouting: Option<bool>,
    // #[serde(rename = "gatewaySuccessRateBasedDeciderInput")]
    pub gatewaySuccessRateBasedDeciderInput: String,
    // #[serde(rename = "internalMetadata")]
    pub internalMetadata: Option<String>,
    // #[serde(rename = "installmentEnabled")]
    pub installmentEnabled: Option<bool>,
    // #[serde(rename = "tenantAccountId")]
    pub tenantAccountId: Option<String>,
    // #[serde(rename = "priorityLogicConfig")]
    pub priorityLogicConfig: Option<String>,
    // #[serde(rename = "merchantCategoryCode")]
    pub merchantCategoryCode: Option<String>,
}

// The following functions are placeholders for the Haskell functions.
// They should be implemented as per the Rust project requirements.

impl TryFrom<DBMerchantAccount> for MerchantAccount {
    type Error = ApiError;

    fn try_from(value: DBMerchantAccount) ->  Result<Self, ApiError> {
        Ok(MerchantAccount {
            id: to_merchant_pid(value.id),
            merchantId: value.merchant_id.map(|mid| to_merchant_id(mid)).ok_or(ApiError::ParsingError("Merchant Id Not Found"))?,
            country: value.country,
            gatewayDecidedByHealthEnabled: value.gateway_decided_by_health_enabled,
            gatewayPriority: value.gateway_priority,
            gatewayPriorityLogic: value.gateway_priority_logic.unwrap_or("".to_string()),
            useCodeForGatewayPriority: value.use_code_for_gateway_priority,
            internalHashKey: value.internal_hash_key,
            userId: value.user_id,
            secondaryMerchantAccountId: value.secondary_merchant_account_id.map(|mid| to_merchant_pid(mid)),
            enableGatewayReferenceIdBasedRouting: value.enable_gateway_reference_id_based_routing,
            gatewaySuccessRateBasedDeciderInput: value.gateway_success_rate_based_decider_input.unwrap_or("".to_string()),
            internalMetadata: value.internal_metadata,
            installmentEnabled: value.installment_enabled,
            tenantAccountId: value.tenant_account_id,
            priorityLogicConfig: value.priority_logic_config,
            merchantCategoryCode: value.merchant_category_code,
        })
    }
}

// pub fn compare_tenant_ids<T: MonadFlow>(
//     tenant_acct_id_from_req: String,
//     m_tenant_account_id_from_db: Option<String>,
//     m_id: String,
// ) -> T {
//     // Placeholder implementation
//     unimplemented!()
// }

// pub fn check_if_tenant_id_in_context_m<T: MonadFlow>(
//     either_macc_m: Result<Option<DB::MerchantAccount>, MeshError>,
// ) -> T {
//     // Placeholder implementation
//     unimplemented!()
// }

// pub fn get_tenant_account_id<T: MonadFlow>() -> T {
//     // Placeholder implementation
//     unimplemented!()
// }

// #TOD implement db calls

// pub fn get_by_merchant_id_db<T: MonadFlow>(
//     merchant_id: MerchantId,
// ) -> Result<Option<DB::MerchantAccount>, MeshError> {
//     // Placeholder implementation
//     unimplemented!()
// }

// pub fn load_merchant_by_merchant_id<T: MonadFlow>(
//     merchant_id: MerchantId,
// ) -> Option<MerchantAccount> {
//     // Placeholder implementation
//     unimplemented!()
// }

pub async fn load_merchant_by_merchant_id(
    merchant_id: String,
) -> Option<MerchantAccount> {
    // Perform the query using Diesel's generic_find_all
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<
        <DBMerchantAccount as HasTable>::Table,
        _,
        DBMerchantAccount
    >(
        &app_state.db,
        dsl::merchant_id.eq(merchant_id)
    ).await {
        Ok(mut db_results) => db_results.pop().and_then(|db_merchant| MerchantAccount::try_from(db_merchant).ok()),
        Err(_) => None, // Silently handle errors and return None
    }
}
