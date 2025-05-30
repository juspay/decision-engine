use crate::error::ApiError;
use masking::Secret;
use serde::{Deserialize, Serialize};
// use eulerhs::types::{MeshError};
// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal::find_all_rows;
use crate::app::get_tenant_app_state;
use crate::storage::types::{BitBool, MerchantGatewayAccount as DBMerchantGatewayAccount};
// use types::utils::dbconfig::get_euler_db_conf;
use crate::types::merchant::id::{to_merchant_id, MerchantId};
// use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, lift_either, lift_pure, mandated, parse_field, project, secret};
// use juspay::extra::secret::{Secret, SecretContext};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
use std::i64;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
// use std::text::Text;

#[cfg(feature = "mysql")]
use crate::storage::schema::merchant_gateway_account::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::merchant_gateway_account::dsl;
use diesel::associations::HasTable;
use diesel::*;
use std::fmt::Debug;

// #[derive(Debug, PartialEq, Serialize, Deserialize)]
// pub struct EulerAccountDetails {
//     #[serde(rename = "merchantId")]
//     pub merchantId: MerchantId,
//     #[serde(rename = "checksumKey")]
//     pub checksumKey: String,
// }

// pub fn to_euler_account_details(ctx: &Text) -> Result<EulerAccountDetails, ParsingErrorType> {
//     match serde_json::from_str::<EulerAccountDetails>(ctx) {
//         Ok(res) => Ok(res),
//         Err(err) => Err(ParsingErrorType::Other(format!("{:?}", err))),
//     }
// }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupportedPaymentFlows {
    #[serde(rename = "paymentFlowIds")]
    pub payment_flow_ids: Vec<String>,
    #[serde(rename = "enforcedPaymentFlows")]
    pub enforcedPaymentFlows: Option<Vec<String>>,
}

pub fn to_supported_payment_flows(
    supported_payment_flows: String,
) -> Result<SupportedPaymentFlows, ApiError> {
    match serde_json::from_str::<SupportedPaymentFlows>(&supported_payment_flows) {
        Ok(res) => Ok(res),
        Err(_) => Err(ApiError::ParsingError("Inavlid Supported Payment Flowws")),
    }
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Clone, Hash, Serialize, Deserialize)]
pub struct MerchantGwAccId {
    #[serde(rename = "merchantGwAccId")]
    pub merchantGwAccId: i64,
}

pub fn to_merchant_gw_acc_id(id: i64) -> MerchantGwAccId {
    MerchantGwAccId {
        merchantGwAccId: id,
    }
}

pub fn merchant_gw_acc_id_to_id(id: MerchantGwAccId) -> i64 {
    id.merchantGwAccId
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MgaReferenceId {
    #[serde(rename = "mgaReferenceId")]
    pub mga_reference_id: String,
}

pub fn to_mga_reference_id(id: String) -> MgaReferenceId {
    MgaReferenceId {
        mga_reference_id: id,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantGatewayAccount {
    #[serde(rename = "id")]
    pub id: MerchantGwAccId,
    #[serde(rename = "accountDetails")]
    pub account_details: Secret<String>,
    #[serde(rename = "gateway")]
    pub gateway: String,
    #[serde(rename = "merchantId")]
    pub merchantId: MerchantId,
    #[serde(rename = "paymentMethods")]
    pub paymentMethods: Option<String>,
    #[serde(rename = "supportedPaymentFlows")]
    pub supported_payment_flows: Option<SupportedPaymentFlows>,
    #[serde(rename = "disabled")]
    pub disabled: Option<bool>,
    #[serde(rename = "referenceId")]
    pub referenceId: Option<MgaReferenceId>,
    #[serde(rename = "supportedCurrencies")]
    pub supportedCurrencies: Option<String>,
    #[serde(rename = "gatewayIdentifier")]
    pub gatewayIdentifier: Option<String>,
    #[serde(rename = "gatewayType")]
    pub gatewayType: Option<String>,
    #[serde(rename = "supportedTxnType")]
    pub supportedTxnType: Option<String>,
}

impl TryFrom<DBMerchantGatewayAccount> for MerchantGatewayAccount {
    type Error = ApiError;

    fn try_from(value: DBMerchantGatewayAccount) -> Result<Self, ApiError> {
        Ok(Self {
            id: to_merchant_gw_acc_id(value.id),
            account_details: Secret::new(value.account_details),
            gateway: value.gateway,
            merchantId: to_merchant_id(value.merchant_id),
            paymentMethods: value.payment_methods,
            supported_payment_flows: value
                .supported_payment_flows
                .map(|flows| to_supported_payment_flows(flows))
                .transpose()?,
            disabled: value.disabled.map(|f| f.0),
            referenceId: value.reference_id.map(|id| to_mga_reference_id(id)),
            supportedCurrencies: value.supported_currencies,
            gatewayIdentifier: value.gateway_identifier,
            gatewayType: value.gateway_type,
            supportedTxnType: value.supported_txn_type,
        })
    }
}

// #TOD Implement DB calls

// getEnabledMgasByMerchantIdDB :: (MonadFlow m, HasCallStack) =>
//   MerchantId -> m (Either MeshError [DB.MerchantGatewayAccount])
// getEnabledMgasByMerchantIdDB mid = do
//   dbConf <- getEulerDbConf @DB.MerchantGatewayAccountT
//   findAllRows dbConf meshConfig
//     [And
//     [ Is DB.merchantId (Eq $ review merchantIdText mid)
//     , Or [ Is DB.disabled (Eq $ Just False)
//            , Is DB.disabled (Eq Nothing)]
//     ]]

// getEnabledMgasByMerchantId :: (MonadFlow m, HasCallStack) =>
//   MerchantId -> m [MerchantGatewayAccount]
// getEnabledMgasByMerchantId mid = do
//   res <- getEnabledMgasByMerchantIdDB mid
//   toDomainAll
//     res
//     parseMerchantGatewayAccount
//     ! #function_name "getEnabledMgasByMerchantId"
//     ! #parser_name "parseMerchantGatewayAccount"

// getEnabledMgasByMerchantIdAndRefIdDB :: (MonadFlow m, HasCallStack) =>
//   MerchantId -> [MgaReferenceId] -> m (Either MeshError [DB.MerchantGatewayAccount])
// getEnabledMgasByMerchantIdAndRefIdDB mid mgaRefId = do
//   dbConf <- getEulerDbConf @DB.MerchantGatewayAccountT
//   let refIds = fmap (\ refId -> unMgaReferenceId refId) mgaRefId
//   findAllRows dbConf meshConfig
//     [And
//       [ Is DB.merchantId (Eq $ review merchantIdText mid)
//       , Or [ Is DB.disabled (Eq $ Just False)
//           , Is DB.disabled (Eq Nothing)]
//       , Or [ Is DB.referenceId (In $ Just <$> refIds)
//           , Is DB.referenceId (Eq Nothing)]
//       ]]

// getEnabledMgasByMerchantIdAndRefId :: (MonadFlow m, HasCallStack) =>
//   MerchantId -> [MgaReferenceId]-> m [MerchantGatewayAccount]
// getEnabledMgasByMerchantIdAndRefId mid refIds = do
//   res <- getEnabledMgasByMerchantIdAndRefIdDB mid refIds
//   toDomainAll
//     res
//     parseMerchantGatewayAccount
//     ! #function_name "getEnabledMgasByMerchantIdAndRefId"
//     ! #parser_name "parseMerchantGatewayAccount"

// #[derive(Debug, PartialEq, Serialize, Deserialize)]
// pub struct ShouldUseV2LinkAndPay {
//     #[serde(rename = "shouldUseV2LinkAndPay")]
//     pub shouldUseV2LinkAndPay: String,
// }

// #[derive(Debug, PartialEq, Serialize, Deserialize)]
// pub struct IsPowerWallet {
//     #[serde(rename = "powerWallet")]
//     pub powerWallet: String,
// }

// #[derive(Debug, PartialEq, Serialize, Deserialize)]
// pub struct IsMandateEnabled {
//     #[serde(rename = "subscription")]
//     pub subscription: String,
// }

pub async fn getEnabledMgasByMerchantId(mid: String) -> Vec<MerchantGatewayAccount> {
    // Call the DB function and handle results using Diesel
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<
        <DBMerchantGatewayAccount as HasTable>::Table,
        _,
        DBMerchantGatewayAccount,
    >(&app_state.db, dsl::merchant_id.eq(mid))
    .await
    {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record| MerchantGatewayAccount::try_from(db_record).ok())
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

pub async fn getEnabledMgasByMerchantIdAndRefId(
    mid: String,
    ref_ids: Vec<String>,
) -> Vec<MerchantGatewayAccount> {
    // Use Diesel's query builder with multiple conditions
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<
        <DBMerchantGatewayAccount as HasTable>::Table,
        _,
        DBMerchantGatewayAccount,
    >(
        &app_state.db,
        dsl::merchant_id
            .eq(mid)
            .and(
                dsl::reference_id
                    .eq_any(ref_ids)
                    .or(dsl::reference_id.is_null()),
            )
            .and(dsl::disabled.eq(BitBool(false)).or(dsl::disabled.is_null())),
    )
    .await
    {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record| MerchantGatewayAccount::try_from(db_record).ok())
            .collect(),
        Err(err) => {
            crate::logger::info!("Error in getEnabledMgasByMerchantIdAndRefId: {:?}", err);
            Vec::new() // Silently handle any errors by returning empty vec
        }
    }
}
