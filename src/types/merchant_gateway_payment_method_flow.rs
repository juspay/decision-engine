use crate::app::get_tenant_app_state;
#[cfg(not(feature = "db_migration"))]
use crate::storage::schema::merchant_gateway_payment_method_flow::dsl;
#[cfg(feature = "db_migration")]
use crate::storage::schema_pg::merchant_gateway_payment_method_flow::dsl;
use diesel::associations::HasTable;
use diesel::*;
use serde::{Deserialize, Serialize};
use std::option::Option;
use std::string::String;
use std::vec::Vec;
use time::PrimitiveDateTime;
// use db::euler_mesh_impl::mesh_config;
// use db::mesh::internal::*;
use crate::storage::types::{BitBool, MerchantGatewayPaymentMethodFlow as DBMerchantGatewayPaymentMethodFlow};
// use types::utils::dbconfig::get_euler_db_conf;
// use eulerhs::extra::aeson::aeson_omit_nothing_fields;
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
// use juspay::extra::parsing::{Parsed, Step, parse_field, project, to_utc};
// use named::*;
use crate::types::gateway_payment_method_flow::{
    gateway_payment_method_flow_id_text, to_gateway_payment_method_flow_id,
    GatewayPaymentMethodFlowId,
};
// use sequelize::{Clause::{Is, And}, Term::{Eq, In}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantGatewayPaymentMethodFlow {
    #[serde(rename = "id")]
    pub id: Option<i64>,
    #[serde(rename = "gatewayPaymentMethodFlowId")]
    pub gatewayPaymentMethodFlowId: GatewayPaymentMethodFlowId,
    #[serde(rename = "merchantGatewayAccountId")]
    pub merchantGatewayAccountId: i64,
    #[serde(rename = "currencyConfigs")]
    pub currencyConfigs: Option<String>,
    #[serde(rename = "dateCreated")]
    pub dateCreated: PrimitiveDateTime,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: PrimitiveDateTime,
    #[serde(rename = "disabled")]
    pub disabled: Option<bool>,
    #[serde(rename = "gatewayBankCode")]
    pub gatewayBankCode: Option<String>,
}

impl From<DBMerchantGatewayPaymentMethodFlow> for MerchantGatewayPaymentMethodFlow {
    fn from(db_mgpmf: DBMerchantGatewayPaymentMethodFlow) -> Self {
        Self {
            id: Some(db_mgpmf.id),
            gatewayPaymentMethodFlowId: to_gateway_payment_method_flow_id(
                db_mgpmf.gateway_payment_method_flow_id,
            ),
            merchantGatewayAccountId: db_mgpmf.merchant_gateway_account_id,
            currencyConfigs: db_mgpmf.currency_configs,
            dateCreated: db_mgpmf.date_created,
            lastUpdated: db_mgpmf.last_updated,
            disabled: db_mgpmf.disabled.map(|f| f.0) ,
            gatewayBankCode: db_mgpmf.gateway_bank_code,
        }
    }
}

// #TOD Implement DB Calls (Only last function required)

// pub async fn get_mgpmf_by_id_db(
//     mgpmf_id: i64,
// ) -> Result<Option<DB::MerchantGatewayPaymentMethodFlow>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::MerchantGatewayPaymentMethodFlowT>().await?;
//     find_one_row(db_conf, mesh_config(), vec![Is(DB::id, Eq(Some(mgpmf_id)))]).await
// }

// pub async fn get_mgpmf_by_id(
//     mgpmf_id: i64,
// ) -> Option<MerchantGatewayPaymentMethodFlow> {
//     let res = get_mgpmf_by_id_db(mgpmf_id).await;
//     to_domain_all(
//         res,
//         parse_merchant_gateway_payment_method_flow,
//         named! {
//             function_name: "getMgpmfById",
//             parser_name: "getMgpmfById"
//         },
//     )
// }

// pub async fn get_mgpmfs_by_ids(
//     ids: Vec<i64>,
// ) -> Vec<MerchantGatewayPaymentMethodFlow> {
//     let db_conf = get_euler_db_conf::<DB::MerchantGatewayPaymentMethodFlowT>().await?;
//     let res = find_all_rows(
//         db_conf,
//         mesh_config(),
//         vec![Is(DB::id, In(ids.into_iter().map(Some).collect()))],
//     )
//     .await;
//     to_domain_all(
//         res,
//         parse_merchant_gateway_payment_method_flow,
//         named! {
//             function_name: "getMgpmfsByIds",
//             parser_name: "getMgpmfsByIds"
//         },
//     )
// }

// pub async fn get_all_mgpmf_by_mga_id_and_gpmf_ids_db(
//     mga_ids: Vec<i64>,
//     gpmf_ids: Vec<GatewayPaymentMethodFlowId>,
// ) -> Result<Vec<DB::MerchantGatewayPaymentMethodFlow>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::MerchantGatewayPaymentMethodFlowT>().await?;
//     let gpmf_ids_text: Vec<String> = gpmf_ids
//         .into_iter()
//         .map(|id| review(gateway_payment_method_flow_id_text(), id))
//         .collect();
//     find_all_rows(
//         db_conf,
//         mesh_config(),
//         vec![And(vec![
//             Is(DB::merchantGatewayAccountId, In(mga_ids)),
//             Is(DB::gatewayPaymentMethodFlowId, In(gpmf_ids_text)),
//             Is(DB::disabled, Eq(Some(false))),
//         ])],
//     )
//     .await
// }

// pub async fn get_all_mgpmf_by_mga_id_and_gpmf_ids(
//     mga_ids: Vec<i64>,
//     gpmf_ids: Vec<GatewayPaymentMethodFlowId>,
// ) -> Vec<MerchantGatewayPaymentMethodFlow> {
//     let res = get_all_mgpmf_by_mga_id_and_gpmf_ids_db(mga_ids, gpmf_ids).await;
//     to_domain_all(
//         res,
//         parse_merchant_gateway_payment_method_flow,
//         named! {
//             function_name: "getAllMgpmfByMgaIdAndGpmfIds",
//             parser_name: "parseMerchantGatewayPaymentMethodFlow"
//         },
//     )
// }

pub async fn get_all_mgpmf_by_mga_id_and_gpmf_ids(
    mga_ids: Vec<i64>,
    gpmf_ids: Vec<GatewayPaymentMethodFlowId>,
) -> Vec<MerchantGatewayPaymentMethodFlow> {
    // Convert GatewayPaymentMethodFlowId to Strings for query
    let gpmf_id_strings: Vec<String> = gpmf_ids
        .into_iter()
        .map(gateway_payment_method_flow_id_text)
        .collect();
    let app_state = get_tenant_app_state().await;
    // Query using Diesel and generic_find_all
    match crate::generics::generic_find_all::<
        <DBMerchantGatewayPaymentMethodFlow as HasTable>::Table,
        _,
        DBMerchantGatewayPaymentMethodFlow,
    >(
        &app_state.db,
        dsl::merchant_gateway_account_id
            .eq_any(mga_ids)
            .and(dsl::gateway_payment_method_flow_id.eq_any(gpmf_id_strings))
            .and(dsl::disabled.eq(BitBool(false))),
    )
    .await
    {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record| MerchantGatewayPaymentMethodFlow::try_from(db_record).ok())
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}
