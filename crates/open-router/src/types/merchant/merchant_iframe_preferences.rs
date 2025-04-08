use serde::{Deserialize, Serialize};
// use db::euler_mesh_impl::mesh_config;
// use db::mesh::internal::find_one_row;
// use eulerhs::types::MeshError;
use crate::app::get_tenant_app_state;
use crate::storage::types::MerchantIframePreferences as DBMerchantIframePreferences;
// use types::utils::dbconfig::get_euler_db_conf;
use crate::types::merchant::id::{to_merchant_id, MerchantId};
// use juspay::extra::parsing::{Parsed, Step, defaulting, lift_pure, mandated, non_negative, parse_field, project};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
use std::i64;
use std::option::Option;
use std::string::String;
// use named::Named;
// use optics_core::review;
use crate::storage::schema::merchant_iframe_preferences::dsl;
use crate::storage::types::BitBool;
use diesel::associations::HasTable;
use diesel::*;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantIFramePrefsPId {
    pub merchantIFramePrefsPId: i64,
}

pub fn to_merchant_iframe_prefs_pid(id: i64) -> MerchantIFramePrefsPId {
    MerchantIFramePrefsPId {
        merchantIFramePrefsPId: id,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantIframePreferences {
    pub id: MerchantIFramePrefsPId,
    pub merchantId: MerchantId,
    pub dynamicSwitchingEnabled: bool,
    pub isinRoutingEnabled: bool,
    pub issuerRoutingEnabled: bool,
    pub txnFailureGatewayPenality: bool,
    pub cardBrandRoutingEnabled: bool,
}

impl From<DBMerchantIframePreferences> for MerchantIframePreferences {
    fn from(value: DBMerchantIframePreferences) -> Self {
        MerchantIframePreferences {
            id: to_merchant_iframe_prefs_pid(value.id),
            merchantId: to_merchant_id(value.merchant_id),
            dynamicSwitchingEnabled: value.dynamic_switching_enabled.unwrap_or(BitBool(false)).0,
            isinRoutingEnabled: value.isin_routing_enabled.unwrap_or(BitBool(false)).0,
            issuerRoutingEnabled: value.issuer_routing_enabled.unwrap_or(BitBool(false)).0,
            txnFailureGatewayPenality: value
                .txn_failure_gateway_penality
                .unwrap_or(BitBool(false))
                .0,
            cardBrandRoutingEnabled: value.card_brand_routing_enabled.unwrap_or(BitBool(false)).0,
        }
    }
}

//  #TOD implement db calls

// pub fn getMerchantIPrefsByMIdDB(
//     m_id: MerchantId,
// ) -> impl MonadFlow<Option<DB::MerchantIframePreferences>> {
//     let db_conf = get_euler_db_conf::<DB::MerchantIframePreferencesT>();
//     find_one_row(
//         db_conf,
//         mesh_config,
//         vec![Named::Is(DB::merchantId, Named::Eq(review(merchantIdText, m_id)))],
//     )
// }

// pub fn getMerchantIPrefsByMId(
//     m_id: MerchantId,
// ) -> impl MonadFlow<Option<MerchantIframePreferences>> {
//     let res = getMerchantIPrefsByMIdDB(m_id);
//     to_domain_all(
//         res,
//         parseMerchantIframePreferences,
//         Named::function_name("getMerchantIPrefsByMId"),
//         Named::parser_name("parseMerchantIframePreferences"),
//     )
// }

pub async fn getMerchantIPrefsByMId(m_id: String) -> Option<MerchantIframePreferences> {
    // Query the database using generic_find_one_optional with Diesel
    let app_state = get_tenant_app_state().await;
    print!("merchant id: {:?}", m_id);
    match crate::generics::generic_find_one_optional::<
        <DBMerchantIframePreferences as HasTable>::Table,
        _,
        DBMerchantIframePreferences,
    >(&app_state.db, dsl::merchant_id.eq(m_id))
    .await
    {
        Ok(Some(db_prefs)) => Some(MerchantIframePreferences::from(db_prefs)),
        Ok(None) => {
            print!("merchant iframe preferences not found");
            None
        }
        Err(err) => {
            print!(
                "Error getting merchant iframe preferences: {:?}",
                err.to_string()
            );
            None
        } // Silently handle any errors by returning None
    }
}
