use masking::Secret;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, from_str, to_string, Value};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::iter::FromIterator;
use std::option::Option;
use std::result::Result;
use std::str::FromStr;
use std::string::String;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::vec::Vec;
use time::format_description::well_known::Rfc3339;
use time::format_description::parse;
use time::parsing::Parsed;
use time::{Date, UtcDateTime};
use crate::decider::gatewaydecider::types::{self, DeciderFlow};
use crate::redis::types::ServiceConfigKey;
use crate::types::merchant::id::{merchant_id_to_text, MerchantId};
use crate::types::merchant::merchant_account::MerchantAccount;
use crate::types::money::internal::Money;
use crate::types::payment_flow::PaymentFlow;
use crate::types::token_bin_info::{getAllTokenBinInfoByTokenBins, TokenBinInfo};
use crate::utils::{generate_random_number, get_current_date_in_millis};
use masking::PeekInterface;

use serde_json::from_value;

// // use eulerhs::prelude::*;
// // use eulerhs::language::*;
// // use optics::prelude::*;
// // use regex::Regex;
// // use juspay::extra::secret::{Secret, unsafe_extract_secret, make_secret};
// // use juspay::extra::json::encode_json;
// // use gateway_decider::types::*;
// // use types::gateway_payment_method::GpmPId;
// // use types::card::card_info::{CardInfo, get_all_card_info_by_isins};
// // use types::card::Isin;
// // use types::merchant_config::types::{ConfigCategory, ConfigName};
// // use eulerhs::language as L;
// // use serde_json::json;
// // use eulerhs::types::TxResult;
// // use data_encoding::BASE64;
use crate::decider::gatewaydecider::constants as C;
use crate::types::{card as ETCa, merchant, payment};
// // use types::currency as Curr;
use crate::types::feature as ETF;
// use crate::types::gateway as Gateway;
// // use types::gateway_payment_method as ETGPM;
use crate::types::merchant as ETM;

use super::types::{
    ConfigurableBlock, GatewayList, GatewayRedisKeyMap, GatewayScoreMap, GatewayScoringData, GatewayWiseExtraScore, InternalMetadata, MessageFormat, OptimizationRedisBlockData, ScoreKeyType, SplitSettlementDetails, SrV3InputConfig, SrV3SubLevelInputConfig, ValidationType
};
use crate::types::merchant_gateway_card_info as ETMGCI;
// // use types::merchant_gateway_card_info as ETMGCI;
// // use types::merchant_gateway_payment_method as ETMGPM;
// // use types::money as Money;
use crate::types::card::txn_card_info as ETTCa;
use crate::types::order as ETO;
use crate::types::payment::{self as ETP, payment_method::PaymentMethodType};
use crate::types::txn_details::types as ETTD;
use crate::types::txn_offer as ETTO;
// use juspay::extra::parsing as P;
use crate::types::gateway::{self as ETG, gateway_to_text, Gateway};
use crate::types::token_bin_info as ETTB;
// // use utils::config::constants as Config;
// // use utils::logging as EWL;
// // use safe::Safe;
// // use control::category::Category;
// // use juspay::extra::non_empty_text as NET;
use crate::types::isin_routes as ETIsinR;
use crate::redis::cache as RService;
use crate::redis::commands as RC;
// // use utils::redis as EWRedis;
// // use db::common::types::payment_flows as PF;
// // use utils::redis as Redis;
// // use eulerhs::tenant_redis_layer as RC;
// // use eulerhs::types as EHT;
// // use configs::env_vars as ENV;

pub fn either_decode_t<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, String> {
    from_slice(text.as_bytes()).map_err(|e| e.to_string())
}

pub fn get_vault_provider(t: Option<&str>) -> Option<ETCa::vault_provider::VaultProvider> {
    match t {
        Some(t) if t.starts_with("sodexo") => Some(ETCa::vault_provider::VaultProvider::Sodexo),
        Some(t) if t.starts_with("payu") => Some(ETCa::vault_provider::VaultProvider::PayU),
        Some(_) => Some(ETCa::vault_provider::VaultProvider::Juspay),
        None => None,
    }
}

pub fn is_card_transaction(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> bool {
    match txn_card_info.card_isin.as_deref() {
        Some("") | None => false,
        _ => true,
    }
}

pub fn is_nb_transaction(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> bool {
    txn_card_info.card_type == Some(ETCa::card_type::CardType::NB)
}

pub fn is_subscription(mga: &ETM::merchant_gateway_account::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "MANDATE", "subscription")
}

pub fn is_emandate_enabled(mga: &ETM::merchant_gateway_account::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "EMANDATE", "enableEmandate")
}

pub fn is_only_subscription(mga: &ETM::merchant_gateway_account::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "SUBSCRIPTION_ONLY", "onlySubscription")
}

pub fn is_otm_enabled(mga: &ETM::merchant_gateway_account::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "ONE_TIME_MANDATE", "OTM_ENABLED")
}

pub fn is_seamless(mga: &ETM::merchant_gateway_account::MerchantGatewayAccount) -> bool {
    false
}

pub fn check_no_or_low_cost_emi(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> bool {
    fetch_emi_type(txn_card_info)
        .map(|emi_type| ["NO_COST_EMI", "LOW_COST_EMI"].contains(&emi_type.as_str()))
        .unwrap_or(false)
}

pub fn fetch_emi_type(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> Option<String> {
    txn_card_info
        .paymentSource
        .as_ref()
        .and_then(|source| get_value("emi_type", source))
}

pub fn fetch_extended_card_bin(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> Option<String> {
    txn_card_info
        .paymentSource
        .as_ref()
        .and_then(|source| get_value("extended_card_bin", source))
}

fn fetch_juspay_bank_code(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> Option<String> {
    txn_card_info
        .paymentSource
        .as_ref()
        .and_then(|source| get_value("juspay_bank_code", source))
}

fn get_pl_gw_ref_id_map(decider_flow: &DeciderFlow<'_>) -> HashMap<String, String> {
    decider_flow
        .get()
        .dpPriorityLogicOutput
        .as_ref()
        .map_or_else(HashMap::new, |output| output.gatewayReferenceIds.clone())
}

pub fn get_order_metadata_and_pl_ref_id_map(
    decider_flow: &mut DeciderFlow<'_>,
    enable_gateway_reference_id_based_routing: Option<bool>,
    order: &ETO::Order,
) -> (HashMap<String, String>, HashMap<String, String>) {
    if enable_gateway_reference_id_based_routing.unwrap_or(false) {
        let order_metadata = get_metadata(decider_flow);
        let pl_gw_ref_id_map = get_pl_gw_ref_id_map(decider_flow);
        (order_metadata, pl_gw_ref_id_map)
    } else {
        (HashMap::new(), HashMap::new())
    }
}

fn is_emandate_supported_payment_method(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> bool {
    matches!(
        txn_card_info.paymentMethodType,
        PaymentMethodType::Card
            | PaymentMethodType::NB
            | PaymentMethodType::Wallet
            | PaymentMethodType::UPI
            | PaymentMethodType::Aadhaar
            | PaymentMethodType::Papernach
            | PaymentMethodType::PAN
    )
}

fn is_emandate_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    matches!(
        txn_detail.txnObjectType,
        ETTD::TxnObjectType::EmandateRegister
            | ETTD::TxnObjectType::EmandatePayment
            | ETTD::TxnObjectType::TpvEmandateRegister
    )
}

fn is_tpv_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    txn_detail.txnObjectType == ETTD::TxnObjectType::TpvPayment
}

fn is_tpv_mandate_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    txn_detail.txnObjectType == ETTD::TxnObjectType::TpvEmandateRegister
}

fn get_merchant_wise_si_bin_key(gw: &Gateway) -> String {
    format!("MERCHANT_WISE_SI_BINS_{:?}", gw)
}

fn get_merchant_gateway_card_info_feature_name(
    auth_type: Option<&ETCa::txn_card_info::AuthType>,
    validation_type: Option<&ValidationType>,
    gateway: &Gateway,
) -> Option<String> {
    let flow = validation_type
        .map(|v| format!("{:?}", v))
        .or_else(|| auth_type.map(|a| format!("{:?}", a)));
    Some(format!(
        "MERCHANT_GATEWAY_CARD_INFO_{:?}_{:?}",
        flow, gateway
    ))
}

fn is_mandate_transaction(txn: &ETTD::TxnDetail) -> bool {
    matches!(
        txn.txnObjectType,
        ETTD::TxnObjectType::MandateRegister | ETTD::TxnObjectType::MandatePayment
    )
}

pub async fn get_merchant_wise_mandate_bin_eligible_gateways(
    merchant_account: &ETM::merchant_account::MerchantAccount,
    mandate_enabled_gateways: &[Gateway],
) -> Vec<Gateway> {
    let merchant_wise_mandate_bin_enforced_gateways: Vec<Gateway> = todo!();
    // RService::findByNameFromRedis(C::MERCHANT_WISE_MANDATE_BIN_ENFORCED_GATEWAYS)
    //     .await
    //     .unwrap_or_default();
    let merchant_wise_mandate_supported_gateway: Vec<Gateway> =
        merchant_wise_mandate_bin_enforced_gateways
            .into_iter()
            .filter(|gateway| mandate_enabled_gateways.contains(gateway))
            .collect();
    let mut gws = Vec::new();
    for gateway in merchant_wise_mandate_supported_gateway {
        if ETF::get_feature_enabled(
            &get_merchant_wise_si_bin_key(&gateway),
            &merchant_account.merchantId,
            true,
        )
        .await
        .is_some()
        {
            gws.push(gateway);
        }
    }
    gws
}

pub async fn is_merchant_wise_auth_type_check_needed(
    merchant_account: &ETM::merchant_account::MerchantAccount,
    auth_type: Option<&ETCa::txn_card_info::AuthType>,
    validation_type: Option<&ValidationType>,
    gateway: &Gateway,
) -> bool {
    let merchant_wise_auth_type_bin_enforced_gateways: Vec<Gateway> = todo!();
    // RService::findByNameFromRedis(C::MERCHANT_WISE_AUTH_TYPE_BIN_ENFORCED_GATEWAYS)
    //     .await
    //     .unwrap_or_default();
    if merchant_wise_auth_type_bin_enforced_gateways.contains(gateway) {
        if let Some(feature_key) =
            get_merchant_gateway_card_info_feature_name(auth_type, validation_type, gateway)
        {
            return ETF::get_feature_enabled(&feature_key, &merchant_account.merchantId, true)
                .await
                .is_some();
        }
    }
    false
}

async fn get_internal_meta_data(decider_flow: &DeciderFlow<'_>) -> Option<types::InternalMetadata> {
    decider_flow.writer.internalMetaData.clone()
}

pub fn set_internal_meta_data(
    decider_flow: &mut DeciderFlow<'_>,
    internal_metadata: Option<types::InternalMetadata>,
) {
    decider_flow.writer.internalMetaData = internal_metadata;
}

pub fn set_top_gateway_before_sr_downtime_evaluation(
    decider_flow: &mut DeciderFlow<'_>,
    gw: Option<ETG::Gateway>,
) {
    decider_flow.writer.topGatewayBeforeSRDowntimeEvaluation = gw;
}

pub fn set_is_optimized_based_on_sr_metric_enabled(
    decider_flow: &mut DeciderFlow<'_>,
    is_enabled: bool,
) {
    decider_flow.writer.isOptimizedBasedOnSRMetricEnabled = is_enabled;
}

pub fn set_is_sr_v3_metric_enabled(decider_flow: &mut DeciderFlow<'_>, is_enabled: bool) {
    decider_flow.writer.isSrV3MetricEnabled = is_enabled;
}

pub fn set_is_primary_gateway(decider_flow: &mut DeciderFlow<'_>, is_enabled: bool) {
    decider_flow.writer.isPrimaryGateway = Some(is_enabled);
}

pub fn set_is_experiment_tag(decider_flow: &mut DeciderFlow<'_>, exp_tag: Option<String>) {
    decider_flow.writer.experiment_tag = exp_tag;
}

pub fn set_gw_ref_id(decider_flow: &mut DeciderFlow<'_>, gw_ref_id: Option<String>) {
    decider_flow.writer.gateway_reference_id = gw_ref_id;
}

pub fn get_mgas(
    decider_flow: &DeciderFlow<'_>,
) -> Option<Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>> {
    decider_flow.writer.mgas.clone()
}

pub fn set_mgas(
    decider_flow: &mut DeciderFlow<'_>,
    mgas: Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,
) {
    decider_flow.writer.mgas = Some(mgas);
}

pub fn get_routing_dimension(decider_flow: &DeciderFlow<'_>) -> Option<String> {
    decider_flow.writer.routing_dimension.clone()
}

pub fn set_routing_dimension(decider_flow: &mut DeciderFlow<'_>, dim: String) {
    decider_flow.writer.routing_dimension = Some(dim);
}

pub fn get_routing_dimension_level(decider_flow: &DeciderFlow<'_>) -> Option<String> {
    decider_flow.writer.routing_dimension_level.clone()
}

pub fn set_routing_dimension_level(decider_flow: &mut DeciderFlow<'_>, val: String) {
    decider_flow.writer.routing_dimension_level = Some(val);
}

pub fn set_outage_dimension_level(decider_flow: &mut DeciderFlow<'_>, dim: String) {
    decider_flow.writer.outage_dimension = Some(dim);
}

pub fn set_elimination_dimension_level(decider_flow: &mut DeciderFlow<'_>, dim: String) {
    decider_flow.writer.elimination_dimension = Some(dim);
}

pub async fn set_sr_gateway_scores(
    decider_flow: &mut DeciderFlow<'_>,
    gw_scores: Vec<types::GatewayScore>,
) {
    decider_flow.writer.sr_gateway_scores = Some(gw_scores);
}

async fn set_elimination_scores(
    decider_flow: &mut DeciderFlow<'_>,
    gw_scores: Vec<types::GatewayScore>,
) {
    decider_flow.writer.elimination_scores = Some(gw_scores);
}

async fn set_srv3_bucket_size(decider_flow: &mut DeciderFlow<'_>, srv3_bucket_size: i32) {
    decider_flow.writer.srv3_bucket_size = Some(srv3_bucket_size);
}

async fn set_sr_v3_hedging_percent(decider_flow: &mut DeciderFlow<'_>, sr_v3_hedging_percent: f64) {
    decider_flow.writer.sr_v3_hedging_percent = Some(sr_v3_hedging_percent);
}

async fn get_reset_approach(decider_flow: &DeciderFlow<'_>) -> types::ResetApproach {
    decider_flow.writer.reset_approach.clone()
}

async fn set_reset_approach(decider_flow: &mut DeciderFlow<'_>, res_app: types::ResetApproach) {
    decider_flow.writer.reset_approach = res_app;
}

async fn set_is_merchant_enabled_for_dynamic_mga_selection(
    decider_flow: &mut DeciderFlow<'_>,
    is_dynamic_mga_enabled: bool,
) {
    decider_flow.writer.is_dynamic_mga_enabled = is_dynamic_mga_enabled;
}

pub async fn get_is_merchant_enabled_for_dynamic_mga_selection(
    decider_flow: &DeciderFlow<'_>,
) -> bool {
    decider_flow.writer.is_dynamic_mga_enabled
}

pub fn parse_json_from_string(text_data: &str) -> Option<Value> {
    from_str(text_data).ok()
}

pub fn get_value<T: for<'de> Deserialize<'de>>(key: &str, t: &str) -> Option<T> {
    from_str::<Value>(t).ok().and_then(|v| match v {
        Value::Object(map) => map.get(key).and_then(|v| from_value(v.clone()).ok()),
        _ => None,
    })
}

fn is_txn_type_enabled(
    supported_txn_type: Option<&str>,
    payment_method_type: &str,
    txn_type: &str,
) -> bool {
    supported_txn_type
        .and_then(|s| get_value::<Vec<String>>(payment_method_type, s))
        .map_or(true, |types| types.contains(&txn_type.to_string()))
}
fn get_value_from_text(key: &str, t: &Value) -> Option<Value> {
    match t {
        Value::Object(map) => map.get(key).cloned(),
        _ => None,
    }
}

fn get_enabled_gateway_for_brand(brand: &str, enabled_gateways: Option<&Value>) -> Option<Value> {
    enabled_gateways.and_then(|gateways| match gateways {
        Value::Object(map) => map.get(brand).cloned(),
        _ => None,
    })
}

fn text_to_gateway_t(t: &[String]) -> Option<Vec<ETG::Gateway>> {
    t.iter()
        .map(|s| ETG::text_to_gateway(s))
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn parse_aeson_string<T: for<'de> Deserialize<'de>>(value: &Value) -> Option<T> {
    match value {
        Value::String(s) => from_str(s).ok(),
        _ => None,
    }
}

fn result_to_maybe<T>(result: Result<T, serde_json::Error>) -> Option<T> {
    result.ok()
}

fn decode_metadata(text: &str) -> HashMap<String, String> {
    from_str::<HashMap<String, Value>>(text)
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect()
}

pub async fn get_all_possible_ref_ids(
    metadata: HashMap<String, String>,
    oref: ETO::Order,
    pl_ref_id_map: HashMap<String, String>,
) -> Vec<ETM::merchant_gateway_account::MgaReferenceId> {
    let gateway_ref_ids = is_suffix_of_gateway_ref_id(metadata.iter().collect());
    let gateway_ref_ids_from_pl = is_suffix_of_gateway_ref_id(pl_ref_id_map.iter().collect());
    gateway_ref_ids
        .into_iter()
        .chain(gateway_ref_ids_from_pl.into_iter())
        .collect()
}

fn is_suffix_of_gateway_ref_id(
    list_of_key_value: Vec<(&String, &String)>,
) -> Vec<ETM::merchant_gateway_account::MgaReferenceId> {
    list_of_key_value
        .into_iter()
        .filter(|(key, _)| key.ends_with("gateway_reference_id"))
        .map(|(_, val)| ETM::merchant_gateway_account::to_mga_reference_id(val.clone()))
        .collect()
}

pub async fn get_all_ref_ids(
    metadata: HashMap<String, String>,
    pl_ref_id_map: HashMap<String, String>,
) -> HashMap<String, String> {
    let gw_ref_ids_from_pl: Vec<(String, String)> = pl_ref_id_map
        .iter()
        .filter(|(k, _)| k.ends_with(":gateway_reference_id"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let gw_ref_ids_from_order: Vec<(String, String)> = metadata
        .iter()
        .filter(|(k, _)| k.ends_with(":gateway_reference_id"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    gw_ref_ids_from_pl
        .into_iter()
        .chain(gw_ref_ids_from_order.into_iter())
        .collect()
}

pub fn get_gateway_reference_id(
    metadata: HashMap<String, String>,
    gw: &ETG::Gateway,
    oref: ETO::Order,
    pl_ref_id_map: HashMap<String, String>,
) -> Option<ETM::merchant_gateway_account::MgaReferenceId> {
    let meta_res = pl_ref_id_map
        .get(&format!("{:?}:gateway_reference_id", gw))
        .or_else(|| pl_ref_id_map.get("JUSPAY:gateway_reference_id"))
        .or_else(|| metadata.get(&format!("{:?}:gateway_reference_id", gw)))
        .or_else(|| metadata.get("JUSPAY:gateway_reference_id"));

    match meta_res {
        Some(val) if !val.is_empty() => Some(ETM::merchant_gateway_account::to_mga_reference_id(
            val.clone(),
        )),
        _ => None,
    }
}

pub async fn effective_amount_with_txn_amount(txn_detail: ETTD::TxnDetail) -> Money {
    let def_amount = Money::from_double(0.0);
    let amount_txn = &txn_detail.txnAmount;
    let offers = ETTO::getOffers(&txn_detail.id).await;
    let discount_sum: Money = Money::from_double(
        offers
            .iter()
            .map(|offer| offer.discountAmount.clone())
            .map(|m| m.to_double())
            .sum(),
    );
    let final_amount = amount_txn
        .m_sub(&discount_sum)
        .m_add(&txn_detail.surchargeAmount.unwrap_or(def_amount.clone()))
        .m_add(&txn_detail.taxAmount.unwrap_or(def_amount));
    final_amount
}

pub fn filter_gateway_card_info_for_max_register_amount(
    txn_detail: ETTD::TxnDetail,
    txn_card_info: ETTCa::TxnCardInfo,
    merchant_gateway_card_infos: Vec<ETMGCI::MerchantGatewayCardInfo>,
    amount: Money,
) -> Vec<ETMGCI::MerchantGatewayCardInfo> {
    let min_amount = Money::from_double(1.0);
    if is_emandate_amount_filter_needed(&txn_detail, &txn_card_info) {
        merchant_gateway_card_infos
            .into_iter()
            .filter(|mgci| match &mgci.emandateRegisterMaxAmount {
                Some(amt) => amount <= *amt,
                None => amount <= min_amount,
            })
            .collect()
    } else {
        merchant_gateway_card_infos
    }
}

pub fn is_emandate_amount_filter_needed(
    txn_detail: &ETTD::TxnDetail,
    txn_card_info: &ETTCa::TxnCardInfo,
) -> bool {
    is_emandate_register_transaction(txn_detail)
        && matches!(
            txn_card_info.paymentMethodType,
            PaymentMethodType::Card
                | PaymentMethodType::NB
                | PaymentMethodType::Aadhaar
                | PaymentMethodType::PAN
        )
}

pub fn is_emandate_register_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    txn_detail.txnObjectType == ETTD::TxnObjectType::EmandateRegister
}

pub async fn get_card_brand(decider_flow: &mut DeciderFlow<'_>) -> Option<String> {
    let c_card_brand = decider_flow.writer.cardBrand.clone();
    if let Some(cb) = c_card_brand {
        return Some(cb);
    }

    let m_isin = decider_flow.get().dpTxnCardInfo.card_isin.clone();
    if let Some(ref isin) = m_isin {
        if isin.is_empty() {
            decider_flow.writer.cardBrand = None;
            return None;
        }

        let card_isin = isin.chars().filter(|c| c.is_digit(10)).collect::<String>();
        //Older Way: let maybe_card_isin = preview(ETCa::isin_text, &card_isin);
        let maybe_card_isin = ETCa::isin::to_isin(card_isin).ok();
        if let Some(card_isin) = maybe_card_isin {
            //Older Way: let key = format!("card_brand_{}", review(ETCa::isin_text, &card_isin));
            let key = format!("card_brand_{}", ETCa::isin::Isin::to_text(&card_isin));
            match decider_flow
                .state()
                .redis_conn
                .get_key::<String>(&key, "card_brand")
                .await
                .ok()
            {
                Some(val) => {
                    decider_flow.writer.cardBrand = Some(val.clone());
                    Some(val)
                }
                None => {
                    println!("getCardBrand: Not Found in redis querying DB");
                    match get_card_brand_from_db(m_isin).await {
                        Some(cb) => {
                            decider_flow.writer.cardBrand = Some(cb.clone());
                            Some(cb)
                        }
                        None => {
                            decider_flow.writer.cardBrand = None;
                            None
                        }
                    }
                }
            }
        } else {
            decider_flow.writer.cardBrand = None;
            None
        }
    } else {
        decider_flow.writer.cardBrand = None;
        None
    }
}

pub async fn get_card_brand_from_db(isin: Option<String>) -> Option<String> {
    if let Some(isin) = isin {
        let maybe_card_isin = (ETCa::isin::to_isin(isin));
        if let Ok(card_isin) = maybe_card_isin {
            match ETCa::card_info::getCardInfoByIsin(card_isin).await {
                Some(card_info) => Some(card_info.card_switch_provider.to_uppercase()),
                None => None,
            }
        } else {
            None
        }
    } else {
        None
    }
}

pub fn get_metadata(decider_flow: &mut DeciderFlow<'_>) -> HashMap<String, String> {
    let mstmeta = decider_flow.writer.metadata.clone();
    if let Some(m) = mstmeta {
        return m;
    }

    let m_ord_meta_v2 = decider_flow.get().dpOrderMetadata.metadata.clone();
    if let Some(text_meta) = m_ord_meta_v2 {
        let new_meta = decode_metadata(&text_meta).clone();
        decider_flow.writer.metadata = Some(new_meta.clone());
        new_meta
    } else {
        decider_flow.writer.metadata = Some(HashMap::new());
        HashMap::new()
    }
}

pub fn is_enabled_for_all_mgas(decider_flow: &mut DeciderFlow<'_>) -> bool {
    get_metadata(decider_flow).contains_key("enabledForAllMgas")
}

pub async fn get_split_settlement_details(
    decider_flow: &mut DeciderFlow<'_>,
) -> Result<SplitSettlementDetails, String> {
    let meta = get_metadata(decider_flow);
    if let Some(str) = meta.get("split_settlement_details") {
        serde_json::from_str(str).map_err(|e| e.to_string())
    } else {
        Err("Value for split_settlement_details key not found".to_string())
    }
}

pub async fn metric_tracker_log(stage: &str, flowtype: &str, log_data: MessageFormat) {
    if let Some(true) = RService::findByNameFromRedis(C::metricTrackingLogDataKey.get_key()).await {
        println!(
            "metric_tracking_log: {:?}",
            serde_json::to_string(&log_data).ok()
        );
        // log_info_v("metric_tracking_log", log_data).await;
    }
}

pub async fn get_metric_log_format(
    decider_flow: &mut DeciderFlow<'_>,
    stage: &str,
) -> MessageFormat {
    // let mp = decider_flow.write.sr.sr_metric_log_data.clone();
    let mp = decider_flow.writer.srMetricLogData.clone();
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let txn_card_info = decider_flow.get().dpTxnCardInfo.clone();
    let order_reference = decider_flow.get().dpOrder.clone();
    let x_req_id = decider_flow.logger.get("x-request-id");
    let payment_source_m = txn_card_info
        .paymentSource
        .as_ref()
        .and_then(|ps| last(split("@", ps)));

    MessageFormat {
        model: txn_detail.txnObjectType.to_string(),
        log_type: "APP_EVENT".to_string(),
        payment_method: txn_card_info.paymentMethod.clone(),
        payment_method_type: txn_card_info.paymentMethodType.to_text().to_string(),
        payment_source: payment_source_m,
        source_object: txn_detail.sourceObject.clone(),
        txn_detail_id: txn_detail.id.clone(),
        stage: stage.to_string(),
        merchant_id: merchant_id_to_text(order_reference.merchantId.clone()),
        txn_uuid: txn_detail.txnUuid.clone(),
        order_id: order_reference.orderId.orderId.clone(),
        card_type: txn_card_info
            .card_type
            .as_ref()
            .map(|ct| ct.to_string())
            .unwrap_or_default(),
        auth_type: txn_card_info.authType.as_ref().map(|at| at.to_string()),
        bank_code: fetch_juspay_bank_code(&txn_card_info),
        x_request_id: x_req_id.cloned(),
        log_data: serde_json::to_value(mp).unwrap(),
    }
}

// pub async fn log_gateway_decider_approach(m_decided_gateway: Option<ETG::Gateway>, m_top_gateway_before_sr_downtime_evaluation: Option<ETG::Gateway>, sr_elimination_info: Vec<String>, gateway_decider_approach: types::GatewayDeciderApproach, is_primary_gateway: Option<bool>, functional_gateways: Vec<ETG::Gateway>, experiment_tag: Option<String>) {
//     let txn_detail = get_context().dp_txn_detail.clone();
//     let order_reference = get_context().dp_order.clone();
//     let txn_card_info = get_context().dpTxnCardInfo.clone();
//     let x_req_id = get_logger_context("x-request-id").await;
//     let txn_creation_time = txn_detail.date_created.to_rfc3339();
//     let mp = DeciderApproachLogData {
//         m_decided_gateway,
//         gateway_decider_approach,
//         m_top_gateway_before_sr_downtime_evaluation,
//         sr_elimination_info: sr_elimination_info.join("_"),
//         is_primary_gateway,
//         functional_gateways,
//         experiment_tag,
//         txn_creation_time,
//     };
//     let payment_source_m = txn_card_info.payment_source.as_ref().and_then(|ps| last(split(Pattern("@"), ps)));

//     log_debug_v("DeciderApproachData", mp).await;
//     metric_tracker_log("GATEWAY_DECIDER_APPROACH", "DECIDER", MessageFormat {
//         model: txn_detail.txn_object_type.to_string(),
//         log_type: "APP_EVENT".to_string(),
//         payment_method: txn_card_info.payment_method.clone(),
//         payment_method_type: txn_card_info.payment_method_type.to_string(),
//         payment_source: payment_source_m,
//         source_object: txn_detail.source_object.clone(),
//         txn_detail_id: txn_detail.id.clone(),
//         stage: "GATEWAY_DECIDER_APPROACH".to_string(),
//         merchant_id: get_m_id(order_reference.merchant_id.clone()),
//         txn_uuid: txn_detail.txn_uuid.clone(),
//         order_id: order_reference.order_id.un_order_id.clone(),
//         card_type: txn_card_info.card_type.as_ref().map(|ct| ct.to_string()).unwrap_or_default(),
//         auth_type: txn_card_info.auth_type.as_ref().map(|at| unsafe_extract_secret(at).to_string()).unwrap_or_default(),
//         bank_code: fetch_juspay_bank_code(&txn_card_info),
//         x_request_id: x_req_id,
//         log_data: A::to_value(mp).unwrap(),
//     }).await;
// }

pub fn round_off_to_3(db: f64) -> f64 {
    (db * 1000.0).round() / 1000.0
}

pub fn text_to_gateway(t: &str) -> Option<ETG::Gateway> {
    match ETG::text_to_gateway(t) {
        Err(_) => None,
        Ok(g) => Some(g),
    }
}

pub fn get_true_string(val: Option<String>) -> Option<String> {
    match val {
        Some(ref value) if value.is_empty() => None,
        _ => val,
    }
}

pub async fn get_card_bin_from_token_bin(
    decider_flow: &mut DeciderFlow<'_>,
    length: usize,
    token_bin: &str,
) -> String {
    let key = format!("token_bin_{}", token_bin);
    let redis = &decider_flow.state().redis_conn;
    match redis.get_key::<String>(&key, "card_brand").await.ok() {
        Some(bin) => bin.chars().take(length).collect(),
        None => match get_extended_token_bin_info(token_bin).await {
            Some(token_bin_info) => {
                redis.set_key(&key, &token_bin_info.cardBin).await;
                token_bin_info.cardBin.chars().take(length).collect()
            }
            None => {
                println!(
                    "{}",
                    &format!(
                        "getCardBinFromTokenBin: tokenBin <> cardbin mapping not present {}",
                        token_bin
                    ),
                );
                token_bin.to_string()
            }
        },
    }
}

pub fn string_to_int_default_zero(str: &str) -> i32 {
    str.parse().unwrap_or(0)
}

pub async fn get_extended_token_bin_info(token_bin_etbi: &str) -> Option<ETTB::TokenBinInfo> {
    let token_bin_list: Vec<String> = (6..=9)
        .map(|len| token_bin_etbi.chars().take(len).collect())
        .collect();
    let token_bin_infos = ETTB::getAllTokenBinInfoByTokenBins(token_bin_list).await;
    let token_bin_infos_in_db: Vec<i32> = token_bin_infos
        .iter()
        .map(|tbi| string_to_int_default_zero(&tbi.tokenBin))
        .collect();
    let token_bin = token_bin_infos_in_db
        .iter()
        .max()
        .map(|&max| max.to_string())
        .unwrap_or_default();
    token_bin_infos
        .into_iter()
        .find(|bin_info| bin_info.tokenBin == token_bin)
}

pub fn split(pattern: &str, text: &str) -> Vec<String> {
    if pattern.is_empty() {
        text.chars().map(|c| c.to_string()).collect()
    } else {
        text.split(pattern).map(|s| s.to_string()).collect()
    }
}

pub fn last<T>(vec: Vec<T>) -> Option<T> {
    vec.into_iter().last()
}

// pub fn decode_from_text<T: DeserializeOwned>(text: &str) -> Option<T> {
//     serde_json::from_str(text).ok()
// }


pub fn intercalate_without_empty_string(intercalate_with: &str, input_texts: &Vec<String>) -> String {
    // Replace empty strings with "UNKNOWN"
    let modified_texts: Vec<&str> = input_texts
        .iter()
        .map(|text| if text.is_empty() { "UNKNOWN" } else { text })
        .collect();

    // Join the modified texts with the separator
    modified_texts.join(intercalate_with)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnabledGatewaysForBrand {
    EnabledAllGateways(String),
    ListOfGateways(Vec<String>),
}

async fn get_token_supported_gateways(decider_flow: &mut DeciderFlow<'_>, txn_detail: ETTD::TxnDetail, txn_card_info: ETCa::txn_card_info::TxnCardInfo, flow: String, m_internal_meta: Option<InternalMetadata>) -> Option<Vec<ETG::Gateway>> {
    let token_type = get_stored_card_vault_provider(m_internal_meta.clone());
    let brand = txn_card_info.cardSwitchProvider.as_ref().map_or("DEFAULT".to_string(), |secret| secret.peek().to_string());
    let token_provider = get_token_provider(m_internal_meta, &txn_card_info, &brand);
    if token_type == "NETWORK_TOKEN" {
        get_network_token_supported_gateways(&flow, brand).await
    } else {
        get_token_supported_gateways_key(brand, token_type, token_provider, flow).await
    }
}

// fn filtered_gateways_merchant_config(m_list_of_gateways: Option<Vec<ETG::Gateway>>, flow: PaymentFlow, m_acc: MerchantAccount, brand: String) -> Option<Vec<Gateway>> {
//     let (status, merchant_config) = (None, None); // Placeholder for MC.getMerchantConfigStatusAndvalueForPaymentFlow
//     match status {
//         Redis::PaymentFlowNotEligible => Ok(m_list_of_gateways),
//         Redis::Disabled => Ok(None),
//         Redis::Enabled => match merchant_config {
//             None => Ok(m_list_of_gateways),
//             Some(config_value) => {
//                 let m_enabled_gateways = get_value_from_text("enabledGateways", &config_value);
//                 let enabled_gateways_list_object = get_enabled_gateway_for_brand(&brand, m_enabled_gateways);
//                 match A::from_str::<EnabledGatewaysForBrand>(&A::to_string(&enabled_gateways_list_object).unwrap()) {
//                     Ok(enabled_gateway_list) => match enabled_gateway_list {
//                         EnabledGatewaysForBrand::ENABLED_ALL_GATEWAYS(_) => Ok(m_list_of_gateways),
//                         EnabledGatewaysForBrand::LIST_OF_GATEWAYS(list_of_gws) => {
//                             let list_of_all_gateways_before_filter = text_to_gateway_t(&list_of_gws).unwrap_or_default();
//                             let list_of_all_gateways_after_filter = list_of_all_gateways_before_filter
//                                 .into_iter()
//                                 .filter(|gw| m_list_of_gateways.as_ref().map_or(false, |gws| gws.contains(gw)))
//                                 .collect::<Vec<_>>();
//                             if list_of_all_gateways_after_filter.is_empty() {
//                                 Ok(None)
//                             } else {
//                                 Ok(Some(list_of_all_gateways_after_filter))
//                             }
//                         }
//                     },
//                     Err(err) => {
//                         L::log_error_t("MERCHANT_CONFIG_DECODE_FAILED", &Text::from(err.to_string()));
//                         Ok(m_list_of_gateways)
//                     }
//                 }
//             }
//         }
//     }
// }

async fn get_network_token_supported_gateways(flow: &String, network: String) -> Option<Vec<ETG::Gateway>> {
    match flow.as_str() {
        "OTP" => RService::findByNameFromRedis(C::getTokenRepeatOtpGatewayKey(network).get_key()).await.unwrap_or_default(),
        "CVV_LESS" => RService::findByNameFromRedis(C::getTokenRepeatCvvLessGatewayKey(network).get_key()).await.unwrap_or_default(),
        "MANDATE" => RService::findByNameFromRedis(C::getTokenRepeatMandateGatewayKey(network).get_key()).await.unwrap_or_default(),
        "CARD" => RService::findByNameFromRedis(C::getTokenRepeatGatewayKey(network).get_key()).await.unwrap_or_default(),
        _ => Some(vec![]),
    }
}

async fn get_token_supported_gateways_key(brand: String, provider_category: String, token_provider: String, flow: String) -> Option<Vec<ETG::Gateway>> {
    if brand == token_provider {
        RService::findByNameFromRedis(C::TOKEN_SUPPORTED_GATEWAYS(brand, None, provider_category, flow).get_key()).await.unwrap_or_default()
    } else {
        RService::findByNameFromRedis(C::TOKEN_SUPPORTED_GATEWAYS(brand, Some(token_provider), provider_category, flow).get_key()).await.unwrap_or_default()
    }
}

fn get_stored_card_vault_provider(m_internal_meta: Option<InternalMetadata>) -> String {
    m_internal_meta.and_then(|meta| meta.storedCardVaultProvider).unwrap_or_else(|| "DEFAULT".to_string())
}

fn get_token_provider(m_internal_meta: Option<InternalMetadata>, txn_card_info: &ETCa::txn_card_info::TxnCardInfo, card_switch_provider: &String) -> String {
    let juspay_bank_code = fetch_juspay_bank_code(txn_card_info);
    match m_internal_meta {
        Some(internal_meta_data) => match internal_meta_data.tokenProvider{
            Some(token_provider) => token_provider,
            None => {
                if internal_meta_data.storedCardVaultProvider == Some("NETWORK_TOKEN".to_string()) {
                    card_switch_provider.clone()
                } else {
                    juspay_bank_code.unwrap_or_else(|| "DEFAULT".to_string())
                }
            }
        },
        None => juspay_bank_code.unwrap_or_else(|| "DEFAULT".to_string()),
    }
}

fn is_token_repeat_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta.and_then(|meta| meta.storedCardVaultProvider).map_or(false, |provider| {
        ["NETWORK_TOKEN", "ISSUER_TOKEN", "ALT_ID"].contains(&provider.as_str())
    })
}

fn is_network_token_repeat_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta.and_then(|meta| meta.storedCardVaultProvider).map_or(false, |provider| provider == "NETWORK_TOKEN")
}

fn is_issuer_token_repeat_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta.and_then(|meta| meta.storedCardVaultProvider).map_or(false, |provider| provider == "ISSUER_TOKEN")
}

fn is_alt_id_based_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta.and_then(|meta| meta.storedCardVaultProvider).map_or(false, |provider| provider == "ALT_ID")
}

fn get_m_id(mid: ETM::id::MerchantId) -> String {
    mid.merchantId.clone()
}

async fn get_upi_handle_list() -> Vec<String> {

    RService::findByNameFromRedis(C::V2_ROUTING_HANDLE_LIST.get_key()).await.unwrap_or_default()
}

async fn get_upi_psp_list() -> Vec<String> {
    RService::findByNameFromRedis(C::V2_ROUTING_PSP_LIST.get_key()).await.unwrap_or_default()
}

async fn get_routing_top_bank_list() -> Vec<String> {
    RService::findByNameFromRedis(C::V2_ROUTING_TOP_BANK_LIST.get_key()).await.unwrap_or_default()
}

async fn get_upi_package_list() -> Vec<String> {
    
    RService::findByNameFromRedis(C::V2_ROUTING_PSP_PACKAGE_LIST.get_key()).await.unwrap_or_default()
}

fn get_bin_list(card_bin: Option<String>) -> Vec<Option<String>> {
    match get_true_string(card_bin) {
        None => vec![],
        Some(bin) => {
            if bin.len() > 6 {
                (6..=9).map(|len| Some(bin[..len].to_string())).collect()
            } else {
                vec![Some(bin)]
            }
        }
    }
}

async fn get_isin_routes_with_extended_bins(card_bin: Option<String>, merchant_id: MerchantId) -> Option<ETIsinR::IsinRoutes> {
    match get_true_string(card_bin) {
        None => None,
        Some(bin) => {
            let bin_list = if bin.len() > 6 {
                (6..=9).map(|len| bin[..len].to_string()).collect()
            } else {
                vec![bin]
            };
            let mut isin_route_list = ETIsinR::find_all_by_isin_and_merchant_id(bin_list, &merchant_id).await;
            isin_route_list.sort_by(|x, y| y.isin.cmp(&x.isin));
            let reverse_list: Vec<_> = isin_route_list.into_iter().collect();
            reverse_list.first().cloned()
        }
    }
}

pub async fn get_card_info_by_bin(card_bin: Option<String>) -> Option<ETCa::card_info::CardInfo> {
    // L::log_debug_v("getCardInfoByBin cardBin", &card_bin);
    match get_true_string(card_bin) {
        None => None,
        Some(bin) => {
            let bin_list = if bin.len() > 6 {
                (6..=9)
                    .map(|len| (ETCa::isin::to_isin(bin[..len].to_string())))
                    .collect()
            } else {
                vec![(ETCa::isin::to_isin(bin))]
            };
            let card_info_list =
                ETCa::card_info::getAllCardInfoByIsins(bin_list.into_iter().flatten().collect())
                    .await;
            let card_bins_in_db = card_info_list
                .iter()
                .map(|ci| get_int_isin(&ci.card_isin))
                .collect::<Vec<_>>();
            let extended_card_bin = card_bins_in_db.into_iter().max().unwrap_or(0).to_string();
            card_info_list.into_iter().find(|bin_info| {
                (ETCa::isin::Isin::to_text(&bin_info.card_isin)) == extended_card_bin
            })
        }
    }
}

fn get_int_isin(isin: &ETCa::isin::Isin) -> i32 {
    let str = ETCa::isin::Isin::to_text(isin);
    let num2 = str.parse::<i32>().unwrap_or(0);
    num2
}

pub fn get_payment_flow_list_from_txn_detail(txn_detail: &ETTD::TxnDetail) -> Vec<String> {
    match txn_detail
        .internalTrackingInfo
        .as_ref()
        .and_then(|info| either_decode_t(info).ok())
    {
        Some(PaymentFlowInfoInInternalTrackingInfo { paymentFlowInfo }) => paymentFlowInfo
            .paymentFlows
            .into_iter()
            .filter(|flow| C::paymentFlowsRequiredForGwFiltering.contains(&flow.as_str()))
            .collect(),
        None => vec![],
    }
}

use crate::decider::gatewaydecider::types::PaymentFlowInfoInInternalTrackingInfo;

fn get_payment_flow_list_from_txn_detail_(txn_detail: &ETTD::TxnDetail) -> Vec<String> {
    match txn_detail
        .internalTrackingInfo
        .as_ref()
        .and_then(|info| either_decode_t(info).ok())
    {
        Some(PaymentFlowInfoInInternalTrackingInfo { paymentFlowInfo }) => {
            paymentFlowInfo.paymentFlows
        }
        None => vec![],
    }
}

fn set_payment_flow_list(decider_flow: &mut DeciderFlow<'_>, payment_flow_list: Vec<String>) {
    decider_flow.writer.paymentFlowList = payment_flow_list;
}

fn check_if_enabled_in_mga(
    mga: &ETM::merchant_gateway_account::MerchantGatewayAccount,
    payment_flow: &str,
    acc_details_flag_to_be_checked: &str,
) -> bool {
    is_payment_flow_enabled_in_mga(mga, payment_flow).unwrap_or_else(|| {
        get_value(acc_details_flag_to_be_checked, &mga.account_details.peek()).unwrap_or(false)
    })
}

fn check_if_no_ds_enabled_in_mga(
    mga: &ETM::merchant_gateway_account::MerchantGatewayAccount,
    payment_flow: &String,
    acc_details_flag_to_be_checked: &String,
) -> bool {
    is_payment_flow_enabled_in_mga(mga, payment_flow).unwrap_or_else(|| {
        get_value(acc_details_flag_to_be_checked, &mga.account_details.peek()).unwrap_or(true)
    })
}

fn is_payment_flow_enabled_in_mga(
    mga: &ETM::merchant_gateway_account::MerchantGatewayAccount,
    payment_flow: &str,
) -> Option<bool> {
    mga.supported_payment_flows
        .as_ref()
        .map(|flows| flows.payment_flow_ids.contains(&payment_flow.to_string()))
}

pub fn get_max_score_gateway(gsm: &types::GatewayScoreMap) -> Option<(ETG::Gateway, f64)> {
    gsm.iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(Ordering::Equal))
        .map(|(gw, score)| (gw.clone(), *score))
}

async fn random_gateway_selection_for_same_score(
    st: &types::GatewayScoreMap,
    max_score: Option<f64>,
) -> Option<ETG::Gateway> {
    match max_score {
        Some(value) => {
            let gws = st
                .iter()
                .filter(|(_, &score)| score == value)
                .map(|(gw, _)| gw.clone())
                .collect::<Vec<_>>();
            if gws.is_empty() {
                None
            } else {
                todo!()
                // EList::shuffle(gws).map(|shuffled_gws| shuffled_gws.into_iter().next())
            }
        }
        None => None,
    }
}

fn get_gateway_decider_approach(
    get_gwsm: &types::GatewayScoreMap,
    gateway_decider_approach: types::GatewayDeciderApproach,
) -> types::GatewayDeciderApproach {
    let gw_set = get_gwsm.keys().cloned().collect::<HashSet<_>>();
    if !gw_set.is_empty() {
        if gw_set.len() > 1 {
            gateway_decider_approach
        } else {
            types::GatewayDeciderApproach::DEFAULT
        }
    } else {
        types::GatewayDeciderApproach::NONE
    }
}

pub fn modify_gateway_decider_approach(
    gw_decider_approach: types::GatewayDeciderApproach,
    down_time: types::DownTime,
) -> types::GatewayDeciderApproach {
    match gw_decider_approach {
        types::GatewayDeciderApproach::SR_SELECTION_V3_ROUTING => match down_time {
            types::DownTime::ALL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V3_ALL_DOWNTIME_ROUTING
            }
            types::DownTime::GLOBAL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V3_GLOBAL_DOWNTIME_ROUTING
            }
            types::DownTime::DOWNTIME => types::GatewayDeciderApproach::SR_V3_DOWNTIME_ROUTING,
            types::DownTime::NO_DOWNTIME => types::GatewayDeciderApproach::SR_SELECTION_V3_ROUTING,
        },
        types::GatewayDeciderApproach::SR_V3_HEDGING => match down_time {
            types::DownTime::ALL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V3_ALL_DOWNTIME_HEDGING
            }
            types::DownTime::GLOBAL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V3_GLOBAL_DOWNTIME_HEDGING
            }
            types::DownTime::DOWNTIME => types::GatewayDeciderApproach::SR_V3_DOWNTIME_HEDGING,
            types::DownTime::NO_DOWNTIME => types::GatewayDeciderApproach::SR_V3_HEDGING,
        },
        types::GatewayDeciderApproach::SR_SELECTION_V2_ROUTING => match down_time {
            types::DownTime::ALL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V2_ALL_DOWNTIME_ROUTING
            }
            types::DownTime::GLOBAL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V2_GLOBAL_DOWNTIME_ROUTING
            }
            types::DownTime::DOWNTIME => types::GatewayDeciderApproach::SR_V2_DOWNTIME_ROUTING,
            types::DownTime::NO_DOWNTIME => types::GatewayDeciderApproach::SR_SELECTION_V2_ROUTING,
        },
        types::GatewayDeciderApproach::SR_V2_HEDGING => match down_time {
            types::DownTime::ALL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V2_ALL_DOWNTIME_HEDGING
            }
            types::DownTime::GLOBAL_DOWNTIME => {
                types::GatewayDeciderApproach::SR_V2_GLOBAL_DOWNTIME_HEDGING
            }
            types::DownTime::DOWNTIME => types::GatewayDeciderApproach::SR_V2_DOWNTIME_HEDGING,
            types::DownTime::NO_DOWNTIME => types::GatewayDeciderApproach::SR_V2_HEDGING,
        },
        _ => match down_time {
            types::DownTime::ALL_DOWNTIME => types::GatewayDeciderApproach::PL_ALL_DOWNTIME_ROUTING,
            types::DownTime::GLOBAL_DOWNTIME => {
                types::GatewayDeciderApproach::PL_GLOBAL_DOWNTIME_ROUTING
            }
            types::DownTime::DOWNTIME => types::GatewayDeciderApproach::PL_DOWNTIME_ROUTING,
            types::DownTime::NO_DOWNTIME => types::GatewayDeciderApproach::PRIORITY_LOGIC,
        },
    }
}

pub fn get_juspay_bank_code_from_internal_metadata(txn_detail: &ETTD::TxnDetail) -> Option<String> {
    txn_detail.internalMetadata.clone()
}

pub fn get_ref_id_value(
    maybe_ref_id: Option<ETM::merchant_gateway_account::MgaReferenceId>,
) -> String {
    match maybe_ref_id {
        Some(value) => value.mga_reference_id,
        _ => String::new(),
    }
}

pub fn decider_filter_order(filter_name: &str) -> i32 {
    match filter_name {
        "getFunctionalGateways" => 1,
        "filterFunctionalGatewaysForCurrency" => 2,
        "filterFunctionalGateways" => 3,
        "filterFunctionalGatewaysForBrand" => 4,
        "filterFunctionalGatewaysForAuthType" => 5,
        "filterFunctionalGatewaysForValidationType" => 6,
        "filterFunctionalGatewaysForEmi" => 7,
        "filterFunctionalGatewaysForTxnOfferDetails" => 8,
        "filterFunctionalGatewaysForPaymentMethod" => 9,
        "filterFunctionalGatewaysForTokenProvider" => 10,
        "filterFunctionalGatewaysForWallet" => 11,
        "filterFunctionalGatewaysForNbOnly" => 12,
        "filterFunctionalGatewaysForConsumerFinance" => 13,
        "filterFunctionalGatewaysForUpi" => 14,
        "filterFunctionalGatewaysForTxnType" => 15,
        "filterFunctionalGatewaysForTxnDetailType" => 16,
        "filterFunctionalGatewaysForReward" => 17,
        "filterFunctionalGatewaysForCash" => 18,
        "filterFunctionalGatewaysForSplitSettlement" => 19,
        "preferredGateway" => 20,
        "filterEnforcement" => 21,
        "filterFunctionalGatewaysForMerchantRequiredFlow" => 22,
        "filterGatewaysForEMITenureSpecficGatewayCreds" => 23,
        "filterGatewaysForMGASelectionIntegrity" => 24,
        "FilterFunctionalGatewaysForOTM" => 25,
        _ => 26,
    }
}

pub const SR_STALE_SCORE_LOG: &str = "SR stale score";

pub async fn log_sr_stale(orbd: OptimizationRedisBlockData, merchant_id: String, key: String, gateway_scores: GatewayScoreMap) {
    if let Some(gateway_score_detail) = orbd.aggregate.last() {
        let block_time_period = get_block_time_period(&merchant_id).await;
        let current_time_stamp_in_millis = get_current_date_in_millis();
        if (current_time_stamp_in_millis - gateway_score_detail.timestamp as u128) > (4 * block_time_period as u128) {
            // log().await;
        }
    }
}

async fn log() {
    todo!()
    // L::log_info_v(SR_STALE_SCORE_LOG, SRStaleScoreLog {
    //     score_key: key,
    //     merchant_id: merchant_id,
    //     gateway_scores: gateway_scores.into_iter().collect(),
    // }).await;
}


pub async fn get_block_time_period(merchant_id: &str) -> i64 {
    match RService::findByNameFromRedis::<ConfigurableBlock>(C::OPTIMIZATION_ROUTING_CONFIG(merchant_id.to_string()).get_key()).await {
        Some(config_block) => config_block.block_timeperiod.round() as i64,
        None => match RService::findByNameFromRedis::<ConfigurableBlock>(C::DEFAULT_OPTIMIZATION_ROUTING_CONFIG.get_key()).await {
            Some(config_block) => config_block.block_timeperiod.round() as i64,
            None => 1800000,
        },
    }
}

// pub async fn decode_and_log_error<T: DeserializeOwned>(error_tag: &str, a: &[u8]) -> Option<T> {
//     match serde_json::from_slice(a) {
//         Ok(value) => Some(value),
//         Err(e) => {
//             // L::log_error_v(error_tag, &e.to_string()).await;
//             None
//         }
//     }
// }

pub fn compute_block_weights(
    weight_arr: &[(f64, i32)],
    num_blocks: i32,
    prev_block_weight: f64,
) -> f64 {
    find_weight_and_index(weight_arr, num_blocks) * prev_block_weight
}

pub fn find_weight_and_index(weight_arr: &[(f64, i32)], i: i32) -> f64 {
    weight_arr
        .iter()
        .find(|&&(_, t)| t == 0 || t < i)
        .map_or(1.0, |&(score, _)| score)
}


pub fn get_date_in_format(date_text: &str, format_text: &str) -> Result<String, String> {
    // Create a custom format description from the input format
    let parse_format = parse(format_text)
        .map_err(|_| "Invalid input format".to_string())?;

    // Parse the date
    let parsed_date = Date::parse(date_text, &parse_format)
        .map_err(|_| "Invalid date".to_string())?;

    // Format the date in dd-mm-yyyy format
    parsed_date
        .format(&parse("[day]-[month]-[year]")
            .map_err(|_| "Failed to create output format".to_string())?)
        .map_err(|_| "Failed to format date".to_string())
}


pub async fn get_experiment_tag(utc_time: UtcDateTime, dim: &str) -> Option<String> {

    match get_date_in_format(&utc_time.to_string(), "%Y-%m-%d %H:%M:%S %Z") {
        Ok(date) => Some(format!("EXPERIMENT_{}_{}", dim, date)),
        Err(e) => {
            // L::log_error_v("getExperimentTag", &e).await;
            None
        }
    }
}

pub async fn create_moving_window_and_score(queue_key: &str, score_key: &str, score: i32, score_list: &Vec<String>) {
    todo!()
    // let result = RC::multi_exec(redis_name, |k| {
    //     RC::del_tx(&[queue_key.as_bytes()], k);
    //     RC::lpush_tx(queue_key.as_bytes(), &score_list.iter().map(|s| s.as_bytes()).collect::<Vec<_>>(), k);
    //     RC::set_tx(score_key.as_bytes(), score.to_string().as_bytes(), k);
    //     RC::expire_tx(queue_key.as_bytes(), 10000000, k);
    //     RC::expire_tx(score_key.as_bytes(), 10000000, k);
    // }).await;

    // match result {
    //     Err(reply) => L::log_error_v("createMovingWindow", &format!("Error while creating queue in redis - returning Nothing, {:?}", reply)).await,
    //     Ok(T::TxSuccess(_)) => (),
    //     Ok(T::TxAborted) => L::log_error_v("createMovingWindow", "Error while creating queue in redis - returning Nothing, aborted").await,
    //     Ok(T::TxError(e)) => L::log_error_v("createMovingWindow", &format!("Error while creating queue in redis - returning Nothing, {:?}", e)).await,
    // }
}

pub fn get_sr_v3_latency_threshold(sr_v3_input_config: Option<SrV3InputConfig>, pmt: &str, pm: &str) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(&config.subLevelInputConfig, pmt, pm, |x| x.latencyThreshold.is_some())
            .and_then(|sub_config| sub_config.latencyThreshold)
            .or(config.defaultLatencyThreshold)
    })
}

pub fn get_sr_v3_bucket_size(sr_v3_input_config: Option<SrV3InputConfig>, pmt: &str, pm: &str) -> Option<i32> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(&config.subLevelInputConfig, pmt, pm, |x| x.bucketSize.is_some())
            .and_then(|sub_config| sub_config.bucketSize)
            .or(config.defaultBucketSize)
            .filter(|&size| size > 0)
    })
}

pub fn get_sr_v3_hedging_percent(sr_v3_input_config: Option<SrV3InputConfig>, pmt: &str, pm: &str) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(&config.subLevelInputConfig, pmt, pm, |x| x.hedgingPercent.is_some())
            .and_then(|sub_config| sub_config.hedgingPercent)
            .or(config.defaultHedgingPercent)
            .filter(|&percent| percent >= 0.0)
    })
}

pub fn get_sr_v3_lower_reset_factor(sr_v3_input_config: Option<SrV3InputConfig>, pmt: &str, pm: &str) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(&config.subLevelInputConfig, pmt, pm, |x| x.lowerResetFactor.is_some())
            .and_then(|sub_config| sub_config.lowerResetFactor)
            .or(config.defaultLowerResetFactor)
            .filter(|&factor| factor >= 0.0)
    })
}

pub fn get_sr_v3_upper_reset_factor(sr_v3_input_config: Option<SrV3InputConfig>, pmt: &str, pm: &str) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(&config.subLevelInputConfig, pmt, pm, |x| x.upperResetFactor.is_some())
            .and_then(|sub_config| sub_config.upperResetFactor)
            .or(config.defaultUpperResetFactor)
            .filter(|&factor| factor >= 0.0)
    })
}

pub fn get_sr_v3_gateway_sigma_factor(sr_v3_input_config: Option<SrV3InputConfig>, pmt: &str, pm: &str, gw: &Gateway) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(&config.subLevelInputConfig, pmt, pm, |x| x.gatewayExtraScore.as_ref().map_or(false, |scores| scores.iter().any(|score| score.gatewayName == gateway_to_text(gw))))
            .and_then(|sub_config| find_gateway_sigma_factor(&sub_config.gatewayExtraScore, gw))
            .or_else(|| find_gateway_sigma_factor(&config.defaultGatewayExtraScore, gw))
    })
}

fn find_gateway_sigma_factor(gateway_extra_score: &Option<Vec<GatewayWiseExtraScore>>, gw: &Gateway) -> Option<f64> {
    gateway_extra_score.as_ref().and_then(|scores| scores.iter().find(|score| score.gatewayName == gateway_to_text(gw)).map(|score| score.gatewaySigmaFactor))
}

fn get_sr_v3_sub_level_input_config(sub_level_input_config: &Option<Vec<SrV3SubLevelInputConfig>>, pmt: &str, pm: &str, is_input_non_null: impl Fn(&SrV3SubLevelInputConfig) -> bool) -> Option<SrV3SubLevelInputConfig> {
    sub_level_input_config.as_ref().and_then(|configs| {
        configs.iter().find(|config| config.paymentMethodType == Some(pmt.to_string()) && config.paymentMethod == Some(pm.to_string()) && is_input_non_null(config))
            .or_else(|| configs.iter().find(|config| config.paymentMethodType == Some(pmt.to_string()) && is_input_non_null(config)))
    }).cloned()
}

// pub fn filter_upto_pmt(sub_level_input_config: Vec, pmt: Text, is_input_non_null: fn(&SrV3SubLevelInputConfig) -> bool) -> Option {
// sub_level_input_config.into_iter().find(|x| x.payment_method_type.unwrap_or_default() == pmt && x.payment_method.is_none() && is_input_non_null(x))
// }

// pub fn filter_upto_pm(sub_level_input_config: Vec, pmt: Text, pm: Text, is_input_non_null: fn(&SrV3SubLevelInputConfig) -> bool) -> Option {
// sub_level_input_config.into_iter().find(|x| x.payment_method_type.unwrap_or_default() == pmt && x.payment_method.unwrap_or_default() == pm && is_input_non_null(x))
// }

// pub fn get_payment_method(payment_method_type: Text, pm: Text, source_object: Text) -> Text {
// if payment_method_type == "UPI" && pm == "UPI" {
// source_object
// } else {
// pm
// }
// }

//     pub async fn delete_score_key_if_bucket_size_changes(merchant_bucket_size: i32, sr_gateway_redis_key_map: GatewayRedisKeyMap) -> DeciderFlow<()> {
//     let gateways_with_changed_bucket_size = filter_map(sr_gateway_redis_key_map.into_iter(), |gateway_redis_key| async {
//     check_if_bucket_size_changed(merchant_bucket_size, gateway_redis_key).await.map(|changed| if changed { Some(gateway_redis_key) } else { None })
//     }).await;
//     map(gateways_with_changed_bucket_size, |(, sr_redis_key)| async {
//     RC::r_del(Config::kv_redis(), &[T::intercalate("", &[sr_redis_key, "}score"])].concat()).await;
//     }).await;
//     }

//     pub async fn check_if_bucket_size_changed(merchant_bucket_size: i32, gateway_redis_key: (Gateway, RedisKey)) -> DeciderFlow {
//     let (, sr_redis_key) = gateway_redis_key;
//     let queue_key = T::intercalate("", &[sr_redis_key, "}queue"]);
//     match RC::llen(Config::kv_redis(), &TE::encode_utf8(queue_key)).await {
//     Ok(redis_bucket_size) => Ok(redis_bucket_size != merchant_bucket_size as i64),
//     Err(err) => {
//     L::log_error_v("checkIfBucketSizeChanged", "Error while getting queue size in redis - returning True", err).await;
//     Ok(true)
//     }
//     }
//     }

//     pub async fn add_txn_to_hash_map_if_debug_mode(is_debug_mode_enabled: bool, mid: Text, txn_detail: ETTD::TxnDetail) -> DeciderFlow<()> {
//     if is_debug_mode_enabled {
//     let either_pending_txn_key_size = RC::hlen(Config::kv_redis(), &TE::encode_utf8(&format!("{}{}", C::pending_txns_key_prefix(), mid))).await;
//     let pending_txn_key_size = match either_pending_txn_key_size {
//     Ok(size) => size,
//     Err(err) => {
//     L::log_error_v("addTxnToHashMapIfDebugMode", "Error while getting hash map size in redis - returning max size", err).await;
//     10000
//     }
//     };
//     if pending_txn_key_size < 10000 {
//     RC::r_hset_b(Config::kv_redis(), &TE::encode_utf8(&format!("{}{}", C::pending_txns_key_prefix(), mid)), &TE::encode_utf8(&txn_detail.txn_uuid), "1").await;
//     } else {
//     log_info_t("addTxnToHashMapIfDebugMode", &format!("Size limit reached for storing pending txns in SRV3 debug mode, key: {}{}", C::pending_txns_key_prefix(), mid)).await;
//     }
//     } else {
//     RC::r_del(Config::kv_redis(), &[C::pending_txns_key_prefix(), mid].concat()).await;
//     }
//     }

//     pub async fn check_if_bin_is_eligible_for_emi(card_isin: Option, juspay_bank_code: Option, card_type: Option) -> bool {
//     if let (Some(card_isin), Some(juspay_bank_code), Some(card_type)) = (card_isin, juspay_bank_code, card_type) {
//     let bin_check_mandated_banks: Option<Vec> = RService::findByNameFromRedis(C::get_emi_bin_validation_supported_banks_key()).await;
//     let should_do_bin_validation = bin_check_mandated_banks.map_or(false, |banks| banks.contains(&format!("{}::{}", juspay_bank_code, card_type)));
//     if should_do_bin_validation {
//     let bin_list = get_bin_list(Some(card_isin)).into_iter().collect::<Vec<_>>();
//     let emi_eligible_bins = UEI::get_eligibility_info(bin_list, UEI::BIN, juspay_bank_code, PF::PG_EMI).await;
//     !emi_eligible_bins.is_empty()
//     } else {
//     true
//     }
//     } else {
//     true
//     }
//     }

// pub fn is_reverse_penny_drop_txn(txn_detail: &ETTD::TxnDetail) -> bool {
//     txn_detail.get_payment_flow_list_from_txn_detail().contains(&"REVERSE_PENNY_DROP".to_string())
// }

// pub fn check_for_reverse_penny_drop_in_mga(mga: &ETM::MerchantGatewayAccount) -> bool {
//     match &mga.supported_payment_flows {
//         None => false,
//         Some(pf) => pf.payment_flow_ids.contains(&"REVERSE_PENNY_DROP".to_string())
//     }
// }

// pub fn get_default_gateway_scoring_data(merchant_id: Text, order_type: Text, payment_method_type: Text, payment_method: Text, is_gri_enabled_for_elimination: bool, is_gri_enabled_for_sr_routing: bool) -> GatewayScoringData {
//     GatewayScoringData {
//     merchant_id,
//     payment_method_type,
//     payment_method,
//     order_type,
//     card_type: None,
//     bank_code: None,
//     auth_type: None,
//     payment_source: None,
//     is_payment_source_enabled_for_sr_routing: false,
//     is_auth_level_enabled_for_sr_routing: false,
//     is_bank_level_enabled_for_sr_routing: false,
//     is_gri_enabled_for_elimination,
//     is_gri_enabled_for_sr_routing,
//     }
// }

// pub async fn get_gateway_scoring_data(txn_detail: ETTD::TxnDetail, txn_card_info: ETCa::TxnCardInfo, merchant: ETM::MerchantAccount) -> DeciderFlow {
//     let merchant_enabled_for_unification = M::is_feature_enabled(C::merchants_enabled_for_score_keys_unification(), &merchant.merchant_id, Config::kv_redis()).await;
//     let merchant_id = &merchant.merchant_id;
//     let order_type = txn_detail.txn_object_type.to_string();
//     let payment_method_type = txn_card_info.payment_method_type.to_string().to_uppercase();
//     let m_source_object = if txn_card_info.payment_method == "UPI" { txn_detail.source_object.unwrap_or_default() } else { txn_card_info.payment_method.clone() };
//     let is_performing_experiment = M::is_feature_enabled(C::is_performing_experiment(), &merchant.merchant_id, Config::kv_redis()).await;
//     let is_gri_enabled_for_elimination = M::is_feature_enabled(C::gateway_reference_id_enabled_merchant(), &merchant.merchant_id, Config::kv_redis()).await;
//     let is_gri_enabled_for_sr_routing = M::is_feature_enabled(C::gw_ref_id_selection_based_enabled_merchant(), &merchant.merchant_id, Config::kv_redis()).await;
//     let default_gateway_scoring_data = get_default_gateway_scoring_data(merchant_id.clone(), order_type, payment_method_type, m_source_object, is_gri_enabled_for_elimination, is_gri_enabled_for_sr_routing);
// }

pub fn get_unified_key(
    gateway_scoring_data: GatewayScoringData,
    score_key_type: ScoreKeyType,
    enforce1d: bool,
    gateway_ref_id_map: types::GatewayReferenceIdMap,
) -> StorageResult<GatewayRedisKeyMap> {
    // let merchant_id = &gateway_scoring_data.merchant_id;
    // let order_type = &gateway_scoring_data.order_type;
    // let payment_method_type = &gateway_scoring_data.payment_method_type;
    // let payment_method = &gateway_scoring_data.payment_method;

    // let gateway_redis_key_map = match score_key_type {
    //     ScoreKeyType::EliminationGlobalKey => {
    //         let key_prefix = C::ELIMINATION_BASED_ROUTING_GLOBAL_KEY_PREFIX;
    //         let (prefix_key, suffix_key) = if payment_method_type == "CARD" {
    //             (
    //                 vec![key_prefix, order_type],
    //                 vec![
    //                     payment_method_type,
    //                     payment_method,
    //                     gateway_scoring_data.card_type.as_deref().unwrap_or(""),
    //                 ],
    //             )
    //         } else {
    //             (vec![key_prefix, order_type], vec![payment_method_type, payment_method])
    //         };

    //         let result_keys = gateway_ref_id_map.iter().fold(
    //             GatewayRedisKeyMap::new(),
    //             |mut acc, (gw, _)| {
    //                 let final_key = intercalate_without_empty_string(
    //                     "_",
    //                     &[
    //                         &prefix_key,
    //                         &[gw],
    //                         &suffix_key,
    //                     ]
    //                     .concat(),
    //                 );
    //                 acc.insert(gw.clone(), final_key);
    //                 acc
    //             },
    //         );
    //         result_keys
    //     }
    //     ScoreKeyType::EliminationMerchantKey => {
    //         let isgri_enabled = gateway_scoring_data.is_gri_enabled_for_elimination;
    //         let key_prefix = C::ELIMINATION_BASED_ROUTING_KEY_PREFIX;
    //         let (prefix_key, suffix_key) = if payment_method_type == "CARD" {
    //             (
    //                 vec![key_prefix, merchant_id, order_type],
    //                 vec![
    //                     payment_method_type,
    //                     payment_method,
    //                     gateway_scoring_data.card_type.as_deref().unwrap_or(""),
    //                 ],
    //             )
    //         } else {
    //             (
    //                 vec![key_prefix, merchant_id, order_type],
    //                 vec![payment_method_type, payment_method],
    //             )
    //         };

    //         let result_keys = gateway_ref_id_map.iter().fold(
    //             GatewayRedisKeyMap::new(),
    //             |mut acc, (gw, ref_id)| {
    //                 let final_key = if isgri_enabled {
    //                     [
    //                         &prefix_key,
    //                         &[gw],
    //                         &suffix_key,
    //                         &[ref_id.as_deref().unwrap_or("")],
    //                     ]
    //                     .concat()
    //                 } else {
    //                     [&prefix_key, &[gw], &suffix_key].concat()
    //                 };
    //                 acc.insert(gw.clone(), intercalate_without_empty_string("_", &final_key));
    //                 acc
    //             },
    //         );
    //         result_keys
    //     }
    //     ScoreKeyType::SrV2Key => {
    //         let key = get_unified_sr_key(&gateway_scoring_data, false, enforce1d).await?;
    //         let gri_sr_v2_cutover = gateway_scoring_data.is_gri_enabled_for_sr_routing;

    //         if gri_sr_v2_cutover {
    //             gateway_ref_id_map.iter().fold(
    //                 GatewayRedisKeyMap::new(),
    //                 |mut acc, (gateway, ref_id)| {
    //                     acc.insert(
    //                         gateway.clone(),
    //                         intercalate_without_empty_string(
    //                             "_",
    //                             &[&key, ref_id.as_deref().unwrap_or("")],
    //                         ),
    //                     );
    //                     acc
    //                 },
    //             )
    //         } else {
    //             let mut map = GatewayRedisKeyMap::new();
    //             map.insert("".to_string(), key);
    //             map
    //         }
    //     }
    //     ScoreKeyType::SrV3Key => {
    //         let base_key = get_unified_sr_key(&gateway_scoring_data, true, enforce1d).await?;
    //         let gri_sr_v2_cutover = gateway_scoring_data.is_gri_enabled_for_sr_routing;

    //         if gri_sr_v2_cutover {
    //             gateway_ref_id_map.iter().fold(
    //                 GatewayRedisKeyMap::new(),
    //                 |mut acc, (gateway, ref_id)| {
    //                     let key = intercalate_without_empty_string(
    //                         "_",
    //                         &[&base_key, ref_id.as_deref().unwrap_or(""), gateway],
    //                     );
    //                     acc.insert(gateway.clone(), key);
    //                     acc
    //                 },
    //             )
    //         } else {
    //             gateway_ref_id_map.iter().fold(
    //                 GatewayRedisKeyMap::new(),
    //                 |mut acc, (gateway, _)| {
    //                     acc.insert(
    //                         gateway.clone(),
    //                         intercalate_without_empty_string("_", &[&base_key, gateway]),
    //                     );
    //                     acc
    //                 },
    //             )
    //         }
    //     }
    //     ScoreKeyType::OutageGlobalKey => {
    //         let key_prefix = C::GLOBAL_LEVEL_OUTAGE_KEY_PREFIX;
    //         let base_key = if payment_method_type == "CARD" {
    //             vec![
    //                 key_prefix,
    //                 payment_method_type,
    //                 payment_method,
    //                 gateway_scoring_data.bank_code.as_deref().unwrap_or(""),
    //                 gateway_scoring_data.card_type.as_deref().unwrap_or(""),
    //             ]
    //         } else if payment_method_type == "UPI" {
    //             vec![
    //                 key_prefix,
    //                 payment_method_type,
    //                 payment_method,
    //                 gateway_scoring_data.payment_source.as_deref().unwrap_or(""),
    //             ]
    //         } else {
    //             vec![key_prefix, payment_method_type, payment_method]
    //         };

    //         let mut map = GatewayRedisKeyMap::new();
    //         map.insert("".to_string(), intercalate_without_empty_string("_", &base_key));
    //         map
    //     }
    //     ScoreKeyType::OutageMerchantKey => {
    //         let key_prefix = C::MERCHANT_LEVEL_OUTAGE_KEY_PREFIX;
    //         let base_key = if payment_method_type == "CARD" {
    //             vec![
    //                 key_prefix,
    //                 merchant_id,
    //                 payment_method_type,
    //                 payment_method,
    //                 gateway_scoring_data.bank_code.as_deref().unwrap_or(""),
    //                 gateway_scoring_data.card_type.as_deref().unwrap_or(""),
    //             ]
    //         } else if payment_method_type == "UPI" {
    //             vec![
    //                 key_prefix,
    //                 merchant_id,
    //                 payment_method_type,
    //                 payment_method,
    //                 gateway_scoring_data.payment_source.as_deref().unwrap_or(""),
    //             ]
    //         } else {
    //             vec![
    //                 key_prefix,
    //                 merchant_id,
    //                 payment_method_type,
    //                 payment_method,
    //             ]
    //         };

    //         let mut map = GatewayRedisKeyMap::new();
    //         map.insert("".to_string(), intercalate_without_empty_string("_", &base_key));
    //         map
    //     }
    // };

    // let gateway_key_log = gateway_redis_key_map
    //     .iter()
    //     .map(|(gw, key)| format!("{} :{}", gw, key))
    //     .collect::<Vec<_>>()
    //     .join(" ");
    // log_info_t("GatewayRedisKeyMap", &gateway_key_log).await;

    // Ok(gateway_redis_key_map)
    todo!()
}

pub async fn get_unified_sr_key(
    gateway_scoring_data: &GatewayScoringData,
    is_sr_v3_metric_enabled: bool,
    enforce1d: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let merchant_id = gateway_scoring_data.merchantId.clone();
    let order_type = gateway_scoring_data.orderType.clone();
    let payment_method_type = gateway_scoring_data.paymentMethodType.clone();
    let payment_method = gateway_scoring_data.paymentMethod.clone();
    let key_prefix = if is_sr_v3_metric_enabled {
        C::gateway_selection_v3_order_type_key_prefix.to_string()
    } else {
        C::gateway_selection_order_type_key_prefix.to_string()
    };
    let base_key = vec![
        key_prefix.clone(),
        merchant_id.clone(),
        order_type.clone(),
        payment_method_type.clone(),
        payment_method.clone(),
    ];

    if enforce1d && payment_method_type == "CARD" {
        let res = &[
            base_key.clone(),
            vec![gateway_scoring_data.cardType.clone().unwrap_or_default()],
        ]
        .concat();
        Ok(intercalate_without_empty_string(
            "_",
            res,
        ))
    } else if enforce1d {
        Ok(intercalate_without_empty_string("_", &base_key))
    } else if payment_method_type == "UPI" {
        if gateway_scoring_data.isPaymentSourceEnabledForSrRouting {
            match payment_method.as_str() {
                "UPI_COLLECT" | "COLLECT" => {
                    let handle_list = get_upi_handle_list().await;
                    let upi_handle = gateway_scoring_data.paymentSource.as_deref().unwrap_or("");
                    let append_handle = if handle_list.contains(&upi_handle.to_string()) {
                        upi_handle
                    } else {
                        ""
                    };
                    Ok(intercalate_without_empty_string(
                        "_",
                        &[
                            base_key.clone(),
                            vec![append_handle.to_string()],
                        ]
                        .concat(),
                    ))
                }
                "UPI_PAY" | "PAY" => {
                    let package_list = get_upi_package_list().await;
                    let upi_package = gateway_scoring_data.paymentSource.as_deref().unwrap_or("");
                    let append_package = if package_list.contains(&upi_package.to_string()) {
                        upi_package
                    } else {
                        ""
                    };
                    Ok(intercalate_without_empty_string(
                        "_",
                        &[
                            base_key.clone(),
                            vec![append_package.to_string()],
                        ]
                        .concat(),
                    ))
                }
                _ => Ok(intercalate_without_empty_string("_", &base_key)),
            }
        } else {
            Ok(intercalate_without_empty_string("_", &base_key))
        }
    } else if payment_method_type == "CARD" {
        let v = &[
            base_key.clone(),
            vec![
                gateway_scoring_data.cardType.clone().unwrap_or_default(),
                gateway_scoring_data.authType.clone().unwrap_or_default(),
            ],
        ]
        .concat();
        if gateway_scoring_data.isAuthLevelEnabledForSrRouting {
            Ok(intercalate_without_empty_string(
                "_",
                v
            ))
        } else if gateway_scoring_data.isBankLevelEnabledForSrRouting {
            let top_bank_list = get_routing_top_bank_list().await;
            let bank_code = gateway_scoring_data.bankCode.as_deref().unwrap_or("UNKNOWN");
            let append_bank_code = if top_bank_list.contains(&bank_code.to_string()) {
                bank_code
            } else {
                ""
            };
            let v = &[
                base_key.clone(),
                vec![
                    gateway_scoring_data.cardType.clone().unwrap_or_default(),
                    append_bank_code.to_string(),
                ],
            ]
            .concat();
            Ok(intercalate_without_empty_string(
                "_",
                v
            ))
        } else {
            let v = &[
                base_key.clone(),
                vec![gateway_scoring_data.cardType.clone().unwrap_or_default()],
            ]
            .concat();
            Ok(intercalate_without_empty_string(
                "_",
                v
            ))
        }
    } else {
        Ok(intercalate_without_empty_string("_", &base_key))
    }
}

use crate::generics::StorageResult;

pub async fn get_consumer_key(
    decider_flow: &mut DeciderFlow<'_>,
    gateway_scoring_data: GatewayScoringData,
    score_key_type: ScoreKeyType,
    enforce1d: bool,
    gateway_list: GatewayList,
) -> StorageResult<GatewayRedisKeyMap> {
    let merchant = decider_flow.get().dpMerchantAccount.clone();
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let gw_ref_id_map = if gateway_scoring_data.isGriEnabledForElimination
        || gateway_scoring_data.isGriEnabledForSrRouting
    {
        let order_ref = decider_flow.get().dpOrder.clone();
        let (meta, pl_ref_id_map) = get_order_metadata_and_pl_ref_id_map(
            decider_flow,
            merchant.enableGatewayReferenceIdBasedRouting,
            &order_ref,
        );
        let gw_ref_ids = gateway_list
            .iter()
            .fold(Ok(HashMap::new()), |acc, gateway| {
                acc.and_then(|mut map| {
                    let gwref_id = get_gateway_reference_id(
                        meta.clone(),
                        gateway,
                        order_ref.clone(),
                        pl_ref_id_map.clone(),
                    );
                    let val = match gwref_id {
                        None => "NULL".to_string(),
                        Some(ref_id) => ref_id.mga_reference_id,
                    };
                    map.insert(format!("{:?}", gateway), Some(val));
                    Ok(map)
                })
            })?;
        set_gw_ref_id(decider_flow, gw_ref_ids.values().next().cloned().flatten());
        // log_debug_v("gwRefId", &gw_ref_ids);
        gw_ref_ids
    } else {
        gateway_list
            .iter()
            .fold(HashMap::new(), |mut acc, gateway| {
                acc.insert(format!("{:?}", gateway), None);
                acc
            })
    };
    let gateway_redis_key_map = get_unified_key(
        gateway_scoring_data,
        score_key_type,
        enforce1d,
        gw_ref_id_map,
    )?;
    Ok(gateway_redis_key_map)
}

fn get_gateway_list(gwsm: GatewayScoreMap) -> Vec<Gateway> {
    gwsm.keys().cloned().collect()
}

async fn set_routing_dimension_and_reference(
    decider_flow: &mut DeciderFlow<'_>,
    gateway_scoring_data: GatewayScoringData,
) -> () {
    let base_dimension = vec![
        gateway_scoring_data.orderType,
        gateway_scoring_data.paymentMethodType.clone(),
        gateway_scoring_data.paymentMethod.clone(),
    ];
    let (final_dimension, routing_dimension_level) =
        if gateway_scoring_data.paymentMethodType == "UPI" {
            if gateway_scoring_data.isPaymentSourceEnabledForSrRouting {
                match gateway_scoring_data.paymentMethod.as_str() {
                    "UPI_COLLECT" | "COLLECT" => {
                        let handle_list = get_upi_handle_list().await;
                        let upi_handle = gateway_scoring_data.paymentSource.unwrap_or_default();
                        let append_handle = if handle_list.contains(&upi_handle) {
                            upi_handle
                        } else {
                            "".to_string()
                        };
                        (
                            intercalate_without_empty_string(
                                ", ",
                                &[base_dimension, vec![append_handle]].concat(),
                            ),
                            "UPI_SOURCE_LEVEL".to_string(),
                        )
                    }
                    "UPI_PAY" | "PAY" => {
                        let package_list = get_upi_package_list().await;
                        let upi_package = gateway_scoring_data.paymentSource.unwrap_or_default();
                        let append_package = if package_list.contains(&upi_package) {
                            upi_package
                        } else {
                            "".to_string()
                        };
                        (
                            intercalate_without_empty_string(
                                ", ",
                                &[base_dimension, vec![append_package]].concat(),
                            ),
                            "UPI_SOURCE_LEVEL".to_string(),
                        )
                    }
                    _ => (
                        intercalate_without_empty_string(", ", &base_dimension),
                        "PM_LEVEL".to_string(),
                    ),
                }
            } else {
                (
                    intercalate_without_empty_string(", ", &base_dimension),
                    "PM_LEVEL".to_string(),
                )
            }
        } else if gateway_scoring_data.paymentMethodType == "CARD" {
            if gateway_scoring_data.isAuthLevelEnabledForSrRouting {
                (
                    intercalate_without_empty_string(
                        ", ",
                        &[
                            base_dimension,
                            vec![
                                gateway_scoring_data.cardType.unwrap_or_default(),
                                gateway_scoring_data.authType.unwrap_or_default(),
                            ],
                        ]
                        .concat(),
                    ),
                    "AUTH_LEVEL".to_string(),
                )
            } else if gateway_scoring_data.isBankLevelEnabledForSrRouting {
                let top_bank_list = get_routing_top_bank_list().await;
                let bank_code = gateway_scoring_data
                    .bankCode
                    .unwrap_or("UNKNOWN".to_string());
                let append_bank_code = if top_bank_list.contains(&bank_code) {
                    bank_code.clone()
                } else {
                    "".to_string()
                };
                (
                    intercalate_without_empty_string(
                        ", ",
                        &[
                            base_dimension,
                            vec![gateway_scoring_data.cardType.unwrap_or_default(), bank_code],
                        ]
                        .concat(),
                    ),
                    "BANK_LEVEL".to_string(),
                )
            } else {
                (
                    intercalate_without_empty_string(
                        ", ",
                        &[
                            base_dimension,
                            vec![gateway_scoring_data.cardType.unwrap_or_default()],
                        ]
                        .concat(),
                    ),
                    "CARD_LEVEL".to_string(),
                )
            }
        } else {
            (
                intercalate_without_empty_string(", ", &base_dimension),
                "PM_LEVEL".to_string(),
            )
        };
    set_routing_dimension(decider_flow, final_dimension);
    set_routing_dimension_level(decider_flow, routing_dimension_level);
    // log_info_t(
    //     "DIMENSION_AND_REFERENCE",
    //     &format!(
    //         "dimension : {}, reference : {}",
    //         final_dimension, routing_dimension_level
    //     ),
    // );
}

fn set_elimination_dimension(
    decider_flow: &mut DeciderFlow<'_>,
    gateway_scoring_data: GatewayScoringData,
) {
    let base_dimension = vec![
        gateway_scoring_data.orderType,
        gateway_scoring_data.paymentMethodType.clone(),
        gateway_scoring_data.paymentMethod,
    ];
    let dimension = if gateway_scoring_data.paymentMethodType == "CARD" {
        intercalate_without_empty_string(
            "/",
            &[
                base_dimension,
                vec![gateway_scoring_data.cardType.unwrap_or_default()],
            ]
            .concat(),
        )
    } else {
        intercalate_without_empty_string("/", &base_dimension)
    };
    set_elimination_dimension_level(decider_flow, dimension)
}

pub fn set_outage_dimension(
    decider_flow: &mut DeciderFlow<'_>,
    gateway_scoring_data: GatewayScoringData,
) -> () {
    let base_dimension = vec![
        gateway_scoring_data.paymentMethodType.clone(),
        gateway_scoring_data.paymentMethod,
    ];
    let dimension = if gateway_scoring_data.paymentMethodType == "CARD" {
        intercalate_without_empty_string(
            "/",
            &[
                base_dimension,
                vec![
                    gateway_scoring_data.cardType.unwrap_or_default(),
                    gateway_scoring_data.bankCode.unwrap_or_default(),
                ],
            ]
            .concat(),
        )
    } else if gateway_scoring_data.paymentMethodType == "UPI" {
        intercalate_without_empty_string(
            "/",
            &[
                base_dimension,
                vec![gateway_scoring_data.paymentSource.unwrap_or_default()],
            ]
            .concat(),
        )
    } else {
        intercalate_without_empty_string("/", &base_dimension)
    };
    set_outage_dimension_level(decider_flow, dimension)
}

fn route_random_traffic_to_explore(
    hedging_percent: f64,
    functional_gateways: Vec<Gateway>,
    tag: String,
) -> bool {
    let num = generate_random_number(
        format!("GatewayDecider::routeRandomTrafficToExplore::{}", tag),
        (0.0, 100.0),
    );
    let explore_hedging_percent = hedging_percent * (functional_gateways.len() as f64);
    num < explore_hedging_percent
}

// fn push_to_stream(decided_gateway: OptionETG::Gateway, final_decider_approach: types::GatewayDeciderApproach, m_priority_logic_tag: Option, current_gateway_score_map: GatewayScoreMap) -> DeciderFlow<()> {
//     if let Some(decided_gateway) = decided_gateway {
//     let merchant = asks(|ctx| ctx.dp_merchant_account);
//     let txn_detail = asks(|ctx| ctx.dp_txn_detail);
//     let txn_card_info = asks(|ctx| ctx.dpTxnCardInfo);
//     let decider_state = get::()?;
//     let t_conf_obj = L::get_option_local::()?;
//     let txn_creation_time = txn_detail.date_created.replace(" ", "T").replace(" UTC", "Z");
//     let merchant_id = get_m_id(merchant.merchant_id);
//     let sr_metric_log_data = decider_state.sr_metric_log_data;
//     let tenant_name_m = t_conf_obj.map(|t| t.tenant_id_v2);
//     let sr_routing_dimension = decider_state.routing_dimension.map(|d| d.replace(", ", "/"));
//     let is_gwswitched_txn = is_gw_switched(&decided_gateway, &decider_state.top_gateway_before_sr_downtime_evaluation);
//     let value = encode_json(&serde_json::json!({
//     "tenant": tenant_name_m,
//     "txn_uuid": txn_detail.txn_uuid,
//     "payment_gateway": decided_gateway,
//     "created_at": txn_creation_time,
//     "merchant_id": merchant_id,
//     "txn_object_type": txn_detail.txn_object_type,
//     "payment_method_type": txn_card_info.payment_method_type,
//     "sr_routing_dimension": sr_routing_dimension,
//     "reference_dimension": decider_state.routing_dimension_level,
//     "is_elimination_triggered": is_gwswitched_txn,
//     "routing_approach": final_decider_approach,
//     "reset_approach": decider_state.reset_approach,
//     "outage_dimension": decider_state.outage_dimension,
//     "elimination_dimension": decider_state.elimination_dimension,
//     "priority_logic_tag": m_priority_logic_tag,
//     "gateway_before_elimination": decider_state.top_gateway_before_sr_downtime_evaluation,
//     "sr_gateway_score": decider_state.sr_gateway_scores,
//     "elimination_score": decider_state.elimination_scores,
//     "srv3_bucket_size": decider_state.srv3_bucket_size,
//     "srv3_hedging_percent": decider_state.sr_v3_hedging_percent,
//     "gateway_reference_id": decider_state.gateway_reference_id,
//     }));
//     let metric_data: Vec<(Text, Text)> = vec![
//     ("log".to_string(), value),
//     ("partition_key".to_string(), txn_detail.txn_uuid.clone())
//     ];
//     log_info_v("ROUTING_ETL_DATA", &value);
//     if tenant_name_m.is_some() {
//     let stream_with_shard = MetricsStreamKeyShard(txn_detail.txn_uuid.clone(), ENV::number_of_streams_for_routing_metrics);
//     let result = Redis::add_to_stream(Config::kv_redis, stream_with_shard, EHT::AutoID, metric_data.iter().map(|(f, s)| (TE::encode_utf8(f), TE::encode_utf8(s))).collect())?;
//     if let Some(EHT::KVDBStreamEntryID(l, r)) = result {
//     return Ok(Some(format!("{}-{}", l, r)));
//     } else {
//     L::log_error_v("redis_set_error", &format!("Error while adding value to cache stream: {}, args: {:?}", RC::get_key(&stream_with_shard), args));
//     return Ok(None);
//     }
//     } else {
//     Ok(None)
//     }
//     } else {
//     Ok(())
//     }
// }

// fn add_metrics_to_stream(txn_uuid: Text, merchant_id: Text, args: Vec<(Text, Text)>) -> DeciderFlow<Option> {
//     if M::is_feature_enabled(C::push_data_to_routing_etl_stream, &merchant_id, &Config::kv_redis)? {
//     let stream_with_shard = MetricsStreamKeyShard(txn_uuid, ENV::number_of_streams_for_routing_metrics);
//     let result = Redis::add_to_stream(&Config::kv_redis, &stream_with_shard, EHT::AutoID, args.iter().map(|(f, s)| (TE::encode_utf8(f), TE::encode_utf8(s))).collect())?;
//     if let Some(EHT::KVDBStreamEntryID(l, r)) = result {
//     Ok(Some(format!("{}-{}", l, r)))
//     } else {
//     L::log_error_v("redis_set_error", &format!("Error while adding value to cache stream: {}, args: {:?}", RC::get_key(&stream_with_shard), args));
//     Ok(None)
//     }
//     } else {
//     Ok(None)
//     }
// }
