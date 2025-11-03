use crate::{
    decider::configs::env_vars::groovy_executor_url,
    error::ApiClientError,
    logger,
    redis::feature::roller,
    types::{
        card::{
            card_info::CardInfo,
            card_type::{card_type_to_text, CardType},
            isin::{to_isin, Isin},
            txn_card_info::{auth_type_to_text, TxnCardInfo},
        },
        currency::Currency,
        customer::CustomerId,
        merchant::{
            id::{merchant_pid_to_text, MerchantId},
            merchant_account::MerchantAccount,
        },
        merchant_priority_logic::{
            find_all_priority_logic_by_merchant_pid, find_priority_logic_by_id,
        },
        money::internal::Money,
        order::{
            id::{OrderId, ProductId},
            udfs::get_udf,
            Order,
        },
        tenant::{
            tenant_config::{ConfigType, FilterDimension, ModuleName},
            tenant_config_filter::get_tenant_config_filter_by_group_id_and_dimension_value,
        },
        tenant_config::get_tenant_config_by_tenant_id_and_module_name_and_module_key_and_type,
        transaction::id::TransactionId,
        txn_details::types::{TxnDetail, TxnObjectType},
    },
    utils::call_api,
};
use masking::PeekInterface;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, Value};

use crate::decider::gatewaydecider::types as DeciderTypes;

use super::utils;
use crate::types::payment::payment_method_type_const::*;
// use serde_json::Value as AValue;
// use eulerhs::prelude::*;
// use data::aeson::{Object, either_decode, (.:)};
// use data::aeson::types::parse_either;
// use data::byte_string::lazy as BSL;
// use data::maybe::from_just;
// use data::reflection::give;
// use data::text as DT::{is_infix_of, pack, to_upper, strip, to_lower};
// use types::card::{TxnCardInfo, card_type_to_text};
// use types::card::card_info::{CardInfo};
// use types::currency::Currency;
// use types::customer::CustomerId;
// use types::gateway::{text_to_gateway};
// use types::merchant::MerchantId;
// use types::money::{Money, to_double};
// use types::order::{Order, OrderId, ProductId, get_udf};
// use juspay::extra::json::{decode_json, encode_json};
// use juspay::extra::parsing::{Parsed, parse};
// use juspay::extra::secret::{SecretContext, unsafe_extract_secret};
// use types::transaction::TransactionId;
// use types::txn_detail::TxnDetail;
// use eulerhs::api_helpers as ET;
// use eulerhs::language::{MonadFlow, call_api};
// use servant::{JSON, Post, ReqBody, (:<|>), (:>)};
// use utils::api_tag;
// use types::gateway::{gateway_to_text, text_to_gateway};
// use types::card::card_info::{CardInfo};
// use types::card::{Isin};
// use optics::core::preview;
// use types::gateway::Gateway;
// use servant::client::{ClientError, ResponseF};
// use types::merchant_priority_logic as MPL;
// use gateway_decider::utils as utils;
// use data::aeson as A;
// use juspay::extra::env as Env;
// use types::card as ETCa;
// use eulerhs::types as T;
// use eulerhs::api_helpers as T;
// use network::http::client as HC;
// use servant::client as SC;
// use eulerhs::language as L;
// use gateway_decider::types as DeciderTypes;
// use gateway_decider::types::{GatewayPriorityLogicOutput, PriorityLogicData};
// use data::text::encoding as TE;
// use types::txn_detail as ETTD;
// use types::token_bin_info as ETTB;
// use data::text as T;
// use types::merchant as ETM;
// use types::order as ETO;
// use utils::redis::feature as Feature;
// use juspay::extra::json as JSON;
// use types::tenant_config as TenantConfig;
// use types::tenant_config_filter as TenantConfigFilter;

#[derive(Debug, Serialize, Deserialize)]
pub struct FilteredOrderInfo {
    pub amount: f64,
    pub currency: Currency,
    pub customerId: Option<CustomerId>,
    pub orderId: OrderId,
    pub productId: Option<ProductId>,
    pub description: Option<String>,
    pub preferredGateway: Option<String>,
    pub udf1: Option<String>,
    pub udf2: Option<String>,
    pub udf3: Option<String>,
    pub udf4: Option<String>,
    pub udf5: Option<String>,
    pub udf6: Option<String>,
    pub udf7: Option<String>,
    pub udf8: Option<String>,
    pub udf9: Option<String>,
    pub udf10: Option<String>,
    pub orderMetaData: Option<Value>,
}

pub fn filter_order(order: Order, metaData: Option<Value>) -> FilteredOrderInfo {
    FilteredOrderInfo {
        amount: Money::to_double(&order.amount) / 10000.0,
        currency: order.currency,
        customerId: order.customerId,
        orderId: order.orderId,
        productId: order.productId,
        description: order.description,
        preferredGateway: order.preferredGateway,
        udf1: get_udf(&order.udfs, 0).cloned(),
        udf2: get_udf(&order.udfs, 1).cloned(),
        udf3: get_udf(&order.udfs, 2).cloned(),
        udf4: get_udf(&order.udfs, 3).cloned(),
        udf5: get_udf(&order.udfs, 4).cloned(),
        udf6: get_udf(&order.udfs, 5).cloned(),
        udf7: get_udf(&order.udfs, 6).cloned(),
        udf8: get_udf(&order.udfs, 7).cloned(),
        udf9: get_udf(&order.udfs, 8).cloned(),
        udf10: get_udf(&order.udfs, 9).cloned(),
        orderMetaData: metaData,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilteredTxnInfo {
    pub isEmi: bool,
    pub emiBank: Option<String>,
    pub emiTenure: Option<i32>,
    pub txnId: TransactionId,
    pub addToLocker: bool,
    pub expressCheckout: bool,
    pub sourceObject: Option<String>,
    pub txnObjectType: TxnObjectType,
}

pub fn filter_txn(detail: TxnDetail) -> FilteredTxnInfo {
    FilteredTxnInfo {
        isEmi: detail.isEmi.unwrap_or(false),
        emiBank: detail.emiBank,
        emiTenure: detail.emiTenure,
        txnId: detail.txnId,
        addToLocker: detail.addToLocker.unwrap_or(false),
        expressCheckout: detail.expressCheckout.unwrap_or(false),
        sourceObject: detail.sourceObject,
        txnObjectType: detail.txnObjectType.unwrap_or(TxnObjectType::Unknown),
    }
}

pub fn fetch_emi_type(txnCardInfo: TxnCardInfo) -> Result<String, Vec<LogEntry>> {
    match txnCardInfo.paymentSource {
        None => Err(vec![]),
        Some(ps) => {
            if ps.contains("emi_type") {
                Err(vec![])
            } else {
                match from_str::<Value>(&ps) {
                    Ok(value) => match value.get("emi_type") {
                        Some(emi_type) => match emi_type.as_str() {
                            Some(emi_type_str) => Ok(emi_type_str.to_string()),
                            None => Err(vec![LogEntry::Error(
                                "Invalid emi_type".to_string(),
                                "Error while parsing emi_type value from JSON".to_string(),
                            )]),
                        },
                        None => Err(vec![LogEntry::Error(
                            "emi_type not found".to_string(),
                            "Error while parsing emi_type value from JSON".to_string(),
                        )]),
                    },
                    Err(_) => Err(vec![]),
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilteredPaymentInfo {
    pub cardBin: Option<String>,
    pub extendedCardBin: Option<Isin>,
    pub cardBrand: Option<String>,
    pub cardIssuer: Option<String>,
    pub cardType: Option<String>,
    pub cardIssuerCountry: Option<String>,
    pub paymentMethod: Option<String>,
    pub paymentMethodType: Option<String>,
    pub authType: Option<String>,
    pub paymentSource: Option<String>,
    pub emiType: Option<String>,
    pub cardSubType: Option<String>,
    pub storedCardProvider: Option<String>,
    pub extendedCardType: Option<String>,
    pub cvvLessTxn: Option<bool>,
    pub juspayBankCode: Option<String>,
    pub cardSubTypeCategory: Option<String>,
    pub countryCode: Option<String>,
}

pub fn make_payment_info(
    txnCardInfo: TxnCardInfo,
    mCardInfo: Option<CardInfo>,
    mInternalMeta: Option<DeciderTypes::InternalMetadata>,
    juspayBankCode: Option<String>,
) -> FilteredPaymentInfo {
    match txnCardInfo.card_isin {
        Some(ref cardIsin) => go_card_isin(
            cardIsin.to_string(),
            txnCardInfo.clone(),
            mCardInfo,
            mInternalMeta,
            juspayBankCode,
        ),
        None => FilteredPaymentInfo {
            paymentMethodType: txnCardInfo
                .card_type
                .as_ref()
                .map(|ct| card_type_to_text(&ct))
                .or_else(|| Some(txnCardInfo.paymentMethodType)),
            paymentMethod: txnCardInfo
                .cardIssuerBankName
                .clone()
                .or_else(|| Some(txnCardInfo.paymentMethod)),
            paymentSource: txnCardInfo.paymentSource,
            cardIssuer: txnCardInfo.cardIssuerBankName,
            cardType: txnCardInfo.card_type.map(|c| card_type_to_text(&c)),
            cardBin: None,
            extendedCardBin: None,
            cardBrand: None,
            cardIssuerCountry: None,
            authType: None,
            emiType: None,
            cardSubType: None,
            storedCardProvider: None,
            extendedCardType: None,
            cvvLessTxn: None,
            juspayBankCode: juspayBankCode
                .or_else(|| Some("".to_string()))
                .map(|j| j.to_uppercase()),
            cardSubTypeCategory: None,
            countryCode: None,
        },
    }
}

fn go_card_isin(
    cardIsin: String,
    txnCardInfo: TxnCardInfo,
    mCardInfo: Option<CardInfo>,
    mInternalMeta: Option<DeciderTypes::InternalMetadata>,
    juspayBankCode: Option<String>,
) -> FilteredPaymentInfo {
    let card_type = mCardInfo
        .clone()
        .as_ref()
        .and_then(|ci| ci.card_type.clone())
        .map(|ct| card_type_to_text(&ct));
    let extended_card_type = mCardInfo
        .as_ref()
        .and_then(|ci| ci.extended_card_type.clone());
    // case Utils.fetchExtendedCardBin txnCardInfo of
    //     Just extendedCardIsin -> (preview ETCa.isinText extendedCardIsin) <|> (.cardIsin) <$> mCardInfo
    //     Nothing -> (.cardIsin) <$> mCardInfo
    let extended_card_bin = utils::fetch_extended_card_bin(&txnCardInfo)
        .map(|card_bin| to_isin(card_bin).ok())
        .unwrap_or(None)
        .or_else(|| mCardInfo.clone().map(|ci| ci.card_isin));
    let card_sub_type_v = mCardInfo
        .clone()
        .map(|ci| ci.card_sub_type)
        .unwrap_or(Some("".to_string()))
        .map(|s| s.to_uppercase());
    let card_sub_type_category = match mCardInfo {
        Some(ref card_info) => match card_info.card_sub_type_category {
            Some(ref card_info_card_sub_type_category) => {
                Some(card_info_card_sub_type_category.clone().to_uppercase())
            }
            None => match utils::get_true_string(card_sub_type_v.clone()) {
                Some(sub_type) => {
                    if sub_type.to_lowercase().contains("business")
                        || sub_type.to_lowercase().contains("corp")
                    {
                        Some("CORPORATE".to_string())
                    } else {
                        Some("RETAIL".to_string())
                    }
                }
                None => None,
            },
        },
        None => None,
    };
    let cloned_txn_card_info = txnCardInfo.clone();

    FilteredPaymentInfo {
        paymentMethodType: Some(CARD.to_string()),
        paymentMethod: txnCardInfo
            .cardSwitchProvider
            .clone()
            .map(|csp| csp.peek().to_uppercase()),
        paymentSource: None,
        cardIssuer: txnCardInfo
            .cardIssuerBankName
            .clone()
            .map(|ci| ci.to_uppercase()),
        cardType: cloned_txn_card_info
            .card_type
            .or(Some(CardType::Credit))
            .map(|ct| card_type_to_text(&ct)),
        cardBin: Some(cardIsin.chars().take(6).collect()),
        extendedCardBin: extended_card_bin,
        cardBrand: cloned_txn_card_info
            .cardSwitchProvider
            .clone()
            .map(|csp| csp.peek().to_uppercase()),
        cardIssuerCountry: mCardInfo
            .as_ref()
            .and_then(|ci| ci.card_issuer_country.clone().map(|c| c.to_uppercase()))
            .or_else(|| Some("".to_string())),
        authType: txnCardInfo
            .authType
            .clone()
            .map(|at| auth_type_to_text(&at)),
        emiType: fetch_emi_type(txnCardInfo.clone())
            .ok()
            .or_else(|| Some("".to_string())),
        cardSubType: card_sub_type_v,
        storedCardProvider: mInternalMeta
            .as_ref()
            .and_then(|im| im.storedCardVaultProvider.clone())
            .or_else(|| Some("JUSPAY".to_string())),
        extendedCardType: extended_card_type
            .or(card_type)
            .map(|ect| ect.to_uppercase())
            .or_else(|| Some("".to_string())),
        cvvLessTxn: mInternalMeta.as_ref().and_then(|im| im.isCvvLessTxn),
        juspayBankCode: juspayBankCode
            .map(|j| j.to_uppercase())
            .or_else(|| Some("".to_string())),
        cardSubTypeCategory: card_sub_type_category,
        countryCode: mCardInfo
            .as_ref()
            .and_then(|ci| ci.country_code.clone())
            .or_else(|| Some("".to_string())),
    }
}

// fn hush<T, E>(result: Result<T, E>) -> Option<T> {
//     result.ok()
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityLogicConfig {
    pub stagger: Option<Stagger>,
    pub activeLogic: String,
    pub fallbackLogic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Stagger {
    StaggerBtwnTwo(BtwnTwo),
    UnhandledText(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BtwnTwo {
    pub staggeredLogic: String,
    pub rollout: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroovyEvalPayload {
    pub orderInfo: FilteredOrderInfo,
    pub txnInfo: FilteredTxnInfo,
    pub paymentInfo: FilteredPaymentInfo,
    pub merchantId: MerchantId,
    pub script: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TenantPLConfig {
    pub name: String,
    pub priorityLogic: String,
    pub priorityLogicRules: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LogEntry {
    Info(String),
    Error(String, String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseBody {
    pub ok: bool,
    pub log: Option<Vec<Vec<String>>>,
    pub result: PriorityLogicOutput,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PriorityLogicOutput {
    pub isEnforcement: Option<bool>,
    pub gatewayPriority: Option<Vec<String>>,
    pub gatewayReferenceIds: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug)]
pub enum EvaluationResult {
    PLResponse(
        DeciderTypes::GatewayPriorityLogicOutput,
        DeciderTypes::PriorityLogicData,
        Vec<LogEntry>,
        DeciderTypes::Status,
    ),
    EvaluationError(DeciderTypes::PriorityLogicData, Vec<LogEntry>),
}

// type API = (
//     ReqBody<GroovyEvalPayload> :> Post<ResponseBody>,
//     "health" :> Post<bool>
// );

// pub fn api() -> API {
//     (ReqBody::<GroovyEvalPayload>::default(), "health".to_string())
// }

// pub fn post_groovy_eval_n(payload: GroovyEvalPayload) -> T::EulerClient<ResponseBody> {
//     ET::client(api()).0(payload)
// }

// pub fn post_groovy_health_n() -> T::EulerClient<bool> {
//     ET::client(api()).1()
// }

pub fn parse_log_entry(log: Vec<String>) -> LogEntry {
    match log.as_slice() {
        [val, descr] if val == "Info" => LogEntry::Info(descr.clone()),
        [val, descr, err] if val == "Error" => LogEntry::Error(descr.clone(), err.clone()),
        _ => LogEntry::Error(
            "Malformed log entry".to_string(),
            "no logs from executor".to_string(),
        ),
    }
}

// pub fn groovy_executor_url() -> SC::BaseUrl {
//     SC::BaseUrl {
//         baseUrlScheme: SC::Http,
//         baseUrlHost: Env::lookup_env(Env::JuspayEnv {
//             key: "GROOVY_RUNNER_HOST".to_string(),
//             actionLeft: Env::mk_default_env_action("euler-groovy-runner.ec-prod.svc.cluster.local".to_string()),
//             decryptFunc: Box::new(|s| Ok(s.to_string())),
//             logWhenThrowException: None,
//         }),
//         baseUrlPort: 80,
//         baseUrlPath: "/evaluate-script".to_string(),
//     }
// }

pub fn pl_execution_retry_failure_reasons() -> Vec<DeciderTypes::PriorityLogicFailure> {
    vec![
        DeciderTypes::PriorityLogicFailure::ConnectionFailed,
        DeciderTypes::PriorityLogicFailure::ResponseContentTypeNotSupported,
        DeciderTypes::PriorityLogicFailure::ResponseDecodeFailure,
        DeciderTypes::PriorityLogicFailure::ResponseParseError,
    ]
}

pub async fn execute_priority_logic(
    req: DeciderTypes::ExecutePriorityLogicRequest,
) -> DeciderTypes::GatewayPriorityLogicOutput {
    let internal_metadata: Option<DeciderTypes::InternalMetadata> = req
        .txnDetail
        .internalMetadata
        .as_ref()
        .and_then(|im| serde_json::from_str(im.peek()).ok());
    let order_metadata = req.orderMetadata.metadata.clone();
    // resolveBin <- case Utils.fetchExtendedCardBin req.txnCardInfo of
    // Just cardBin -> pure (Just cardBin)
    // Nothing -> do
    //   case req.txnCardInfo.cardIsin of
    //     Just cIsin -> do
    //       resBin <- Utils.getCardBinFromTokenBin 6 cIsin
    //       pure $ Just resBin
    //     Nothing -> pure $ req.txnCardInfo.cardIsin
    let resolve_bin = match utils::fetch_extended_card_bin(&req.txnCardInfo) {
        Some(card_bin) => Some(card_bin),
        None => match req.txnCardInfo.card_isin {
            Some(c_isin) => Some(utils::get_card_bin_from_token_bin(6, c_isin.as_str()).await),
            None => None,
        },
    };

    get_gateway_priority(
        req.merchantAccount,
        req.order,
        req.txnDetail,
        TxnCardInfo {
            card_isin: resolve_bin,
            ..req.txnCardInfo
        },
        internal_metadata,
        order_metadata,
        None,
    )
    .await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PLExecutorError {
    pub error: bool,
    #[serde(rename = "error_message")]
    pub errorMessage: DeciderTypes::PriorityLogicFailure,
    #[serde(rename = "user_message")]
    pub userMessage: String,
    pub log: Option<Vec<Vec<String>>>,
}

pub async fn get_gateway_priority(
    macc: MerchantAccount,
    order: Order,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    m_internal_meta: Option<DeciderTypes::InternalMetadata>,
    order_meta_data: Option<String>,
    priority_logic_script_m: Option<String>,
) -> DeciderTypes::GatewayPriorityLogicOutput {
    let m_card_info = utils::get_card_info_by_bin(txn_card_info.card_isin.clone()).await;
    if macc.useCodeForGatewayPriority {
        let evaluate_script = |script, tag| {
            eval_script(
                order.clone(),
                txn_detail.clone(),
                txn_card_info.clone(),
                m_card_info.clone(),
                macc.merchantId.clone(),
                script,
                m_internal_meta.clone(),
                order_meta_data.clone(),
                tag,
            )
        };
        let (script, priority_logic_tag) = get_script(macc.clone(), priority_logic_script_m).await;
        let result = evaluate_script(script.clone(), priority_logic_tag.clone()).await;

        match result {
            EvaluationResult::PLResponse(gws, pl_data, logs, status) => {
                logger::info!(
                    tag = "PRIORITY_LOGIC_EXECUTION_",
                    "MerchantId: {:?} , Gateways: {:?}, Logs: {:?}",
                    macc.merchantId.clone(),
                    gws,
                    logs
                );
                DeciderTypes::GatewayPriorityLogicOutput {
                    is_enforcement: gws.is_enforcement,
                    gws: gws.gws,
                    priority_logic_tag: gws.priority_logic_tag,
                    gateway_reference_ids: gws.gateway_reference_ids,
                    primary_logic: Some(pl_data),
                    fallback_logic: gws.fallback_logic,
                }
            }
            EvaluationResult::EvaluationError(priority_logic_data, err) => {
                logger::info!(
                    tag = "PRIORITY_LOGIC_EXECUTION_FAILURE",
                    "MerchantId: {:?}, Error: {:?}",
                    macc.merchantId,
                    err
                );
                if pl_execution_retry_failure_reasons()
                    .contains(&priority_logic_data.failure_reason.clone())
                {
                    let retry_result = evaluate_script(script, priority_logic_tag.clone()).await;
                    match retry_result {
                        EvaluationResult::PLResponse(retry_gws, retry_pl_data, logs, status) => {
                            let tag = format!("PRIORITY_LOGIC_EXECUTION_RETRY_{}", status);
                            logger::info!(
                                tag = %tag,
                                "MerchantId: {:?} , Gateways: {:?}, Logs: {:?}",
                                macc.merchantId,
                                retry_gws,
                                logs
                            );
                            DeciderTypes::GatewayPriorityLogicOutput {
                                is_enforcement: retry_gws.is_enforcement,
                                gws: retry_gws.gws,
                                priority_logic_tag: retry_gws.priority_logic_tag,
                                gateway_reference_ids: retry_gws.gateway_reference_ids,
                                primary_logic: Some(retry_pl_data),
                                fallback_logic: retry_gws.fallback_logic,
                            }
                        }
                        EvaluationResult::EvaluationError(retry_pl_data, err) => {
                            logger::info!(
                                tag = "PRIORITY_LOGIC_EXECUTION_RETRY_FAILURE",
                                "MerchantId: {:?} , Error: {:?}",
                                macc.merchantId,
                                err
                            );
                            handle_fallback_logic(
                                macc,
                                order,
                                txn_detail,
                                txn_card_info,
                                m_card_info,
                                m_internal_meta,
                                order_meta_data,
                                default_gateway_priority_logic_output()
                                    .set_priority_logic_tag(priority_logic_tag)
                                    .set_primary_logic(Some(retry_pl_data))
                                    .build(),
                                priority_logic_data.failure_reason.clone(),
                            )
                            .await
                        }
                    }
                } else {
                    {
                        handle_fallback_logic(
                            macc,
                            order,
                            txn_detail,
                            txn_card_info,
                            m_card_info,
                            m_internal_meta,
                            order_meta_data,
                            default_gateway_priority_logic_output()
                                .set_priority_logic_tag(priority_logic_tag)
                                .set_primary_logic(Some(priority_logic_data.clone()))
                                .build(),
                            priority_logic_data.failure_reason.clone(),
                        )
                        .await
                    }
                }
            }
        }
    } else {
        match macc.gatewayPriority {
            None => default_gateway_priority_logic_output(),
            Some(t) => {
                if t.is_empty() {
                    logger::info!(
                        tag = "gatewayPriority",
                        "gatewayPriority for merchant: {:?} is empty.",
                        macc.merchantId
                    );
                    default_gateway_priority_logic_output()
                } else {
                    let list_of_gateway = t
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    logger::info!(
                        tag = "gatewayPriority",
                        "gatewayPriority for merchant: {:?} listOfGateway: {:?}",
                        macc.merchantId,
                        list_of_gateway
                    );
                    match list_of_gateway.as_slice() {
                        [] => {
                            logger::info!(
                                tag = "gatewayPriority emptyList",
                                "Can't get gatewayPriority for merchant: {:?} . Input: {:?}",
                                macc.merchantId,
                                t
                            );
                            default_gateway_priority_logic_output()
                        }
                        res => {
                            logger::info!(
                                tag = "gatewayPriority decoding",
                                "Decoded successfully. Input: {:?} output: {:?}",
                                t,
                                res
                            );
                            default_gateway_priority_logic_output()
                                .set_gws(res.to_vec())
                                .build()
                        }
                    }
                }
            }
        }
    }
}

async fn get_script(
    macc: MerchantAccount,
    maybe_script: Option<String>,
) -> (String, Option<String>) {
    match maybe_script {
        Some(script) => (script, Some("TEST_PL".to_string())),
        None => get_priority_logic_script(&macc).await,
    }
}

fn default_gateway_priority_logic_output() -> DeciderTypes::GatewayPriorityLogicOutput {
    DeciderTypes::GatewayPriorityLogicOutput {
        is_enforcement: false,
        gws: vec![],
        priority_logic_tag: None,
        gateway_reference_ids: std::collections::HashMap::new(),
        primary_logic: None,
        fallback_logic: None,
    }
}

async fn get_priority_logic_script(macc: &MerchantAccount) -> (String, Option<String>) {
    match macc.priorityLogicConfig.clone() {
        Some(priority_logic_config) => {
            match utils::either_decode_t::<PriorityLogicConfig>(&priority_logic_config) {
                Ok(pl_config) => {
                    let mpl_id = match pl_config.stagger {
                        Some(Stagger::StaggerBtwnTwo(BtwnTwo {
                            staggeredLogic,
                            rollout,
                        })) => {
                            if roller(
                                "GatewayDecider::getPriorityLogicScript".to_string(),
                                rollout,
                            ) {
                                staggeredLogic
                            } else {
                                pl_config.activeLogic
                            }
                        }
                        Some(Stagger::UnhandledText(_)) => pl_config.activeLogic,
                        None => pl_config.activeLogic,
                    };
                    match find_priority_logic_by_id(
                        mpl_id.parse().expect("Id is not able to convert"),
                    )
                    .await
                    {
                        Some(mpl) => (mpl.priorityLogic, mpl.name),
                        None => {
                            logger::error!(
                                tag = "getPriorityLogicScript",
                                "No merchant_priority_logic found for id {}",
                                mpl_id
                            );
                            let pl_tag = get_active_priority_logic_name(macc).await;
                            (macc.gatewayPriorityLogic.clone(), pl_tag)
                        }
                    }
                }
                Err(err) => {
                    logger::error!(
                        tag = "getPriorityLogicScript",
                        "Error while decoding PriorityLogicConfig for {:?} {}",
                        macc.merchantId,
                        err
                    );
                    let pl_tag = get_active_priority_logic_name(macc).await;
                    (macc.gatewayPriorityLogic.clone(), pl_tag)
                }
            }
        }
        None => {
            if macc.gatewayPriorityLogic.trim().is_empty() {
                get_priority_logic_script_from_tenant_config(macc).await
            } else {
                let pl_tag = get_active_priority_logic_name(macc).await;
                (macc.gatewayPriorityLogic.clone(), pl_tag)
            }
        }
    }
}

async fn get_active_priority_logic_name(macc: &MerchantAccount) -> Option<String> {
    match macc.clone().internalMetadata {
        Some(metadata) => match utils::get_value("active_priority_logic_name", &metadata) {
            Some(name) => Some(name),
            None => get_active_priority_logic_name_from_db(macc).await,
        },
        None => get_active_priority_logic_name_from_db(macc).await,
    }
}

async fn get_active_priority_logic_name_from_db(macc: &MerchantAccount) -> Option<String> {
    let all_mpls = find_all_priority_logic_by_merchant_pid(merchant_pid_to_text(macc.id)).await;
    match all_mpls.iter().find(|mpl: &_| mpl.isActiveLogic) {
        Some(mpl) => mpl.name.clone(),
        None => None,
    }
}

async fn get_priority_logic_script_from_tenant_config(
    macc: &MerchantAccount,
) -> (String, Option<String>) {
    match macc.tenantAccountId {
        Some(ref tenant_account_id) => {
            match get_tenant_config_by_tenant_id_and_module_name_and_module_key_and_type(
                tenant_account_id.to_string(),
                ModuleName::PriorityLogic,
                "priority_logic".to_string(),
                ConfigType::FALLBACK,
            )
            .await
            {
                Some(tenant_config) => {
                    match (tenant_config.filterDimension, tenant_config.filterGroupId) {
                        (Some(filter_dimension), Some(filter_group_id)) => {
                            logger::info!(
                                tag = "getPriorityLogicScriptFromTenantConfig",
                                "Filter dimension found: {:?}",
                                filter_dimension
                            );
                            get_pl_by_filter_dimension(
                                macc,
                                filter_dimension,
                                filter_group_id.clone(),
                                tenant_config.configValue,
                            )
                            .await
                        }
                        _ => {
                            logger::info!(
                                tag = "getPriorityLogicScriptFromTenantConfig",
                                "Filter dimension and filter groupId are not present. Proceeding with default tenant config value."
                            );
                            decode_tenant_pl_config(tenant_config.configValue, macc).await
                        }
                    }
                }
                None => {
                    let tenant_account_id = macc.tenantAccountId.clone().unwrap_or_default();
                    logger::info!(
                        tag = "getPriorityLogicScriptFromTenantConfig",
                        "Tenant Config not found for tenant account id {}",
                        tenant_account_id
                    );
                    (String::new(), None)
                }
            }
        }
        None => (String::new(), None),
    }
}

async fn get_pl_by_filter_dimension(
    macc: &MerchantAccount,
    filter_dimension: FilterDimension,
    filter_group_id: String,
    config_value: String,
) -> (String, Option<String>) {
    match filter_dimension {
        FilterDimension::MCC => {
            get_pl_by_merchant_category_code(macc, filter_group_id, config_value).await
        }
        _ => {
            logger::info!(
                tag = "getPLByFilterDimension",
                "Filter dimension is not supported. Proceeding with default tenant config value."
            );
            decode_tenant_pl_config(config_value, macc).await
        }
    }
}

async fn get_pl_by_merchant_category_code(
    macc: &MerchantAccount,
    filter_group_id: String,
    config_value: String,
) -> (String, Option<String>) {
    match &macc.merchantCategoryCode {
        Some(mcc) => {
            match get_tenant_config_filter_by_group_id_and_dimension_value(
                filter_group_id.clone(),
                mcc.to_string(),
            )
            .await
            {
                Some(tenant_config_filter) => {
                    logger::info!(
                        tag = "getPLByMerchantCategoryCode",
                        "Proceeding with tenant config filter priority logic."
                    );
                    decode_tenant_pl_config(tenant_config_filter.configValue, macc).await
                }
                None => {
                    logger::info!(
                        tag = "getPLByMerchantCategoryCode",
                        "Unable to find tenant config filter for groupId {:?} and dimension value {}",
                        filter_group_id,
                        mcc
                    );
                    decode_tenant_pl_config(config_value, macc).await
                }
            }
        }
        None => {
            logger::info!(
                tag = "getPLByMerchantCategoryCode",
                "Merchant category code is not present for merchantId {:?}",
                macc.merchantId
            );
            decode_tenant_pl_config(config_value, macc).await
        }
    }
}

async fn decode_tenant_pl_config(
    config_value: String,
    macc: &MerchantAccount,
) -> (String, Option<String>) {
    match utils::either_decode_t::<TenantPLConfig>(&config_value) {
        Ok(tenant_pl_config) => {
            logger::info!(
                tag = "decodeTenantPLConfig",
                "Tenant Priority Logic Config decoded successfully with name: {}",
                tenant_pl_config.name
            );
            (tenant_pl_config.priorityLogic, Some(tenant_pl_config.name))
        }
        Err(err) => {
            logger::error!(
                tag = "decodeTenantPLConfig",
                "Error while decoding TenantPLConfig for merchantId {:?}: {}",
                macc.merchantId.clone(),
                err
            );
            (String::new(), None)
        }
    }
}

pub async fn get_fallback_priority_logic_script(
    macc: &MerchantAccount,
) -> (Option<String>, Option<String>) {
    match &macc.priorityLogicConfig {
        Some(priority_logic_config) => {
            match utils::either_decode_t::<PriorityLogicConfig>(priority_logic_config) {
                Ok(pl_config) => {
                    let mpl_m = match pl_config.fallbackLogic {
                        Some(mpl_id) => {
                            find_priority_logic_by_id(mpl_id.parse().expect("Invalid mid ")).await
                        }
                        None => None,
                    };
                    match mpl_m {
                        Some(mpl) => (Some(mpl.priorityLogic), mpl.name),
                        None => (None, None),
                    }
                }
                Err(err) => {
                    logger::error!(
                        tag = "getFallbackPriorityLogicScript",
                        "Error while decoding PriorityLogicConfig for merchantId {:?}: {}",
                        macc.merchantId,
                        err
                    );
                    (None, None)
                }
            }
        }
        None => (None, None),
    }
}

pub async fn handle_fallback_logic(
    macc: MerchantAccount,
    order: Order,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    m_card_info: Option<CardInfo>,
    m_internal_meta: Option<DeciderTypes::InternalMetadata>,
    order_meta_data: Option<String>,
    primary_logic_output: DeciderTypes::GatewayPriorityLogicOutput,
    pl_failure_reason: DeciderTypes::PriorityLogicFailure,
) -> DeciderTypes::GatewayPriorityLogicOutput {
    if primary_logic_output.fallback_logic.is_none() && primary_logic_output.primary_logic.is_some() {
        let (fallback_logic, fallback_pl_tag) = get_fallback_priority_logic_script(&macc).await;
        match fallback_logic {
            Some(fallback_script) => {
                let fallback_result = eval_script(
                    order,
                    txn_detail,
                    txn_card_info,
                    m_card_info,
                    macc.merchantId.clone(),
                    fallback_script,
                    m_internal_meta,
                    order_meta_data,
                    fallback_pl_tag.clone(),
                )
                .await;
                match fallback_result {
                    EvaluationResult::PLResponse(gws, pl_data, logs, status) => {
                        logger::info!(
                            tag = "FALLBACK_PRIORITY_LOGIC_EXECUTION_",
                            "MerchantId: {:?} , Gateways: {:?}, Logs: {:?}",
                            macc.merchantId.clone(),
                            gws,
                            logs
                        );
                        DeciderTypes::GatewayPriorityLogicOutput {
                            fallback_logic: Some(pl_data),
                            priority_logic_tag: fallback_pl_tag,
                            primary_logic: check_and_update_pl_failure_reason(
                                primary_logic_output.primary_logic,
                                pl_failure_reason,
                            ),
                            ..primary_logic_output
                        }
                    }
                    EvaluationResult::EvaluationError(priority_logic_data, err) => {
                        logger::info!(
                            tag = "FALLBACK_PRIORITY_LOGIC_EXECUTION_FAILURE",
                            "MerchantId: {:?} , Error: {:?}",
                            macc.merchantId,
                            err
                        );
                        DeciderTypes::GatewayPriorityLogicOutput {
                            primary_logic: check_and_update_pl_failure_reason(
                                primary_logic_output.primary_logic,
                                pl_failure_reason,
                            ),
                            fallback_logic: Some(priority_logic_data),
                            priority_logic_tag: fallback_pl_tag,
                            ..primary_logic_output
                        }
                    }
                }
            }
            None => DeciderTypes::GatewayPriorityLogicOutput {
                primary_logic: check_and_update_pl_failure_reason(
                    primary_logic_output.primary_logic,
                    pl_failure_reason,
                ),
                ..primary_logic_output
            },
        }
    } else {
        DeciderTypes::GatewayPriorityLogicOutput {
            fallback_logic: check_and_update_pl_failure_reason(
                primary_logic_output.fallback_logic,
                pl_failure_reason,
            ),
            ..primary_logic_output
        }
    }
}

fn check_and_update_pl_failure_reason(
    primary_pl_data: Option<DeciderTypes::PriorityLogicData>,
    pl_failure_reason: DeciderTypes::PriorityLogicFailure,
) -> Option<DeciderTypes::PriorityLogicData> {
    match primary_pl_data {
        None => None,
        Some(mut data) => {
            if data.failure_reason != pl_failure_reason {
                data.status = DeciderTypes::Status::Failure;
                data.failure_reason = pl_failure_reason;
            }
            Some(data)
        }
    }
}

pub async fn eval_script(
    order: Order,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    m_card_info: Option<CardInfo>,
    merch_id: MerchantId,
    script: String,
    m_internal_meta: Option<DeciderTypes::InternalMetadata>,
    meta_data: Option<String>,
    priority_logic_tag: Option<String>,
) -> EvaluationResult {
    // Parse metadata into JSON if available
    let order_meta_data = meta_data
        .as_ref()
        .and_then(|md| utils::parse_json_from_string(md));

    // Extract Juspay bank code from internal metadata
    let juspay_bank_code = utils::get_juspay_bank_code_from_internal_metadata(&txn_detail);

    // Prepare the payload for the API call
    let payload = serde_json::json!({
        "orderInfo": filter_order(order, order_meta_data),
        "txnInfo": filter_txn(txn_detail),
        "paymentInfo": make_payment_info(txn_card_info, m_card_info, m_internal_meta, juspay_bank_code),
        "merchantId": merch_id,
        "script": script,
    });

    // Call the API
    let response = call_api(
        &format!("http://{}/evaluate-script", groovy_executor_url()),
        &payload,
    )
    .await;

    // Handle the response
    handle_response(response, priority_logic_tag).await
}

async fn handle_response(
    response: Result<ResponseBody, ApiClientError>,
    priority_logic_tag: Option<String>,
) -> EvaluationResult {
    match response {
        Err(client_error) => {
            let pl_resp = handle_client_error(client_error);
            let log_entries = pl_resp
                .log
                .unwrap_or_default()
                .into_iter()
                .map(parse_log_entry)
                .collect();
            let pl_data = DeciderTypes::PriorityLogicData {
                name: priority_logic_tag,
                status: DeciderTypes::Status::Failure,
                failure_reason: pl_resp.errorMessage,
            };
            EvaluationResult::EvaluationError(pl_data, log_entries)
        }
        Ok(response_body) => {
            let log_entries = response_body
                .clone()
                .log
                .unwrap_or_default()
                .into_iter()
                .map(parse_log_entry)
                .collect();
            if !response_body.ok {
                handle_failure_response(priority_logic_tag, log_entries).await
            } else {
                handle_success_response(response_body, priority_logic_tag, log_entries).await
            }
        }
    }
}

async fn handle_failure_response(
    priority_logic_tag: Option<String>,
    log_entries: Vec<LogEntry>,
) -> EvaluationResult {
    let pl_data = DeciderTypes::PriorityLogicData {
        name: priority_logic_tag,
        status: DeciderTypes::Status::Failure,
        failure_reason: DeciderTypes::PriorityLogicFailure::PlEvaluationFailed,
    };
    EvaluationResult::EvaluationError(pl_data, log_entries)
}

async fn handle_success_response(
    response_body: ResponseBody,
    priority_logic_tag: Option<String>,
    log_entries: Vec<LogEntry>,
) -> EvaluationResult {
    let gws = response_body.result.gatewayPriority.unwrap_or_default();
    let status = DeciderTypes::Status::Success;
    let pl_data = DeciderTypes::PriorityLogicData {
        name: priority_logic_tag.clone(),
        status: status.clone(),
        failure_reason: DeciderTypes::PriorityLogicFailure::NoError,
    };
    let pl_output = DeciderTypes::GatewayPriorityLogicOutput {
        is_enforcement: response_body.result.isEnforcement.unwrap_or(false),
        gws,
        priority_logic_tag: priority_logic_tag.clone(),
        gateway_reference_ids: response_body.result.gatewayReferenceIds.unwrap_or_default(),
        primary_logic: None,
        fallback_logic: None,
    };
    EvaluationResult::PLResponse(pl_output, pl_data, log_entries, status)
}

fn handle_client_error(client_error: ApiClientError) -> PLExecutorError {
    match client_error {
        ApiClientError::BadRequest(bytes) => PLExecutorError {
            error: true,
            errorMessage: DeciderTypes::PriorityLogicFailure::CompilationError,
            userMessage: "Bad request sent to the server.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                "Bad request".to_string(),
                String::from_utf8_lossy(&bytes).to_string(),
            ]]),
        },
        ApiClientError::Unauthorized(bytes) => PLExecutorError {
            error: true,
            errorMessage: DeciderTypes::PriorityLogicFailure::ConnectionFailed,
            userMessage: "Unauthorized access.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                "Unauthorized access".to_string(),
                String::from_utf8_lossy(&bytes).to_string(),
            ]]),
        },
        ApiClientError::InternalServerError(bytes) => PLExecutorError {
            error: true,
            errorMessage: DeciderTypes::PriorityLogicFailure::UnhandledException,
            userMessage: "Internal server error occurred.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                "Internal server error".to_string(),
                String::from_utf8_lossy(&bytes).to_string(),
            ]]),
        },
        ApiClientError::ResponseDecodingFailed => PLExecutorError {
            error: true,
            errorMessage: DeciderTypes::PriorityLogicFailure::ResponseDecodeFailure,
            userMessage: "Failed to decode the response.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                "Response decoding failed".to_string(),
                "Failed to parse the response.".to_string(),
            ]]),
        },
        ApiClientError::RequestNotSent => PLExecutorError {
            error: true,
            errorMessage: DeciderTypes::PriorityLogicFailure::ConnectionFailed,
            userMessage: "The request was not sent.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                "Request not sent".to_string(),
                "The request could not be sent.".to_string(),
            ]]),
        },
        ApiClientError::Unexpected {
            status_code,
            message,
        } => PLExecutorError {
            error: true,
            errorMessage: DeciderTypes::PriorityLogicFailure::UnhandledException,
            userMessage: "An unexpected error occurred.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                format!("Unexpected error: {}", status_code),
                String::from_utf8_lossy(&message).to_string(),
            ]]),
        },
        _ => PLExecutorError {
            error: true,
            errorMessage: DeciderTypes::PriorityLogicFailure::UnhandledException,
            userMessage: "An unknown error occurred.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                "Unknown error".to_string(),
                "No additional details available.".to_string(),
            ]]),
        },
    }
}
