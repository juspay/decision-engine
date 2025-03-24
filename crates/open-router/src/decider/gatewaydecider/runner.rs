use serde::{Serialize, Deserialize};
use serde_json::Value as AValue;
use eulerhs::prelude::*;
use data::aeson::{Object, either_decode, (.:)};
use data::aeson::types::parse_either;
use data::byte_string::lazy as BSL;
use data::maybe::from_just;
use data::reflection::give;
use data::text as DT::{is_infix_of, pack, to_upper, strip, to_lower};
use types::card::{TxnCardInfo, card_type_to_text};
use types::card::card_info::{CardInfo};
use types::currency::Currency;
use types::customer::CustomerId;
use types::gateway::{text_to_gateway};
use types::merchant::MerchantId;
use types::money::{Money, to_double};
use types::order::{Order, OrderId, ProductId, get_udf};
use juspay::extra::json::{decode_json, encode_json};
use juspay::extra::parsing::{Parsed, parse};
use juspay::extra::secret::{SecretContext, unsafe_extract_secret};
use types::transaction::TransactionId;
use types::txn_detail::TxnDetail;
use eulerhs::api_helpers as ET;
use eulerhs::language::{MonadFlow, call_api};
use servant::{JSON, Post, ReqBody, (:<|>), (:>)};
use utils::api_tag;
use types::gateway::{gateway_to_text, text_to_gateway};
use types::card::card_info::{CardInfo};
use types::card::{Isin};
use optics::core::preview;
use types::gateway::Gateway;
use servant::client::{ClientError, ResponseF};
use types::merchant_priority_logic as MPL;
use gateway_decider::utils as Utils;
use data::aeson as A;
use juspay::extra::env as Env;
use types::card as ETCa;
use eulerhs::types as T;
use eulerhs::api_helpers as T;
use network::http::client as HC;
use servant::client as SC;
use eulerhs::language as L;
use gateway_decider::types as DeciderTypes;
use gateway_decider::types::{GatewayPriorityLogicOutput, PriorityLogicData};
use data::text::encoding as TE;
use types::txn_detail as ETTD;
use types::token_bin_info as ETTB;
use data::text as T;
use types::merchant as ETM;
use types::order as ETO;
use utils::redis::feature as Feature;
use juspay::extra::json as JSON;
use types::tenant_config as TenantConfig;
use types::tenant_config_filter as TenantConfigFilter;

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
    pub orderMetaData: Option<AValue>,
}

pub fn filter_order(order: Order, metaData: Option<AValue>) -> FilteredOrderInfo {
    FilteredOrderInfo {
        amount: to_double(order.amount),
        currency: order.currency,
        customerId: order.customerId,
        orderId: order.orderId,
        productId: order.productId,
        description: order.description,
        preferredGateway: order.preferredGateway.map(gateway_to_text),
        udf1: get_udf(1, order.udfs),
        udf2: get_udf(2, order.udfs),
        udf3: get_udf(3, order.udfs),
        udf4: get_udf(4, order.udfs),
        udf5: get_udf(5, order.udfs),
        udf6: get_udf(6, order.udfs),
        udf7: get_udf(7, order.udfs),
        udf8: get_udf(8, order.udfs),
        udf9: get_udf(9, order.udfs),
        udf10: get_udf(10, order.udfs),
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
    pub txnObjectType: ETTD.TxnObjectType,
}

pub fn filter_txn(detail: TxnDetail) -> FilteredTxnInfo {
    FilteredTxnInfo {
        isEmi: detail.isEmi,
        emiBank: detail.emiBank,
        emiTenure: detail.emiTenure,
        txnId: detail.txnId,
        addToLocker: detail.addToLocker,
        expressCheckout: detail.expressCheckout,
        sourceObject: detail.sourceObject,
        txnObjectType: detail.txnObjectType,
    }
}

pub fn fetch_emi_type(txnCardInfo: TxnCardInfo) -> Result<String, Vec<LogEntry>> {
    match txnCardInfo.paymentSource {
        None => Err(vec![]),
        Some(ps) => {
            if !is_infix_of("emi_type", &ps) {
                Err(vec![])
            } else {
                match decode_json::<Object>(&ps) {
                    None => Err(vec![]),
                    Some(obj) => parse_either(&obj, "emi_type").map_err(|e| vec![LogEntry::Error("Error while parsing emi_type value from JSON".to_string(), e.to_string())]),
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

pub fn make_payment_info(txnCardInfo: TxnCardInfo, mCardInfo: Option<CardInfo>, mInternalMeta: Option<DeciderTypes::InternalMetadata>, juspayBankCode: Option<String>) -> FilteredPaymentInfo {
    match txnCardInfo.cardIsin {
        Some(cardIsin) => go_card_isin(cardIsin, txnCardInfo, mCardInfo, mInternalMeta, juspayBankCode),
        None => FilteredPaymentInfo {
            paymentMethodType: txnCardInfo.cardType.map(card_type_to_text).or_else(|| Some(txnCardInfo.paymentMethodType.to_string())),
            paymentMethod: txnCardInfo.cardIssuerBankName.or_else(|| Some(txnCardInfo.paymentMethod)),
            paymentSource: txnCardInfo.paymentSource,
            cardIssuer: txnCardInfo.cardIssuerBankName,
            cardType: txnCardInfo.cardType.map(card_type_to_text),
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
            juspayBankCode: juspayBankCode.map(to_upper).or_else(|| Some("".to_string())),
            cardSubTypeCategory: None,
            countryCode: None,
        },
    }
}

fn go_card_isin(cardIsin: String, txnCardInfo: TxnCardInfo, mCardInfo: Option<CardInfo>, mInternalMeta: Option<DeciderTypes::InternalMetadata>, juspayBankCode: Option<String>) -> FilteredPaymentInfo {
    let card_type = mCardInfo.as_ref().and_then(|ci| ci.cardType).map(card_type_to_text);
    let extended_card_type = mCardInfo.as_ref().and_then(|ci| ci.extendedCardType);
    let extended_card_bin = Utils::fetch_extended_card_bin(&txnCardInfo).or_else(|| mCardInfo.as_ref().and_then(|ci| ci.cardIsin));
    let card_sub_type_v = mCardInfo.as_ref().and_then(|ci| ci.cardSubType).map(to_upper).or_else(|| Some("".to_string()));
    let card_sub_type_category = match mCardInfo {
        Some(ref card_info) => match card_info.cardSubTypeCategory {
            Some(ref card_info_card_sub_type_category) => Some(to_upper(card_info_card_sub_type_category.clone())),
            None => match Utils::get_true_string(&card_sub_type_v) {
                Some(sub_type) => {
                    if is_infix_of("business", &to_lower(&sub_type)) || is_infix_of("corp", &to_lower(&sub_type)) {
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

    FilteredPaymentInfo {
        paymentMethodType: Some("CARD".to_string()),
        paymentMethod: txnCardInfo.cardSwitchProvider.map(|csp| to_upper(unsafe_extract_secret(&csp))),
        paymentSource: None,
        cardIssuer: txnCardInfo.cardIssuerBankName.map(to_upper),
        cardType: txnCardInfo.cardType.or_else(|| Some(ETCa::Credit)).map(card_type_to_text),
        cardBin: Some(cardIsin.chars().take(6).collect()),
        extendedCardBin: extended_card_bin,
        cardBrand: txnCardInfo.cardSwitchProvider.map(|csp| to_upper(unsafe_extract_secret(&csp))),
        cardIssuerCountry: mCardInfo.as_ref().and_then(|ci| ci.cardIssuerCountry).map(to_upper).or_else(|| Some("".to_string())),
        authType: txnCardInfo.authType.map(|at| give(RiskyShowSecrets, || at.to_string())),
        emiType: fetch_emi_type(txnCardInfo).ok().or_else(|| Some("".to_string())),
        cardSubType: card_sub_type_v,
        storedCardProvider: mInternalMeta.as_ref().and_then(|im| im.storedCardVaultProvider.clone()).or_else(|| Some("JUSPAY".to_string())),
        extendedCardType: extended_card_type.or_else(|| card_type).map(to_upper).or_else(|| Some("".to_string())),
        cvvLessTxn: mInternalMeta.as_ref().and_then(|im| im.isCvvLessTxn),
        juspayBankCode: juspayBankCode.map(to_upper).or_else(|| Some("".to_string())),
        cardSubTypeCategory: card_sub_type_category,
        countryCode: mCardInfo.as_ref().and_then(|ci| ci.countryCode.clone()).or_else(|| Some("".to_string())),
    }
}

fn hush<T, E>(result: Result<T, E>) -> Option<T> {
    result.ok()
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody {
    pub ok: bool,
    pub log: Option<Vec<Vec<String>>>,
    pub result: PriorityLogicOutput,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityLogicOutput {
    pub isEnforcement: Option<bool>,
    pub gatewayPriority: Option<Vec<String>>,
    pub gatewayReferenceIds: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug)]
pub enum EvaluationResult {
    PLResponse(DeciderTypes::GatewayPriorityLogicOutput, DeciderTypes::PriorityLogicData, Vec<LogEntry>, DeciderTypes::Status),
    EvaluationError(DeciderTypes::PriorityLogicData, Vec<LogEntry>),
}

type API = (
    ReqBody<GroovyEvalPayload> :> Post<ResponseBody>,
    "health" :> Post<bool>
);

pub fn api() -> API {
    (ReqBody::<GroovyEvalPayload>::default(), "health".to_string())
}

pub fn post_groovy_eval_n(payload: GroovyEvalPayload) -> T::EulerClient<ResponseBody> {
    ET::client(api()).0(payload)
}

pub fn post_groovy_health_n() -> T::EulerClient<bool> {
    ET::client(api()).1()
}

pub fn parse_log_entry(log: Vec<String>) -> LogEntry {
    match log.as_slice() {
        ["Info", descr] => LogEntry::Info(descr.clone()),
        ["Error", descr, err] => LogEntry::Error(descr.clone(), err.clone()),
        _ => LogEntry::Error("Malformed log entry".to_string(), "no logs from executor".to_string()),
    }
}

pub fn groovy_executor_url() -> SC::BaseUrl {
    SC::BaseUrl {
        baseUrlScheme: SC::Http,
        baseUrlHost: Env::lookup_env(Env::JuspayEnv {
            key: "GROOVY_RUNNER_HOST".to_string(),
            actionLeft: Env::mk_default_env_action("euler-groovy-runner.ec-prod.svc.cluster.local".to_string()),
            decryptFunc: Box::new(|s| Ok(s.to_string())),
            logWhenThrowException: None,
        }),
        baseUrlPort: 80,
        baseUrlPath: "/evaluate-script".to_string(),
    }
}

pub fn pl_execution_retry_failure_reasons() -> Vec<DeciderTypes::PriorityLogicFailure> {
    vec![DeciderTypes::PriorityLogicFailure::CONNECTION_FAILED]
}

pub async fn execute_priority_logic(req: DeciderTypes::ExecutePriorityLogicRequest) -> DeciderTypes::GatewayPriorityLogicOutput {
    let internal_metadata = req.txnDetail.internalMetadata.as_ref().and_then(|im| A::decode::<DeciderTypes::InternalMetadata>(BSL::from_str(&TE::encode_utf8(im))));
    let order_metadata = req.orderMetadata.metadata.clone();
    let resolve_bin = match Utils::fetch_extended_card_bin(&req.txnCardInfo) {
        Some(card_bin) => Some(card_bin),
        None => req.txnCardInfo.cardIsin.clone(),
    };

    get_gateway_priority(
        req.merchantAccount,
        req.order,
        req.txnDetail,
        TxnCardInfo {
            cardIsin: resolve_bin,
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
    macc: ETM::MerchantAccount,
    order: ETO::Order,
    txn_detail: ETTD::TxnDetail,
    txn_card_info: TxnCardInfo,
    m_internal_meta: Option<DeciderTypes::InternalMetadata>,
    order_meta_data: Option<String>,
    priority_logic_script_m: Option<String>,
) -> Result<DeciderTypes::GatewayPriorityLogicOutput, Box<dyn std::error::Error>> {
    let m_card_info = Utils::get_card_info_by_bin(txn_card_info.card_isin).await?;
    if macc.use_code_for_gateway_priority {
        let evaluate_script = |script, tag| {
            eval_script(
                order.clone(),
                txn_detail.clone(),
                txn_card_info.clone(),
                m_card_info.clone(),
                macc.merchant_id.clone(),
                script,
                m_internal_meta.clone(),
                order_meta_data.clone(),
                tag,
            )
        };
        let (script, priority_logic_tag) = get_script(macc.clone(), priority_logic_script_m).await?;
        let result = evaluate_script(script.clone(), priority_logic_tag.clone()).await?;
        match result {
            EvaluationResult::PLResponse(gws, pl_data, logs, status) => {
                L::log_info_v(
                    format!("PRIORITY_LOGIC_EXECUTION_{}", status),
                    format!(
                        "MerchantId: {}, Gateways: {:?}, Logs: {:?}",
                        macc.merchant_id, gws, logs
                    ),
                )
                .await;
                Ok(gws.with_primary_logic(Some(pl_data)))
            }
            EvaluationResult::EvaluationError(priority_logic_data, err) => {
                L::log_error_v(
                    "PRIORITY_LOGIC_EXECUTION_FAILURE",
                    format!(
                        "MerchantId: {}, Error: {}",
                        macc.merchant_id, err
                    ),
                )
                .await;
                if pl_execution_retry_failure_reasons.contains(&priority_logic_data.failure_reason) {
                    let retry_result = evaluate_script(script, priority_logic_tag.clone()).await?;
                    match retry_result {
                        EvaluationResult::PLResponse(retry_gws, retry_pl_data, logs, status) => {
                            L::log_info_v(
                                format!("PRIORITY_LOGIC_EXECUTION_RETRY_{}", status),
                                format!(
                                    "MerchantId: {}, Gateways: {:?}, Logs: {:?}",
                                    macc.merchant_id, retry_gws, logs
                                ),
                            )
                            .await;
                            Ok(retry_gws.with_primary_logic(Some(retry_pl_data)))
                        }
                        EvaluationResult::EvaluationError(retry_pl_data, err) => {
                            L::log_error_v(
                                "PRIORITY_LOGIC_EXECUTION_RETRY_FAILURE",
                                format!(
                                    "MerchantId: {}, Error: {}",
                                    macc.merchant_id, err
                                ),
                            )
                            .await;
                            handle_fallback_logic(
                                macc,
                                order,
                                txn_detail,
                                txn_card_info,
                                m_card_info,
                                m_internal_meta,
                                order_meta_data,
                                default_gateway_priority_logic_output()
                                    .with_priority_logic_tag(priority_logic_tag)
                                    .with_primary_logic(Some(retry_pl_data)),
                                retry_pl_data.failure_reason,
                            )
                            .await
                        }
                    }
                } else {
                    handle_fallback_logic(
                        macc,
                        order,
                        txn_detail,
                        txn_card_info,
                        m_card_info,
                        m_internal_meta,
                        order_meta_data,
                        default_gateway_priority_logic_output()
                            .with_priority_logic_tag(priority_logic_tag)
                            .with_primary_logic(Some(priority_logic_data)),
                        priority_logic_data.failure_reason,
                    )
                    .await
                }
            }
        }
    } else {
        match macc.gateway_priority {
            None => Ok(default_gateway_priority_logic_output()),
            Some(t) => {
                if t.is_empty() {
                    L::log_debug_v(
                        "gatewayPriority",
                        format!(
                            "gatewayPriority for merchant: {} is empty.",
                            macc.merchant_id
                        ),
                    )
                    .await;
                    Ok(default_gateway_priority_logic_output())
                } else {
                    let list_of_gateway_in_text = t
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    let (list_of_gateway, errs) = convert_text_to_gateway(list_of_gateway_in_text);
                    L::log_debug_t(
                        "gatewayPriority",
                        format!(
                            "gatewayPriority for merchant: listOfGateway {:?}",
                            list_of_gateway
                        ),
                    )
                    .await;
                    match list_of_gateway.as_slice() {
                        [] => {
                            L::log_error_v(
                                "gatewayPriority emptyList",
                                format!(
                                    "Can't get gatewayPriority for merchant: {}. Input: {}",
                                    macc.merchant_id, t
                                ),
                            )
                            .await;
                            Ok(default_gateway_priority_logic_output())
                        }
                        res => {
                            L::log_debug_t(
                                "gatewayPriority decoding",
                                format!(
                                    "Decoded successfully. Input: {} output: {:?}",
                                    t, res
                                ),
                            )
                            .await;
                            Ok(default_gateway_priority_logic_output().with_gateways(res.to_vec()))
                        }
                    }
                }
            }
        }
    }
}

async fn get_script(
    macc: ETM::MerchantAccount,
    maybe_script: Option<String>,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    match maybe_script {
        Some(script) => Ok((script, Some("TEST_PL".to_string()))),
        None => get_priority_logic_script(macc).await,
    }
}

fn default_gateway_priority_logic_output() -> DeciderTypes::GatewayPriorityLogicOutput {
    DeciderTypes::GatewayPriorityLogicOutput {
        is_enforcement: false,
        gws: vec![],
        priority_logic_tag: None,
        gateway_reference_ids: vec![],
        primary_logic: None,
        fallback_logic: None,
    }
}

async fn get_priority_logic_script(
    macc: ETM::MerchantAccount,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    match Utils::get_true_string(macc.priority_logic_config) {
        Some(priority_logic_config) => match Utils::either_decode_t::<PriorityLogicConfig>(&priority_logic_config) {
            Ok(pl_config) => {
                let mpl_id = match pl_config.stagger {
                    Some(StaggerBtwnTwo { staggered_logic, rollout }) => {
                        if Feature::roller("GatewayDecider::getPriorityLogicScript", rollout).await {
                            staggered_logic
                        } else {
                            pl_config.active_logic
                        }
                    }
                    None => pl_config.active_logic,
                };
                match MPL::find_priority_logic_by_id(mpl_id).await {
                    Some(mpl) => Ok((mpl.priority_logic, mpl.name)),
                    None => {
                        L::log_error_t(
                            "getPriorityLogicScript",
                            format!("No merchant_priority_logic found for id {}", mpl_id),
                        )
                        .await;
                        let pl_tag = get_active_priority_logic_name(macc).await?;
                        Ok((macc.gateway_priority_logic, pl_tag))
                    }
                }
            }
            Err(err) => {
                L::log_error_t(
                    "getPriorityLogicScript",
                    format!(
                        "Error while decoding PriorityLogicConfig for {} {}",
                        ETM::to_text(macc.merchant_id),
                        err
                    ),
                )
                .await;
                let pl_tag = get_active_priority_logic_name(macc).await?;
                Ok((macc.gateway_priority_logic, pl_tag))
            }
        },
        None => {
            if macc.gateway_priority_logic.trim().is_empty() {
                get_priority_logic_script_from_tenant_config(macc).await
            } else {
                let pl_tag = get_active_priority_logic_name(macc).await?;
                Ok((macc.gateway_priority_logic, pl_tag))
            }
        }
    }
}

async fn get_active_priority_logic_name(
    macc: ETM::MerchantAccount,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match macc.internal_metadata {
        Some(metadata) => match Utils::get_value("active_priority_logic_name", &metadata) {
            Some(name) => Ok(Some(name)),
            None => get_active_priority_logic_name_from_db(macc).await,
        },
        None => get_active_priority_logic_name_from_db(macc).await,
    }
}

async fn get_active_priority_logic_name_from_db(
    macc: ETM::MerchantAccount,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let all_mpls = MPL::find_all_priority_logic_by_merchant_p_id(macc.id).await?;
    match all_mpls.iter().find(|mpl| mpl.is_active_logic) {
        Some(mpl) => Ok(Some(mpl.name.clone())),
        None => Ok(None),
    }
}

async fn get_priority_logic_script_from_tenant_config(
    macc: ETM::MerchantAccount,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    match macc.tenant_account_id {
        Some(tenant_account_id) => {
            match TenantConfig::get_tenant_config_by_tenant_id_and_module_name_and_module_key_and_type(
                tenant_account_id,
                TenantConfig::PRIORITY_LOGIC,
                "priority_logic",
                TenantConfig::FALLBACK,
            )
            .await
            {
                Some(tenant_config) => match (tenant_config.filter_dimension, tenant_config.filter_group_id) {
                    (Some(filter_dimension), Some(filter_group_id)) => {
                        L::log_info_t(
                            "getPriorityLogicScriptFromTenantConfig",
                            format!(
                                "Filter dimension found: {}",
                                TenantConfig::filter_dimension_to_text(filter_dimension)
                            ),
                        )
                        .await;
                        get_pl_by_filter_dimension(macc, filter_dimension, filter_group_id, tenant_config.config_value).await
                    }
                    _ => {
                        L::log_info_t(
                            "getPriorityLogicScriptFromTenantConfig",
                            "Filter dimension and filter groupId are not present. Proceeding with default tenant config value.",
                        )
                        .await;
                        decode_tenant_pl_config(tenant_config.config_value).await
                    }
                },
                None => {
                    let tenant_account_id = macc.tenant_account_id.unwrap_or_default();
                    L::log_debug_t(
                        "getPriorityLogicScriptFromTenantConfig",
                        format!(
                            "Unable to find tenant config of tenantAccountId {} for module {} and key priority_logic. Proceeding with gateway priority logic.",
                            tenant_account_id,
                            TenantConfig::module_name_to_text(TenantConfig::PRIORITY_LOGIC),
                        ),
                    )
                    .await;
                    Ok((String::new(), None))
                }
            }
        }
        None => Ok((String::new(), None)),
    }
}

async fn get_pl_by_filter_dimension(
    macc: ETM::MerchantAccount,
    filter_dimension: TenantConfig::FilterDimension,
    filter_group_id: String,
    config_value: String,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    match filter_dimension {
        TenantConfig::MCC => get_pl_by_merchant_category_code(macc, filter_group_id, config_value).await,
        _ => {
            L::log_info_t(
                "getPLByFilterDimension",
                "Filter dimension is not supported. Proceeding with default tenant config value.",
            )
            .await;
            decode_tenant_pl_config(config_value).await
        }
    }
}

async fn get_pl_by_merchant_category_code(
    macc: ETM::MerchantAccount,
    filter_group_id: String,
    config_value: String,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    match macc.merchant_category_code {
        Some(mcc) => {
            match TenantConfigFilter::get_tenant_config_filter_by_group_id_and_dimension_value(filter_group_id, mcc).await {
                Some(tenant_config_filter) => {
                    L::log_info_t(
                        "getPLByMerchantCategoryCode",
                        "Proceeding with tenant config filter priority logic.",
                    )
                    .await;
                    decode_tenant_pl_config(tenant_config_filter.config_value).await
                }
                None => {
                    L::log_info_t(
                        "getPLByMerchantCategoryCode",
                        format!(
                            "Unable to find tenant config filter for groupId {} and dimension value {}",
                            filter_group_id, mcc
                        ),
                    )
                    .await;
                    decode_tenant_pl_config(config_value).await
                }
            }
        }
        None => {
            L::log_error_t(
                "getPLByMerchantCategoryCode",
                format!(
                    "Merchant category code is not present for merchantId {}",
                    ETM::to_text(macc.merchant_id),
                ),
            )
            .await;
            decode_tenant_pl_config(config_value).await
        }
    }
}

async fn decode_tenant_pl_config(
    config_value: String,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    match Utils::either_decode_t::<TenantPLConfig>(&config_value) {
        Ok(tenant_pl_config) => {
            L::log_debug_t(
                "decodeTenantPLConfig",
                format!(
                    "tenantPLConfig decoded successfully with name: {}",
                    tenant_pl_config.name,
                ),
            )
            .await;
            Ok((tenant_pl_config.priority_logic, Some(tenant_pl_config.name)))
        }
        Err(err) => {
            L::log_error_t(
                "decodeTenantPLConfig",
                format!(
                    "Error while decoding TenantPLConfig for {} {}",
                    ETM::to_text(macc.merchant_id),
                    err,
                ),
            )
            .await;
            Ok((String::new(), None))
        }
    }
}

pub async fn get_fallback_priority_logic_script(
    macc: ETM::MerchantAccount,
) -> Result<(Option<String>, Option<String>), Box<dyn std::error::Error>> {
    match macc.priority_logic_config {
        Some(priority_logic_config) => match Utils::either_decode_t::<PriorityLogicConfig>(&priority_logic_config) {
            Ok(pl_config) => {
                let mpl_m = match pl_config.fallback_logic {
                    Some(mpl_id) => MPL::find_priority_logic_by_id(mpl_id).await,
                    None => None,
                };
                match mpl_m {
                    Some(mpl) => Ok((Some(mpl.priority_logic), Some(mpl.name))),
                    None => Ok((None, None)),
                }
            }
            Err(err) => {
                L::log_error_t(
                    "getFallbackPriorityLogicScript",
                    format!(
                        "Error while decoding PriorityLogicConfig for {} {}",
                        ETM::to_text(macc.merchant_id),
                        err,
                    ),
                )
                .await;
                Ok((None, None))
            }
        },
        None => Ok((None, None)),
    }
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

pub async fn handle_fallback_logic(
    macc: ETM::MerchantAccount,
    order: ETO::Order,
    txn_detail: ETTD::TxnDetail,
    txn_card_info: TxnCardInfo,
    m_card_info: Option<CardInfo>,
    m_internal_meta: Option<DeciderTypes::InternalMetadata>,
    order_meta_data: Option<String>,
    primary_logic_output: DeciderTypes::GatewayPriorityLogicOutput,
    pl_failure_reason: DeciderTypes::PriorityLogicFailure,
) -> DeciderTypes::GatewayPriorityLogicOutput {
    if primary_logic_output.fallback_logic.is_none() && primary_logic_output.primary_logic.is_some() {
        let (fallback_logic, fallback_pl_tag) = get_fallback_priority_logic_script(macc).await;
        match fallback_logic {
            Some(fallback_script) => {
                let fallback_result = eval_script(
                    order,
                    txn_detail,
                    txn_card_info,
                    m_card_info,
                    macc.merchant_id,
                    fallback_script,
                    m_internal_meta,
                    order_meta_data,
                    fallback_pl_tag,
                )
                .await;
                match fallback_result {
                    EvaluationResult::PLResponse(gws, pl_data, logs, status) => {
                        L::log_debug_v(
                            format!("FALLBACK_PRIORITY_LOGIC_EXECUTION_{}", status),
                            format!(
                                "MerchantId: {}, Gateways: {}, Logs: {}",
                                macc.merchant_id, gws, logs
                            ),
                        )
                        .await;
                        return DeciderTypes::GatewayPriorityLogicOutput {
                            fallback_logic: Some(pl_data),
                            priority_logic_tag: fallback_pl_tag,
                            primary_logic: check_and_update_pl_failure_reason(
                                primary_logic_output.primary_logic,
                            ),
                            ..primary_logic_output
                        };
                    }
                    EvaluationResult::EvaluationError(priority_logic_data, err) => {
                        L::log_error_v(
                            "FALLBACK_PRIORITY_LOGIC_EXECUTION_FAILURE",
                            format!(
                                "MerchantId: {}, Error: {}",
                                macc.merchant_id, err
                            ),
                        )
                        .await;
                        return DeciderTypes::GatewayPriorityLogicOutput {
                            primary_logic: check_and_update_pl_failure_reason(
                                primary_logic_output.primary_logic,
                            ),
                            fallback_logic: Some(priority_logic_data),
                            priority_logic_tag: fallback_pl_tag,
                            ..primary_logic_output
                        };
                    }
                }
            }
            None => {
                return DeciderTypes::GatewayPriorityLogicOutput {
                    primary_logic: check_and_update_pl_failure_reason(
                        primary_logic_output.primary_logic,
                    ),
                    ..primary_logic_output
                };
            }
        }
    } else {
        return DeciderTypes::GatewayPriorityLogicOutput {
            fallback_logic: check_and_update_pl_failure_reason(
                primary_logic_output.fallback_logic,
            ),
            ..primary_logic_output
        };
    }
}

fn check_and_update_pl_failure_reason(
    primary_pl_data: Option<DeciderTypes::PriorityLogicData>,
) -> Option<DeciderTypes::PriorityLogicData> {
    match primary_pl_data {
        None => None,
        Some(mut data) => {
            if data.failure_reason != pl_failure_reason {
                data.status = DeciderTypes::FAILURE;
                data.failure_reason = pl_failure_reason;
            }
            Some(data)
        }
    }
}

pub async fn eval_script(
    order: ETO::Order,
    txn_detail: ETTD::TxnDetail,
    txn_card_info: TxnCardInfo,
    m_card_info: Option<CardInfo>,
    merch_id: MerchantId,
    script: Script,
    m_internal_meta: Option<DeciderTypes::InternalMetadata>,
    meta_data: Option<String>,
    priority_logic_tag: Option<String>,
) -> EvaluationResult {
    let order_meta_data = meta_data.and_then(|md| Utils::parse_json_from_string(&md));
    let juspay_bank_code = Utils::get_juspay_bank_code_from_internal_metadata(&txn_detail);
    let response = L::call_api(
        Some(T::ManagerSelector::TlsManager),
        groovy_executor_url,
        EC_PL_EVALUATION,
        || None,
        make_request(order_meta_data, juspay_bank_code),
    )
    .await;
    handle_response(response, priority_logic_tag).await
}

fn make_request(
    meta_data: Option<Object>,
    juspay_bank_code: Option<String>,
) -> SD {
    SD {
        order_info: filter_order(order, meta_data),
        txn_info: filter_txn(txn_detail),
        payment_info: make_payment_info(txn_card_info, m_card_info, m_internal_meta, juspay_bank_code),
        merchant_id: merch_id,
        script,
    }
}

async fn handle_response(
    response: Result<ResponseBody, ClientError>,
    priority_logic_tag: Option<String>,
) -> EvaluationResult {
    match response {
        Err(client_error) => {
            let pl_resp = handle_client_error(client_error);
            let log_entries = pl_resp.log.unwrap_or_default().into_iter().map(parse_log_entry).collect();
            let pl_data = DeciderTypes::PriorityLogicData {
                name: priority_logic_tag,
                status: DeciderTypes::FAILURE,
                failure_reason: pl_resp.error_message,
            };
            EvaluationResult::EvaluationError(pl_data, log_entries)
        }
        Ok(response_body) => {
            let log_entries = response_body.log.unwrap_or_default().into_iter().map(parse_log_entry).collect();
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
        status: DeciderTypes::FAILURE,
        failure_reason: DeciderTypes::PL_EVALUATION_FAILED,
    };
    EvaluationResult::EvaluationError(pl_data, log_entries)
}

async fn handle_success_response(
    response_body: ResponseBody,
    priority_logic_tag: Option<String>,
    log_entries: Vec<LogEntry>,
) -> EvaluationResult {
    let (gws, errs) = convert_text_to_gateway(response_body.result.gateway_priority.unwrap_or_default());
    let is_gateway_parse_failure = !errs.is_empty();
    let status = if is_gateway_parse_failure {
        DeciderTypes::FAILURE
    } else {
        DeciderTypes::SUCCESS
    };
    let pl_data = DeciderTypes::PriorityLogicData {
        name: priority_logic_tag,
        status,
        failure_reason: if is_gateway_parse_failure {
            DeciderTypes::GATEWAY_NAME_PARSE_FAILURE
        } else {
            DeciderTypes::NO_ERROR
        },
    };
    let pl_output = DeciderTypes::GatewayPriorityLogicOutput {
        is_enforcement: response_body.result.is_enforcement.unwrap_or(false),
        gws,
        priority_logic_tag,
        gateway_reference_ids: response_body.result.gateway_reference_ids.unwrap_or_default(),
        primary_logic: None,
        fallback_logic: None,
    };
    EvaluationResult::PLResponse(pl_output, pl_data, log_entries.into_iter().chain(errs).collect(), status)
}

fn convert_text_to_gateway(arr: Vec<String>) -> (Vec<Gateway>, Vec<LogEntry>) {
    arr.into_iter().fold((vec![], vec![]), |(mut gateways, mut errors), gw| {
        match gw.parse::<Gateway>() {
            Ok(res) => {
                gateways.push(res);
            }
            Err(err) => {
                errors.push(LogEntry::Error(format!("Gateway name parse failed: {}", err)));
            }
        }
        (gateways, errors)
    })
}

fn handle_client_error(client_error: ClientError) -> PLExecutorError {
    match client_error {
        ClientError::FailureResponse(_, resp) => match A::either_decode(&resp.body) {
            Ok(api_error) => api_error,
            Err(parse_error) => PLExecutorError {
                error: true,
                error_message: DeciderTypes::RESPONSE_PARSE_ERROR,
                user_message: "Failed to parse response".to_string(),
                log: Some(vec![vec![
                    "Error".to_string(),
                    format!("Failed to parse JSON: {}", parse_error),
                ]]),
            },
        },
        ClientError::DecodeFailure(err, resp) => PLExecutorError {
            error: true,
            error_message: DeciderTypes::RESPONSE_DECODE_FAILURE,
            user_message: "Response decoding failed.".to_string(),
            log: Some(vec![
                vec![
                    "Error".to_string(),
                    "Response decoding failed".to_string(),
                    err.to_string(),
                ],
                vec![
                    "Info".to_string(),
                    format!("Response: {:?}", resp),
                ],
            ]),
        },
        ClientError::UnsupportedContentType(err, resp) => PLExecutorError {
            error: true,
            error_message: DeciderTypes::RESPONSE_CONTENT_TYPE_NOT_SUPPORTED,
            user_message: "The response had an unsupported content type.".to_string(),
            log: Some(vec![
                vec![
                    "Error".to_string(),
                    format!("Unsupported content type: {:?}", err),
                ],
                vec![
                    "Info".to_string(),
                    format!("Response: {:?}", resp),
                ],
            ]),
        },
        ClientError::InvalidContentTypeHeader(resp) => PLExecutorError {
            error: true,
            error_message: DeciderTypes::RESPONSE_CONTENT_TYPE_NOT_SUPPORTED,
            user_message: "The response had an invalid content type header.".to_string(),
            log: Some(vec![
                vec![
                    "Error".to_string(),
                    "Invalid content type header.".to_string(),
                ],
                vec![
                    "Info".to_string(),
                    format!("Response: {:?}", resp),
                ],
            ]),
        },
        ClientError::ConnectionError(err) => PLExecutorError {
            error: true,
            error_message: DeciderTypes::CONNECTION_FAILED,
            user_message: "A connection error occurred.".to_string(),
            log: Some(vec![vec![
                "Error".to_string(),
                format!("A connection error occurred: {:?}", err),
            ]]),
        },
    }
}

