use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::types::{self, DeciderFlow};
use crate::error;
use crate::euclid::errors::EuclidErrors;
use crate::euclid::types::SrDimensionConfig;
use crate::feedback::gateway_elimination_scoring::flow::{
    eliminationV2RewardFactor, getPenaltyFactor,
};
use crate::redis::feature::isFeatureEnabled;
use crate::redis::types::ServiceConfigKey;
use crate::types::card::card_type::card_type_to_text;
use crate::types::country::country_iso::CountryISO2;
use crate::types::currency::Currency;
use crate::types::merchant::id::{merchant_id_to_text, MerchantId};
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;
use crate::types::money::internal::Money;
use crate::types::payment::payment_method_type_const::*;
use crate::types::payment_flow::{payment_flows_to_text, PaymentFlow};
use crate::types::service_configuration::find_config_by_name;
use crate::types::user_eligibility_info::{
    get_eligibility_info, identifier_name_to_text, IdentifierName,
};
use crate::utils::{generate_random_number, get_current_date_in_millis};
use crate::{decider, feedback, logger};
use diesel::Identifiable;
use error_stack::ResultExt;
use fred::prelude::{KeysInterface, ListInterface};
use masking::PeekInterface;
use masking::Secret;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::from_value;
use serde_json::{from_slice, from_str, Value};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::option::Option;
use std::result::Result;
use std::string::String;
use std::vec::Vec;
use time::format_description::parse;
use time::{Date, OffsetDateTime};
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
use crate::types::card as ETCa;
// // use types::currency as Curr;
use crate::types::feature as ETF;
// use crate::types::gateway as Gateway;
// // use types::gateway_payment_method as ETGPM;
use super::types::{
    ConfigurableBlock, GatewayList, GatewayRedisKeyMap, GatewayScoreMap, GatewayScoringData,
    GatewayWiseExtraScore, InternalMetadata, MessageFormat, OptimizationRedisBlockData,
    ScoreKeyType, SplitSettlementDetails, SrRoutingDimensions, SrV3InputConfig,
    SrV3SubLevelInputConfig,
};
use crate::types::merchant as ETM;
use crate::types::merchant_gateway_card_info as ETMGCI;
// // use types::merchant_gateway_card_info as ETMGCI;
// // use types::merchant_gateway_payment_method as ETMGPM;
// // use types::money as Money;
use crate::types::card::txn_card_info::{self as ETTCa, auth_type_to_text, AuthType};
use crate::types::order as ETO;
use crate::types::txn_details::types as ETTD;
use crate::types::txn_offer as ETTO;
// use juspay::extra::parsing as P;
use crate::types::gateway as ETG;
use crate::types::gateway_routing_input::{GatewayScore, GatewaySuccessRateBasedRoutingInput};
use crate::types::token_bin_info as ETTB;
// // use utils::config::constants as Config;
// // use utils::logging as EWL;
// // use safe::Safe;
// // use control::category::Category;
// // use juspay::extra::non_empty_text as NET;
use crate::redis::{self, cache as RService};
use crate::types::isin_routes as ETIsinR;
// // use utils::redis as EWRedis;
// // use db::common::types::payment_flows as PF;
// // use utils::redis as Redis;
// // use eulerhs::tenant_redis_layer as RC;
// // use eulerhs::types as EHT;
// // use configs::env_vars as ENV;
use crate::error::StorageError;
use crate::types::gateway_card_info::ValidationType;

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
    txn_card_info.card_type == Some(ETCa::card_type::CardType::Nb)
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
    let secret_json = Some(mga.account_details.peek());
    secret_json
        .and_then(|seamless_value| get_value("seamless", seamless_value))
        .unwrap_or(false)
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

pub fn fetch_juspay_bank_code(txn_card_info: &ETCa::txn_card_info::TxnCardInfo) -> Option<String> {
    txn_card_info
        .paymentSource
        .as_ref()
        .and_then(|source| get_value("juspay_bank_code", source))
}

pub fn get_pl_gw_ref_id_map(decider_flow: &DeciderFlow<'_>) -> HashMap<String, String> {
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

pub fn is_emandate_supported_payment_method(
    txn_card_info: &ETCa::txn_card_info::TxnCardInfo,
) -> bool {
    matches!(
        txn_card_info.paymentMethodType.as_str(),
        CARD | NB | WALLET | UPI | AADHAAR | PAPERNACH | PAN | RTP
    )
}

pub fn is_emandate_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    matches!(
        txn_detail.txnObjectType,
        Some(ETTD::TxnObjectType::EmandateRegister)
            | Some(ETTD::TxnObjectType::TpvEmandateRegister)
            | Some(ETTD::TxnObjectType::EmandatePayment)
            | Some(ETTD::TxnObjectType::TpvEmandatePayment)
    )
}

pub fn is_emandate_payment_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    matches!(
        txn_detail.txnObjectType,
        Some(ETTD::TxnObjectType::EmandatePayment) | Some(ETTD::TxnObjectType::TpvEmandatePayment)
    )
}

pub fn is_reccuring_payment_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    matches!(
        txn_detail.txnObjectType,
        Some(ETTD::TxnObjectType::EmandatePayment)
            | Some(ETTD::TxnObjectType::TpvEmandatePayment)
            | Some(ETTD::TxnObjectType::MandatePayment)
            | Some(ETTD::TxnObjectType::TpvMandatePayment)
    )
}

pub fn is_tpv_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    matches!(
        txn_detail.txnObjectType,
        Some(ETTD::TxnObjectType::TpvPayment)
    )
}

pub fn is_tpv_mandate_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    txn_detail.txnObjectType == Some(ETTD::TxnObjectType::TpvEmandateRegister)
}

pub fn get_merchant_wise_si_bin_key(gw: &String) -> String {
    format!("MERCHANT_WISE_SI_BINS_{}", gw)
}

fn get_merchant_gateway_card_info_feature_name(
    auth_type: Option<&ETCa::txn_card_info::AuthType>,
    validation_type: Option<&ValidationType>,
    gateway: &String,
) -> Option<String> {
    let flow = validation_type
        .map(|v| format!("{}", v))
        .or_else(|| auth_type.map(|a| format!("{}", a)))?;
    Some(format!("MERCHANT_GATEWAY_CARD_INFO_{}_{}", flow, gateway))
}

pub fn is_mandate_transaction(txn: &ETTD::TxnDetail) -> bool {
    matches!(
        txn.txnObjectType,
        Some(ETTD::TxnObjectType::MandateRegister) | Some(ETTD::TxnObjectType::MandatePayment)
    )
}

pub async fn get_merchant_wise_mandate_bin_eligible_gateways(
    merchant_account: &ETM::merchant_account::MerchantAccount,
    mandate_enabled_gateways: &[String],
) -> Vec<String> {
    let merchant_wise_mandate_bin_enforced_gateways: Vec<String> =
        RService::findByNameFromRedis::<Vec<String>>(
            C::MerchantWiseMandateBinEnforcedGateways.get_key(),
        )
        .await
        .unwrap_or_default();
    let merchant_wise_mandate_supported_gateway: Vec<String> =
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
    gateway: &String,
) -> bool {
    let merchant_wise_auth_type_bin_enforced_gateways: Vec<String> =
        RService::findByNameFromRedis::<Vec<String>>(
            C::MerchantWiseAuthTypeBinEnforcedGateways.get_key(),
        )
        .await
        .unwrap_or_default();
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

pub fn get_internal_meta_data(decider_flow: &DeciderFlow<'_>) -> Option<types::InternalMetadata> {
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
    gw: Option<String>,
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

pub fn set_sr_gateway_scores(
    decider_flow: &mut DeciderFlow<'_>,
    gw_scores: Vec<types::GatewayScore>,
) {
    decider_flow.writer.sr_gateway_scores = Some(gw_scores);
}

pub fn set_elimination_scores(
    decider_flow: &mut DeciderFlow<'_>,
    gw_scores: Vec<types::GatewayScore>,
) {
    decider_flow.writer.elimination_scores = Some(gw_scores);
}

pub fn set_srv3_bucket_size(decider_flow: &mut DeciderFlow<'_>, srv3_bucket_size: i32) {
    decider_flow.writer.srv3_bucket_size = Some(srv3_bucket_size);
}

pub fn set_sr_v3_hedging_percent(decider_flow: &mut DeciderFlow<'_>, sr_v3_hedging_percent: f64) {
    decider_flow.writer.sr_v3_hedging_percent = Some(sr_v3_hedging_percent);
}

pub fn get_reset_approach(decider_flow: &DeciderFlow<'_>) -> types::ResetApproach {
    decider_flow.writer.reset_approach.clone()
}

pub fn set_reset_approach(decider_flow: &mut DeciderFlow<'_>, res_app: types::ResetApproach) {
    decider_flow.writer.reset_approach = res_app;
}

pub fn set_is_merchant_enabled_for_dynamic_mga_selection(
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

pub fn get_value<T: DeserializeOwned>(key: &str, json_text: &str) -> Option<T> {
    let parsed: Value = serde_json::from_str(json_text).ok()?;
    let obj = parsed.as_object()?;
    let val = obj.get(key)?;

    serde_json::from_value(val.clone())
        .ok()
        .or_else(|| match val {
            Value::String(s) => match s.as_str() {
                "True" => serde_json::from_str("true").ok(),
                "False" => serde_json::from_str("false").ok(),
                _ => serde_json::from_str(s).ok(),
            },
            _ => None,
        })
}

pub fn is_txn_type_enabled(
    supported_txn_type: Option<&str>,
    payment_method_type: &str,
    txn_type: &str,
) -> bool {
    supported_txn_type
        .and_then(|s| get_value::<Vec<String>>(payment_method_type, s))
        .is_none_or(|types| types.contains(&txn_type.to_string()))
}
pub fn get_value_from_text(key: &str, t: &Value) -> Option<Value> {
    match t {
        Value::Object(map) => map.get(key).cloned(),
        _ => None,
    }
}

fn decode_metadata(text: &str) -> HashMap<String, String> {
    from_str::<HashMap<String, Value>>(text)
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| {
            if let Value::String(s) = v {
                (k, s)
            } else {
                (k, v.to_string())
            }
        })
        .collect()
}

pub fn get_all_possible_ref_ids(
    metadata: HashMap<String, String>,
    oref: ETO::Order,
    pl_ref_id_map: HashMap<String, String>,
) -> Vec<ETM::merchant_gateway_account::MgaReferenceId> {
    let gateway_ref_ids = is_suffix_of_gateway_ref_id(metadata.iter().collect());
    let gateway_ref_ids_from_pl = is_suffix_of_gateway_ref_id(pl_ref_id_map.iter().collect());
    gateway_ref_ids
        .into_iter()
        .chain(gateway_ref_ids_from_pl)
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
    gw: &String,
    oref: ETO::Order,
    pl_ref_id_map: HashMap<String, String>,
) -> Option<ETM::merchant_gateway_account::MgaReferenceId> {
    let meta_res = pl_ref_id_map
        .get(&format!("{}:gateway_reference_id", gw))
        .or_else(|| pl_ref_id_map.get("JUSPAY:gateway_reference_id"))
        .or_else(|| metadata.get(&format!("{}:gateway_reference_id", gw)))
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
    let amount_txn = txn_detail.txnAmount.as_ref().unwrap_or(&def_amount);
    let offers = ETTO::getOffers(&txn_detail.id).await;
    let discount_sum: Money = Money::from_double(
        offers
            .iter()
            .map(|offer| offer.discountAmount.clone())
            .map(|m| m.to_double())
            .sum(),
    );

    amount_txn
        .m_sub(&discount_sum)
        .m_add(&txn_detail.surchargeAmount.unwrap_or(def_amount.clone()))
        .m_add(&txn_detail.taxAmount.unwrap_or(def_amount))
}

pub fn filter_gateway_card_info_for_max_register_amount(
    txn_detail: ETTD::TxnDetail,
    txn_card_info: ETTCa::TxnCardInfo,
    merchant_gateway_card_infos: Vec<ETMGCI::MerchantGatewayCardInfo>,
    amount: Money,
) -> Vec<ETMGCI::MerchantGatewayCardInfo> {
    let min_amount = Money::from_double(10000.0);
    if is_emandate_amount_filter_needed(&txn_detail, &txn_card_info) {
        merchant_gateway_card_infos
            .into_iter()
            .filter(|mgci| match &mgci.emandateRegisterMaxAmount {
                Some(amt) => amount <= Money::from_double(amt.to_double() * 10000.0),
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
            txn_card_info.paymentMethodType.as_str(),
            CARD | NB | AADHAAR | PAN
        )
}

pub fn is_emandate_register_transaction(txn_detail: &ETTD::TxnDetail) -> bool {
    txn_detail.txnObjectType == Some(ETTD::TxnObjectType::EmandateRegister)
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

        let card_isin = isin
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();
        //Older Way: let maybe_card_isin = preview(ETCa::isin_text, &card_isin);
        let maybe_card_isin = ETCa::isin::to_isin(card_isin).ok();
        if let Some(card_isin) = maybe_card_isin {
            //Older Way: let key = format!("card_brand_{}", review(ETCa::isin_text, &card_isin));
            let key = format!("card_brand_{}", ETCa::isin::Isin::to_text(&card_isin));
            match decider_flow
                .state()
                .redis_conn
                .get_key_string(&key)
                .await
                .ok()
            {
                Some(val) => {
                    decider_flow.writer.cardBrand = Some(val.clone());
                    Some(val)
                }
                None => {
                    crate::logger::info!("getCardBrand: Not Found in redis querying DB");
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
        let maybe_card_isin = ETCa::isin::to_isin(isin);
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
    let normalized_log_data = match serde_json::to_value(&log_data) {
        Ok(value) => value,
        Err(e) => {
            crate::logger::error!(
                action = "metric_tracking_log_error",
                "Failed to serialize log_data: {}",
                e
            );
            return;
        }
    };
    crate::logger::info!(
        action = "metric_tracking_log",
        "{}",
        normalized_log_data.to_string(),
    );
}

pub fn get_metric_log_format(decider_flow: &mut DeciderFlow<'_>, stage: &str) -> MessageFormat {
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
        model: txn_detail
            .txnObjectType
            .map(|t| t.to_string())
            .unwrap_or_default(),
        log_type: "APP_EVENT".to_string(),
        payment_method: txn_card_info.paymentMethod.clone(),
        payment_method_type: txn_card_info.paymentMethodType.clone(),
        payment_source: payment_source_m,
        source_object: txn_detail.sourceObject.clone(),
        txn_detail_id: txn_detail.id.clone(),
        stage: stage.to_string(),
        merchant_id: merchant_id_to_text(order_reference.merchantId.clone()),
        txn_uuid: txn_detail.txnUuid.clone(),
        order_id: order_reference.orderId.0.clone(),
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

// ... existing code ...

pub async fn log_gateway_decider_approach(
    decider_flow: &mut DeciderFlow<'_>,
    m_decided_gateway: Option<String>,
    m_top_gateway_before_sr_downtime_evaluation: Option<String>,
    sr_elimination_info: Vec<String>,
    gateway_decider_approach: types::GatewayDeciderApproach,
    is_primary_gateway: Option<bool>,
    functional_gateways: Vec<String>,
    experiment_tag: Option<String>,
) {
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let order_reference = decider_flow.get().dpOrder.clone();
    let txn_card_info = decider_flow.get().dpTxnCardInfo.clone();
    let x_req_id = decider_flow.logger.get("x-request-id").cloned();
    let txn_creation_time = txn_detail.dateCreated.to_string(); // Assuming dateCreated is a DateTime field

    let mp = types::DeciderApproachLogData {
        decided_gateway: m_decided_gateway,
        routing_approach: gateway_decider_approach,
        gateway_before_downtime_evaluation: m_top_gateway_before_sr_downtime_evaluation,
        elimination_level_info: sr_elimination_info.join("_"),
        isPrimary_approach: is_primary_gateway,
        functional_gateways_before_scoring_flow: functional_gateways,
        experimentTag: experiment_tag,
        dateCreated: txn_creation_time,
    };

    let payment_source_m = txn_card_info
        .paymentSource
        .as_ref()
        .and_then(|ps| ps.split('@').next_back().map(String::from));

    metric_tracker_log(
        "GATEWAY_DECIDER_APPROACH",
        "DECIDER",
        MessageFormat {
            model: txn_detail
                .txnObjectType
                .map(|t| t.to_string())
                .unwrap_or_default(),
            log_type: "APP_EVENT".to_string(),
            payment_method: txn_card_info.clone().paymentMethod,
            payment_method_type: txn_card_info.clone().paymentMethodType.to_string(),
            payment_source: payment_source_m,
            source_object: txn_detail.sourceObject,
            txn_detail_id: txn_detail.id,
            stage: "GATEWAY_DECIDER_APPROACH".to_string(),
            merchant_id: merchant_id_to_text(order_reference.merchantId),
            txn_uuid: txn_detail.txnUuid,
            order_id: order_reference.orderId.0,
            card_type: txn_card_info
                .card_type
                .clone()
                .map(|ct| ct.to_string())
                .unwrap_or_default(),
            auth_type: txn_card_info.authType.clone().map(|at| at.to_string()),
            bank_code: fetch_juspay_bank_code(&txn_card_info),
            x_request_id: x_req_id,
            log_data: serde_json::to_value(mp).unwrap(),
        },
    )
    .await;
}

// ... existing code ...

pub fn round_off_to_3(db: f64) -> f64 {
    (db * 1000.0).round() / 1000.0
}

pub fn get_true_string(val: Option<String>) -> Option<String> {
    match val {
        Some(ref value) if value.is_empty() => None,
        _ => val,
    }
}

pub async fn get_card_bin_from_token_bin(length: usize, token_bin: &str) -> String {
    let key = format!("token_bin_{}", token_bin);
    let app_state = get_tenant_app_state().await;
    // let redis = &decider_flow.state().redis_conn;
    match app_state.redis_conn.get_key_string(&key).await.ok() {
        Some(bin) => bin.chars().take(length).collect(),
        None => match get_extended_token_bin_info(token_bin).await {
            Some(token_bin_info) => {
                app_state
                    .redis_conn
                    .set_key(&key, &token_bin_info.cardBin)
                    .await;
                token_bin_info.cardBin.chars().take(length).collect()
            }
            None => {
                crate::logger::info!(
                    "getCardBinFromTokenBin: tokenBin <> cardbin mapping not present {}",
                    token_bin
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

pub fn intercalate_without_empty_string(
    intercalate_with: &str,
    input_texts: &Vec<String>,
) -> String {
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

pub async fn get_token_supported_gateways(
    txn_detail: ETTD::TxnDetail,
    txn_card_info: ETCa::txn_card_info::TxnCardInfo,
    flow: String,
    m_internal_meta: Option<InternalMetadata>,
) -> Option<Vec<String>> {
    let token_type = get_stored_card_vault_provider(m_internal_meta.clone());
    let brand = txn_card_info
        .cardSwitchProvider
        .as_ref()
        .map_or("DEFAULT".to_string(), |secret| secret.peek().to_string());
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

async fn get_network_token_supported_gateways(
    flow: &String,
    network: String,
) -> Option<Vec<String>> {
    match flow.as_str() {
        "OTP" => RService::findByNameFromRedis(C::getTokenRepeatOtpGatewayKey(network).get_key())
            .await
            .unwrap_or_default(),
        "CVV_LESS" => {
            RService::findByNameFromRedis(C::getTokenRepeatCvvLessGatewayKey(network).get_key())
                .await
                .unwrap_or_default()
        }
        "MANDATE" => {
            RService::findByNameFromRedis(C::getTokenRepeatMandateGatewayKey(network).get_key())
                .await
                .unwrap_or_default()
        }
        "CARD" => RService::findByNameFromRedis(C::getTokenRepeatGatewayKey(network).get_key())
            .await
            .unwrap_or_default(),
        _ => Some(vec![]),
    }
}

async fn get_token_supported_gateways_key(
    brand: String,
    provider_category: String,
    token_provider: String,
    flow: String,
) -> Option<Vec<String>> {
    if brand == token_provider {
        RService::findByNameFromRedis(
            C::TokenSupportedGateways(brand, None, provider_category, flow).get_key(),
        )
        .await
        .unwrap_or_default()
    } else {
        RService::findByNameFromRedis(
            C::TokenSupportedGateways(brand, Some(token_provider), provider_category, flow)
                .get_key(),
        )
        .await
        .unwrap_or_default()
    }
}

fn get_stored_card_vault_provider(m_internal_meta: Option<InternalMetadata>) -> String {
    m_internal_meta
        .and_then(|meta| meta.storedCardVaultProvider)
        .unwrap_or_else(|| "DEFAULT".to_string())
}

fn get_token_provider(
    m_internal_meta: Option<InternalMetadata>,
    txn_card_info: &ETCa::txn_card_info::TxnCardInfo,
    card_switch_provider: &String,
) -> String {
    let juspay_bank_code = fetch_juspay_bank_code(txn_card_info);
    match m_internal_meta {
        Some(internal_meta_data) => match internal_meta_data.tokenProvider {
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

pub fn is_token_repeat_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta
        .and_then(|meta| meta.storedCardVaultProvider)
        .is_some_and(|provider| {
            ["NETWORK_TOKEN", "ISSUER_TOKEN", "ALT_ID"].contains(&provider.as_str())
        })
}

pub fn is_network_token_repeat_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta
        .and_then(|meta| meta.storedCardVaultProvider)
        .is_some_and(|provider| provider == "NETWORK_TOKEN")
}

pub fn is_issuer_token_repeat_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta
        .and_then(|meta| meta.storedCardVaultProvider)
        .is_some_and(|provider| provider == "ISSUER_TOKEN")
}

pub fn is_alt_id_based_txn(m_internal_meta: Option<InternalMetadata>) -> bool {
    m_internal_meta
        .and_then(|meta| meta.storedCardVaultProvider)
        .is_some_and(|provider| provider == "ALT_ID")
}

pub fn get_m_id(mid: ETM::id::MerchantId) -> String {
    mid.0.clone()
}

async fn get_upi_handle_list() -> Vec<String> {
    RService::findByNameFromRedis(C::V2RoutingHandleList.get_key())
        .await
        .unwrap_or_default()
}

async fn get_routing_top_bank_list() -> Vec<String> {
    RService::findByNameFromRedis(C::V2RoutingTopBankList.get_key())
        .await
        .unwrap_or_default()
}

async fn get_upi_package_list() -> Vec<String> {
    RService::findByNameFromRedis(C::V2RoutingPspPackageList.get_key())
        .await
        .unwrap_or_default()
}

pub fn get_bin_list(card_bin: Option<String>) -> Vec<Option<String>> {
    match get_true_string(card_bin) {
        None => vec![],
        Some(bin) => {
            if bin.len() > 6 {
                (6..=9)
                    .map(|len| Some(bin[..len.min(bin.len())].to_string()))
                    .collect()
            } else {
                vec![Some(bin)]
            }
        }
    }
}

pub async fn get_card_info_by_bin(card_bin: Option<String>) -> Option<ETCa::card_info::CardInfo> {
    logger::debug!("getCardInfoByBin cardBin: {:?}", card_bin);
    match get_true_string(card_bin) {
        None => None,
        Some(bin) => {
            let bin_list = if bin.len() > 6 {
                (6..=9)
                    .filter(|&len| len <= bin.len())
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

    str.parse::<i32>().unwrap_or(0)
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
            .filter(|flow| C::PAYMENT_FLOWS_REQUIRED_FOR_GW_FILTERING.contains(&flow.as_str()))
            .collect(),
        None => vec![],
    }
}

use crate::decider::gatewaydecider::types::PaymentFlowInfoInInternalTrackingInfo;

pub fn set_payment_flow_list(decider_flow: &mut DeciderFlow<'_>, payment_flow_list: Vec<String>) {
    decider_flow.writer.paymentFlowList = payment_flow_list;
}

pub fn check_if_enabled_in_mga(
    mga: &ETM::merchant_gateway_account::MerchantGatewayAccount,
    payment_flow: &str,
    acc_details_flag_to_be_checked: &str,
) -> bool {
    is_payment_flow_enabled_in_mga(mga, payment_flow).unwrap_or_else(|| {
        get_value(acc_details_flag_to_be_checked, mga.account_details.peek()).unwrap_or(false)
    })
}

pub fn check_if_no_ds_enabled_in_mga(
    mga: &ETM::merchant_gateway_account::MerchantGatewayAccount,
    payment_flow: &str,
    acc_details_flag_to_be_checked: &str,
) -> bool {
    is_payment_flow_enabled_in_mga(mga, payment_flow).unwrap_or_else(|| {
        get_value(acc_details_flag_to_be_checked, mga.account_details.peek()).unwrap_or(true)
    })
}

pub fn is_payment_flow_enabled_in_mga(
    mga: &ETM::merchant_gateway_account::MerchantGatewayAccount,
    payment_flow: &str,
) -> Option<bool> {
    mga.supported_payment_flows
        .as_ref()
        .map(|flows| flows.payment_flow_ids.contains(&payment_flow.to_string()))
}

pub fn get_max_score_gateway(gsm: &types::GatewayScoreMap) -> Option<(String, f64)> {
    gsm.iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(Ordering::Equal))
        .map(|(gw, score)| (gw.clone(), *score))
}

pub fn random_gateway_selection_for_same_score(
    st: &types::GatewayScoreMap,
    max_score: Option<f64>,
) -> Option<String> {
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
                // #TODO: Implement shuffle
                Some(gws[0].clone())
                // todo!()
                // EList::shuffle(gws).map(|shuffled_gws| shuffled_gws.into_iter().next())
            }
        }
        None => None,
    }
}

pub fn get_gateway_decider_approach(
    get_gwsm: &types::GatewayScoreMap,
    gateway_decider_approach: types::GatewayDeciderApproach,
) -> types::GatewayDeciderApproach {
    let gw_set = get_gwsm.keys().cloned().collect::<HashSet<_>>();
    if !gw_set.is_empty() {
        if gw_set.len() > 1 {
            gateway_decider_approach
        } else {
            types::GatewayDeciderApproach::Default
        }
    } else {
        types::GatewayDeciderApproach::None
    }
}

pub fn modify_gateway_decider_approach(
    gw_decider_approach: types::GatewayDeciderApproach,
    down_time: types::DownTime,
) -> types::GatewayDeciderApproach {
    match gw_decider_approach {
        types::GatewayDeciderApproach::SrSelectionV3Routing => match down_time {
            types::DownTime::AllDowntime => types::GatewayDeciderApproach::SrV3AllDowntimeRouting,
            types::DownTime::GlobalDowntime => {
                types::GatewayDeciderApproach::SrV3GlobalDowntimeRouting
            }
            types::DownTime::Downtime => types::GatewayDeciderApproach::SrV3DowntimeRouting,
            types::DownTime::NoDowntime => types::GatewayDeciderApproach::SrSelectionV3Routing,
        },
        types::GatewayDeciderApproach::SrV3Hedging => match down_time {
            types::DownTime::AllDowntime => types::GatewayDeciderApproach::SrV3AllDowntimeHedging,
            types::DownTime::GlobalDowntime => {
                types::GatewayDeciderApproach::SrV3GlobalDowntimeHedging
            }
            types::DownTime::Downtime => types::GatewayDeciderApproach::SrV3DowntimeHedging,
            types::DownTime::NoDowntime => types::GatewayDeciderApproach::SrV3Hedging,
        },
        types::GatewayDeciderApproach::SrSelectionV2Routing => match down_time {
            types::DownTime::AllDowntime => types::GatewayDeciderApproach::SrV2AllDowntimeRouting,
            types::DownTime::GlobalDowntime => {
                types::GatewayDeciderApproach::SrV2GlobalDowntimeRouting
            }
            types::DownTime::Downtime => types::GatewayDeciderApproach::SrV2DowntimeRouting,
            types::DownTime::NoDowntime => types::GatewayDeciderApproach::SrSelectionV2Routing,
        },
        types::GatewayDeciderApproach::SrV2Hedging => match down_time {
            types::DownTime::AllDowntime => types::GatewayDeciderApproach::SrV2AllDowntimeHedging,
            types::DownTime::GlobalDowntime => {
                types::GatewayDeciderApproach::SrV2GlobalDowntimeHedging
            }
            types::DownTime::Downtime => types::GatewayDeciderApproach::SrV2DowntimeHedging,
            types::DownTime::NoDowntime => types::GatewayDeciderApproach::SrV2Hedging,
        },
        _ => match down_time {
            types::DownTime::AllDowntime => types::GatewayDeciderApproach::PlAllDowntimeRouting,
            types::DownTime::GlobalDowntime => {
                types::GatewayDeciderApproach::PlGlobalDowntimeRouting
            }
            types::DownTime::Downtime => types::GatewayDeciderApproach::PlDowntimeRouting,
            types::DownTime::NoDowntime => types::GatewayDeciderApproach::PriorityLogic,
        },
    }
}

pub fn get_juspay_bank_code_from_internal_metadata(txn_detail: &ETTD::TxnDetail) -> Option<String> {
    txn_detail.internalMetadata.as_ref().and_then(|metadata| {
        from_str::<Value>(metadata.peek()).ok().and_then(|json| {
            json.get("juspayBankCode")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
        })
    })
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
        "filterFunctionalGatewaysForReversePennyDrop" => 3,
        "filterFunctionalGateways" => 4,
        "filterFunctionalGatewaysForBrand" => 5,
        "filterFunctionalGatewaysForAuthType" => 6,
        "filterFunctionalGatewaysForValidationType" => 7,
        "filterFunctionalGatewaysForEmi" => 8,
        "filterFunctionalGatewaysForTxnOfferDetails" => 9,
        "filterFunctionalGatewaysForPaymentMethod" => 10,
        "filterFunctionalGatewaysForTokenProvider" => 11,
        "filterFunctionalGatewaysForWallet" => 12,
        "filterFunctionalGatewaysForNbOnly" => 13,
        "filterFunctionalGatewaysForConsumerFinance" => 14,
        "filterFunctionalGatewaysForUpi" => 15,
        "filterFunctionalGatewaysForTxnType" => 16,
        "filterFunctionalGatewaysForTxnDetailType" => 17,
        "filterFunctionalGatewaysForReward" => 18,
        "filterFunctionalGatewaysForCash" => 19,
        "filterFunctionalGatewaysForSplitSettlement" => 20,
        "preferredGateway" => 21,
        "filterEnforcement" => 22,
        "filterFunctionalGatewaysForMerchantRequiredFlow" => 23,
        "filterGatewaysForEMITenureSpecficGatewayCreds" => 24,
        "filterGatewaysForMGASelectionIntegrity" => 25,
        "FilterFunctionalGatewaysForOTM" => 26,
        _ => 27,
    }
}

// pub const SR_STALE_SCORE_LOG: &str = "SR stale score";

// pub async fn log_sr_stale(
//     orbd: OptimizationRedisBlockData,
//     merchant_id: String,
//     key: String,
//     gateway_scores: GatewayScoreMap,
// ) {
//     if let Some(gateway_score_detail) = orbd.aggregate.last() {
//         let block_time_period = get_block_time_period(&merchant_id).await;
//         let current_time_stamp_in_millis = get_current_date_in_millis();
//         if (current_time_stamp_in_millis - gateway_score_detail.timestamp as u128)
//             > (4 * block_time_period as u128)
//         {
//             // log().await;
//         }
//     }
// }

// async fn log() {
//     todo!()
//     // L::log_info_v(SR_STALE_SCORE_LOG, SRStaleScoreLog {
//     //     score_key: key,
//     //     merchant_id: merchant_id,
//     //     gateway_scores: gateway_scores.into_iter().collect(),
//     // }).await;
// }

// pub async fn get_block_time_period(merchant_id: &str) -> i64 {
//     match RService::findByNameFromRedis::<ConfigurableBlock>(
//         C::OptimizationRoutingConfig(merchant_id.to_string()).get_key(),
//     )
//     .await
//     {
//         Some(config_block) => config_block.block_timeperiod.round() as i64,
//         None => match RService::findByNameFromRedis::<ConfigurableBlock>(
//             C::DefaultOptimizationRoutingConfig.get_key(),
//         )
//         .await
//         {
//             Some(config_block) => config_block.block_timeperiod.round() as i64,
//             None => 1800000,
//         },
//     }
// }

pub fn decode_and_log_error(
    error_tag: &str,
    a: &String,
) -> Option<GatewaySuccessRateBasedRoutingInput> {
    match serde_json::from_str(a) {
        Ok(value) => Some(value),
        Err(e) => {
            logger::error!("Error decoding JSON: {}. Error: {}", error_tag, e);
            None
        }
    }
}

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
    let parse_format = parse(format_text).map_err(|_| "Invalid input format".to_string())?;

    // Parse the date
    let parsed_date =
        Date::parse(date_text, &parse_format).map_err(|_| "Invalid date".to_string())?;

    // Format the date in dd-mm-yyyy format
    parsed_date
        .format(
            &parse("[day]-[month]-[year]")
                .map_err(|_| "Failed to create output format".to_string())?,
        )
        .map_err(|_| "Failed to format date".to_string())
}

pub async fn get_experiment_tag(utc_time: OffsetDateTime, dim: &str) -> Option<String> {
    match get_date_in_format(&utc_time.to_string(), "%Y-%m-%d %H:%M:%S %Z") {
        Ok(date) => Some(format!("EXPERIMENT_{}_{}", dim, date)),
        Err(e) => {
            logger::error!("Error in getExperimentTag: {}", e);
            None
        }
    }
}

pub async fn create_moving_window_and_score(
    redis: String,
    queue_key: String, // Take owned strings
    score_key: String,
    score: i32,
    score_list: Vec<String>,
) {
    // todo!()
    let app_state = get_tenant_app_state().await;
    let r: Result<Vec<String>, error_stack::Report<redis_interface::errors::RedisError>> =
        app_state
            .redis_conn
            .multi(false, |transaction| {
                Box::pin(async move {
                    transaction.del::<(), _>(queue_key.clone()).await?;
                    transaction
                        .lpush::<(), _, _>(
                            queue_key.as_bytes().clone(),
                            score_list.iter().map(|s| s.as_bytes()).collect::<Vec<_>>(),
                        )
                        .await?;
                    transaction
                        .set::<(), _, _>(
                            score_key.clone(),
                            score.to_string().clone(),
                            None,
                            None,
                            false,
                        )
                        .await?;
                    transaction
                        .expire::<(), _>(queue_key.as_bytes().clone(), 10000000)
                        .await?;
                    transaction
                        .expire::<(), _>(score_key.as_bytes().clone(), 10000000)
                        .await?;
                    Ok(())
                })
            })
            .await;
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

pub fn get_sr_v3_latency_threshold(
    sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: &str,
    pm: &str,
    sr_routing_dimesions: &SrRoutingDimensions,
) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(
            &config.subLevelInputConfig,
            pmt,
            pm,
            sr_routing_dimesions,
            |x| x.latencyThreshold.is_some(),
        )
        .and_then(|sub_config| sub_config.latencyThreshold)
        .or(config.defaultLatencyThreshold)
    })
}

pub fn get_sr_v3_bucket_size(
    sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: &str,
    pm: &str,
    sr_routing_dimesions: &SrRoutingDimensions,
) -> Option<i32> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(
            &config.subLevelInputConfig,
            pmt,
            pm,
            sr_routing_dimesions,
            |x| x.bucketSize.is_some(),
        )
        .and_then(|sub_config| sub_config.bucketSize)
        .or(config.defaultBucketSize)
        .filter(|&size| size > 0)
    })
}

pub fn get_sr_v3_hedging_percent(
    sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: &str,
    pm: &str,
    sr_routing_dimesions: &SrRoutingDimensions,
) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(
            &config.subLevelInputConfig,
            pmt,
            pm,
            sr_routing_dimesions,
            |x| x.hedgingPercent.is_some(),
        )
        .and_then(|sub_config| sub_config.hedgingPercent)
        .or(config.defaultHedgingPercent)
        .filter(|&percent| percent >= 0.0)
    })
}

pub fn get_sr_v3_lower_reset_factor(
    sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: &str,
    pm: &str,
    sr_routing_dimesions: &SrRoutingDimensions,
) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(
            &config.subLevelInputConfig,
            pmt,
            pm,
            sr_routing_dimesions,
            |x| x.lowerResetFactor.is_some(),
        )
        .and_then(|sub_config| sub_config.lowerResetFactor)
        .or(config.defaultLowerResetFactor)
        .filter(|&factor| factor >= 0.0)
    })
}

pub fn get_sr_v3_upper_reset_factor(
    sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: &str,
    pm: &str,
    sr_routing_dimesions: &SrRoutingDimensions,
) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(
            &config.subLevelInputConfig,
            pmt,
            pm,
            sr_routing_dimesions,
            |x| x.upperResetFactor.is_some(),
        )
        .and_then(|sub_config| sub_config.upperResetFactor)
        .or(config.defaultUpperResetFactor)
        .filter(|&factor| factor >= 0.0)
    })
}

pub fn get_sr_v3_gateway_sigma_factor(
    sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: &str,
    pm: &str,
    gw: &String,
    sr_routing_dimesions: &SrRoutingDimensions,
) -> Option<f64> {
    sr_v3_input_config.and_then(|config| {
        get_sr_v3_sub_level_input_config(
            &config.subLevelInputConfig,
            pmt,
            pm,
            sr_routing_dimesions,
            |x| {
                x.gatewayExtraScore
                    .as_ref()
                    .is_some_and(|scores| scores.iter().any(|score| score.gatewayName == *gw))
            },
        )
        .and_then(|sub_config| find_gateway_sigma_factor(&sub_config.gatewayExtraScore, gw))
        .or_else(|| find_gateway_sigma_factor(&config.defaultGatewayExtraScore, gw))
    })
}

fn find_gateway_sigma_factor(
    gateway_extra_score: &Option<Vec<GatewayWiseExtraScore>>,
    gw: &String,
) -> Option<f64> {
    gateway_extra_score.as_ref().and_then(|scores| {
        scores
            .iter()
            .find(|score| score.gatewayName == *gw)
            .map(|score| score.gatewaySigmaFactor)
    })
}

fn get_sr_v3_sub_level_input_config(
    sub_level_input_config: &Option<Vec<SrV3SubLevelInputConfig>>,
    pmt: &str,
    pm: &str,
    sr_routing_dimesions: &SrRoutingDimensions,
    is_input_non_null: impl Fn(&SrV3SubLevelInputConfig) -> bool,
) -> Option<SrV3SubLevelInputConfig> {
    sub_level_input_config
        .as_ref()
        .and_then(|configs| {
            configs
                .iter()
                .find(|config| {
                    is_sr_v3_config_match(
                        config,
                        Some(pmt.to_string()),
                        Some(pm.to_string()),
                        &sr_routing_dimesions,
                    ) && is_input_non_null(config)
                })
                .or_else(|| {
                    configs.iter().find(|config| {
                        is_sr_v3_config_match(
                            config,
                            Some(pmt.to_string()),
                            None,
                            &sr_routing_dimesions,
                        ) && is_input_non_null(config)
                    })
                })
        })
        .cloned()
}

fn is_sr_v3_config_match(
    config: &SrV3SubLevelInputConfig,
    pmt: Option<String>,
    pm: Option<String>,
    sr_routing_dimesions: &SrRoutingDimensions,
) -> bool {
    let pmt_matches = config.paymentMethodType == pmt;
    let pm_matches = config.paymentMethod.is_none() || config.paymentMethod == pm;
    let card_network_matches =
        config.cardNetwork.is_none() || config.cardNetwork == sr_routing_dimesions.card_network;
    let card_isin_matches =
        config.cardIsIn.is_none() || config.cardIsIn == sr_routing_dimesions.card_isin;
    let currency_matches =
        config.currency.is_none() || config.currency == sr_routing_dimesions.currency;
    let country_matches =
        config.country.is_none() || config.country == sr_routing_dimesions.country;
    let auth_type_matches =
        config.authType.is_none() || config.authType == sr_routing_dimesions.auth_type;

    pmt_matches
        && pm_matches
        && card_network_matches
        && card_isin_matches
        && currency_matches
        && auth_type_matches
        && country_matches
}

pub fn filter_upto_pmt(
    sub_level_input_config: Vec<SrV3SubLevelInputConfig>,
    pmt: String,
    is_input_non_null: impl Fn(&SrV3SubLevelInputConfig) -> bool,
) -> Option<SrV3SubLevelInputConfig> {
    sub_level_input_config.into_iter().find(|x| {
        (x.paymentMethodType.as_ref() == Some(&pmt))
            && x.paymentMethod.is_none()
            && is_input_non_null(x)
    })
}

pub fn get_payment_method(
    payment_method_type: String,
    pm: String,
    source_object: String,
) -> String {
    if payment_method_type == UPI && pm == "UPI" {
        source_object
    } else {
        pm
    }
}

pub async fn delete_score_key_if_bucket_size_changes(
    decider_flow: &mut DeciderFlow<'_>,
    merchant_bucket_size: i32,
    sr_gateway_redis_key_map: GatewayRedisKeyMap,
) {
    for gateway_redis_key in (sr_gateway_redis_key_map).into_iter() {
        // Check if the bucket size has changed
        match check_if_bucket_size_changed(
            decider_flow,
            merchant_bucket_size,
            gateway_redis_key.clone(),
        )
        .await
        {
            true => {
                // If bucket size changed, delete the score key
                let (_, sr_redis_key) = gateway_redis_key.clone();
                match get_tenant_app_state()
                    .await
                    .redis_conn
                    .conn
                    .delete_key(&[sr_redis_key, "}score".to_string()].concat())
                    .await
                {
                    Ok(res) => (),
                    Err(err) => {
                        logger::error!(
                            action = "deleteScoreKeyIfBucketSizeChanges",
                            tag = "deleteScoreKeyIfBucketSizeChanges",
                            "Error while deleting score key in redis: {}",
                            err
                        );
                    }
                }
            }
            _ => (), // Skip if bucket size hasn't changed or there's an error
        }
    }
}

pub fn intercalate<S: AsRef<str>>(separator: &str, strings: &[S]) -> String {
    strings
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<&str>>()
        .join(separator)
}

// Function to check if the bucket size has changed for a specific gateway
pub async fn check_if_bucket_size_changed(
    decider_flow: &mut DeciderFlow<'_>,
    merchant_bucket_size: i32,
    gateway_redis_key: (String, String),
) -> bool {
    let (_, sr_redis_key) = gateway_redis_key;

    // Construct the queue key
    let queue_key = intercalate("", &[sr_redis_key, "}queue".to_string()]);

    // Check the length of the queue in Redis
    match get_tenant_app_state()
        .await
        .redis_conn
        .conn
        .get_list_length(queue_key.as_str())
        .await
    {
        Ok(redis_bucket_size) => redis_bucket_size != merchant_bucket_size as usize,
        Err(err) => {
            logger::error!(
                action = "checkIfBucketSizeChanged",
                tag = "checkIfBucketSizeChanged",
                "Error while getting queue size in redis - returning True: {}",
                err
            );
            true
        }
    }
}

// pub async fn add_txn_to_hash_map_if_debug_mode(is_debug_mode_enabled: bool, mid: Text, txn_detail: ETTD::TxnDetail) -> DeciderFlow<()> {
//     if is_debug_mode_enabled {
//         let either_pending_txn_key_size = RC::hlen(Config::kv_redis(), &TE::encode_utf8(&format!("{}{}", C::pending_txns_key_prefix(), mid))).await;
//         let pending_txn_key_size = match either_pending_txn_key_size {
//         Ok(size) => size,
//         Err(err) => {
//             L::log_error_v("addTxnToHashMapIfDebugMode", "Error while getting hash map size in redis - returning max size", err).await;
//             10000
//             }
//         };
//         if pending_txn_key_size < 10000 {
//              RC::r_hset_b(Config::kv_redis(), &TE::encode_utf8(&format!("{}{}", C::pending_txns_key_prefix(), mid)), &TE::encode_utf8(&txn_detail.txn_uuid), "1").await;
//             } else {
//              log_info_t("addTxnToHashMapIfDebugMode", &format!("Size limit reached for storing pending txns in SRV3 debug mode, key: {}{}", C::pending_txns_key_prefix(), mid)).await;
//             }
//         } else {
//              RC::r_del(Config::kv_redis(), &[C::pending_txns_key_prefix(), mid].concat()).await;
//     }
// }

pub async fn check_if_bin_is_eligible_for_emi(
    card_isin: Option<String>,
    juspay_bank_code: Option<String>,
    card_type: Option<String>,
) -> bool {
    if let (Some(card_isin), Some(juspay_bank_code), Some(card_type)) =
        (card_isin, juspay_bank_code, card_type)
    {
        let bin_check_mandated_banks: Option<Vec<String>> =
            RService::findByNameFromRedis(C::GET_EMI_BIN_VALIDATION_SUPPORTED_BANKS_KEY.get_key())
                .await;
        let should_do_bin_validation = bin_check_mandated_banks
            .is_some_and(|banks| banks.contains(&format!("{}::{}", juspay_bank_code, card_type)));
        if should_do_bin_validation {
            let bin_list: Vec<String> = get_bin_list(Some(card_isin))
                .into_iter()
                .flatten()
                .collect();

            let emi_eligible_bins = get_eligibility_info(
                bin_list,
                identifier_name_to_text(IdentifierName::BIN),
                juspay_bank_code,
                payment_flows_to_text(&PaymentFlow::PgEmi),
            )
            .await;
            !emi_eligible_bins.is_empty()
        } else {
            true
        }
    } else {
        true
    }
}

pub fn is_reverse_penny_drop_txn(txn_detail: &ETTD::TxnDetail) -> bool {
    get_payment_flow_list_from_txn_detail(txn_detail).contains(&"REVERSE_PENNY_DROP".to_string())
}

pub fn check_for_reverse_penny_drop_in_mga(mga: &MerchantGatewayAccount) -> bool {
    match mga.supported_payment_flows.as_ref() {
        None => false,
        Some(pf) => pf
            .payment_flow_ids
            .contains(&"REVERSE_PENNY_DROP".to_string()),
    }
}

pub fn get_default_gateway_scoring_data(
    merchant_id: String,
    order_type: String,
    payment_method_type: String,
    payment_method: String,
    is_gri_enabled_for_elimination: bool,
    is_gri_enabled_for_sr_routing: bool,
    date_created: OffsetDateTime,
    card_isin: Option<String>,
    card_switch_provider: Option<Secret<String>>,
    currency: Option<Currency>,
    country: Option<CountryISO2>,
    auth_type: Option<String>,
) -> GatewayScoringData {
    GatewayScoringData {
        merchantId: merchant_id,
        paymentMethodType: payment_method_type,
        paymentMethod: payment_method,
        orderType: order_type,
        cardType: None,
        bankCode: None,
        authType: auth_type,
        paymentSource: None,
        isPaymentSourceEnabledForSrRouting: false,
        isAuthLevelEnabledForSrRouting: false,
        isBankLevelEnabledForSrRouting: false,
        isGriEnabledForElimination: is_gri_enabled_for_elimination,
        isGriEnabledForSrRouting: is_gri_enabled_for_sr_routing,
        routingApproach: None,
        dateCreated: date_created,
        eliminationEnabled: false,
        cardIsIn: card_isin,
        cardSwitchProvider: card_switch_provider,
        currency: currency,
        country: country,
        is_legacy_decider_flow: false,
    }
}

pub async fn get_gateway_scoring_data(
    decider_flow: &mut DeciderFlow<'_>,
    txn_detail: ETTD::TxnDetail,
    txn_card_info: ETCa::txn_card_info::TxnCardInfo,
    merchant: ETM::merchant_account::MerchantAccount,
) -> GatewayScoringData {
    let merchant_enabled_for_unification = isFeatureEnabled(
        C::MerchantsEnabledForScoreKeysUnification.get_key(),
        merchant_id_to_text(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let merchant_id = merchant_id_to_text(merchant.merchantId.clone());
    let order_type = txn_detail
        .txnObjectType
        .map(|t| t.to_string())
        .unwrap_or_default();
    let payment_method_type = txn_card_info.paymentMethodType.to_uppercase();
    let m_source_object = if txn_card_info.paymentMethod == UPI {
        txn_detail.sourceObject.clone().unwrap_or_default()
    } else {
        txn_card_info.paymentMethod.clone()
    };
    let is_performing_experiment = isFeatureEnabled(
        C::MerchantEnabledForRoutingExperiment.get_key(),
        merchant_id_to_text(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let is_gri_enabled_for_elimination = isFeatureEnabled(
        C::GatewayReferenceIdEnabledMerchant.get_key(),
        merchant_id_to_text(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let is_gri_enabled_for_sr_routing = isFeatureEnabled(
        C::GwRefIdSelectionBasedEnabledMerchant.get_key(),
        merchant_id_to_text(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let mut default_gateway_scoring_data = get_default_gateway_scoring_data(
        merchant_id.clone(),
        order_type,
        payment_method_type,
        m_source_object,
        is_gri_enabled_for_elimination,
        is_gri_enabled_for_sr_routing,
        decider_flow.get().dpTxnDetail.dateCreated.clone(),
        decider_flow.get().dpTxnCardInfo.card_isin.clone(),
        decider_flow.get().dpTxnCardInfo.cardSwitchProvider.clone(),
        Some(decider_flow.get().dpOrder.currency.clone()),
        decider_flow.get().dpTxnDetail.country.clone(),
        decider_flow
            .get()
            .dpTxnCardInfo
            .authType
            .as_ref()
            .map(|a| a.to_string()),
    );
    let updated_gateway_scoring_data = match txn_card_info.paymentMethodType.as_str() {
        UPI => {
            let handle_and_package_based_routing = isFeatureEnabled(
                C::HandlePackageBasedRoutingCutover.get_key(),
                merchant_id.clone(),
                "kv_redis".to_string(),
            )
            .await;
            if is_performing_experiment && handle_and_package_based_routing {
                let experiment_tag = get_experiment_tag(txn_detail.dateCreated, "HANDLE_PSP").await;
                set_is_experiment_tag(decider_flow, experiment_tag);
            }

            let payment_source = get_true_string(txn_card_info.paymentSource.clone())
                .map(|source| source.split("@").last().unwrap_or_default().to_uppercase())
                .unwrap_or_default();
            default_gateway_scoring_data.paymentSource = Some(payment_source);
            default_gateway_scoring_data.isPaymentSourceEnabledForSrRouting =
                handle_and_package_based_routing;
            default_gateway_scoring_data
        }
        CARD => {
            let sr_evaluation_at_auth_level = isFeatureEnabled(
                C::EnableSelectionBasedAuthTypeEvaluation.get_key(),
                merchant_id.clone(),
                "kv_redis".to_string(),
            )
            .await;
            let sr_evaluation_at_bank_level = isFeatureEnabled(
                C::EnableSelectionBasedBankLevelEvaluation.get_key(),
                merchant_id.clone(),
                "kv_redis".to_string(),
            )
            .await;
            if is_performing_experiment {
                if sr_evaluation_at_auth_level {
                    let experiment_tag =
                        get_experiment_tag(txn_detail.dateCreated, "AUTH_TYPE").await;
                    set_is_experiment_tag(decider_flow, experiment_tag);
                } else if sr_evaluation_at_bank_level {
                    let experiment_tag =
                        get_experiment_tag(txn_detail.dateCreated, "BANK_TYPE").await;
                    set_is_experiment_tag(decider_flow, experiment_tag);
                }
            }
            let card_type = txn_card_info
                .card_type
                .clone()
                .map(|card| card_type_to_text(&card).to_uppercase())
                .unwrap_or_default();
            let auth_type = txn_card_info
                .authType
                .clone()
                .map(|auth| auth_type_to_text(&auth).to_uppercase())
                .unwrap_or_default();
            let card_auth_type = match auth_type.as_str() {
                "THREE_DS" | "THREE_DS_2" => "THREE_DS",
                "OTP" => "OTP",
                _ => "UNKNOWN",
            };
            let bank_code = fetch_juspay_bank_code(&txn_card_info).unwrap_or("UNKNOWN".to_string());
            default_gateway_scoring_data.authType = Some(card_auth_type.to_owned());
            default_gateway_scoring_data.cardType = Some(card_type.to_owned());
            default_gateway_scoring_data.bankCode = Some(bank_code.to_owned());
            default_gateway_scoring_data.isBankLevelEnabledForSrRouting =
                sr_evaluation_at_bank_level;
            default_gateway_scoring_data.isAuthLevelEnabledForSrRouting =
                sr_evaluation_at_auth_level;
            default_gateway_scoring_data
        }
        _ => default_gateway_scoring_data,
    };
    set_routing_dimension_and_reference(decider_flow, updated_gateway_scoring_data.clone()).await;
    set_elimination_dimension(decider_flow, updated_gateway_scoring_data.clone());
    set_outage_dimension(decider_flow, updated_gateway_scoring_data.clone());

    if is_performing_experiment && is_gri_enabled_for_elimination {
        let experiment_tag =
            get_experiment_tag(txn_detail.dateCreated, "GRI_BASED_ELIMINATION").await;
        set_is_experiment_tag(decider_flow, experiment_tag);
    }
    if is_performing_experiment && is_gri_enabled_for_sr_routing {
        let experiment_tag =
            get_experiment_tag(txn_detail.dateCreated, "GRI_BASED_SR_ROUTING").await;
        set_is_experiment_tag(decider_flow, experiment_tag);
    }
    let key = [C::GATEWAY_SCORING_DATA, &txn_detail.txnUuid.clone()].concat();
    updated_gateway_scoring_data
}

pub async fn get_unified_key(
    gateway_scoring_data: GatewayScoringData,
    score_key_type: ScoreKeyType,
    enforce1d: bool,
    gateway_ref_id_map: types::GatewayReferenceIdMap,
) -> GatewayRedisKeyMap {
    let merchant_id = gateway_scoring_data.merchantId.clone();
    let order_type = gateway_scoring_data.orderType.clone();
    let payment_method_type = gateway_scoring_data.paymentMethodType.clone();
    let payment_method = gateway_scoring_data.paymentMethod.clone();

    let gateway_redis_key_map = match score_key_type {
        ScoreKeyType::EliminationGlobalKey => {
            let key_prefix = C::ELIMINATION_BASED_ROUTING_GLOBAL_KEY_PREFIX;
            let (prefix_key, suffix_key) = if payment_method_type == CARD {
                (
                    vec![key_prefix, &order_type.as_str()],
                    vec![
                        payment_method_type,
                        payment_method,
                        gateway_scoring_data
                            .cardType
                            .clone()
                            .as_deref()
                            .unwrap_or("")
                            .to_string(),
                    ],
                )
            } else {
                (
                    vec![key_prefix, &order_type.as_str()],
                    vec![payment_method_type, payment_method],
                )
            };

            let result_keys =
                gateway_ref_id_map
                    .iter()
                    .fold(GatewayRedisKeyMap::new(), |mut acc, (gw, _)| {
                        let final_key = intercalate_without_empty_string(
                            "_",
                            &[
                                prefix_key
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect::<Vec<String>>(),
                                vec![gw.to_string()],
                                suffix_key
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect::<Vec<String>>(),
                            ]
                            .concat(),
                        );
                        acc.insert(gw.clone(), final_key);
                        acc
                    });
            result_keys
        }
        ScoreKeyType::EliminationMerchantKey => {
            let isgri_enabled = gateway_scoring_data.isGriEnabledForElimination;
            let key_prefix = C::ELIMINATION_BASED_ROUTING_KEY_PREFIX;
            let (prefix_key, suffix_key) = if payment_method_type == CARD {
                (
                    vec![key_prefix, &merchant_id, &order_type.as_str()],
                    vec![
                        payment_method_type,
                        payment_method,
                        gateway_scoring_data
                            .cardType
                            .clone()
                            .as_deref()
                            .unwrap_or("")
                            .to_string(),
                    ],
                )
            } else {
                (
                    vec![key_prefix, &merchant_id, &order_type.as_str()],
                    vec![payment_method_type, payment_method],
                )
            };

            let result_keys = gateway_ref_id_map.iter().fold(
                GatewayRedisKeyMap::new(),
                |mut acc, (gw, ref_id)| {
                    let final_key = if isgri_enabled {
                        [
                            prefix_key
                                .iter()
                                .map(|s| s.to_string())
                                .collect::<Vec<String>>(),
                            vec![gw.to_string()],
                            suffix_key
                                .iter()
                                .map(|s| s.to_string())
                                .collect::<Vec<String>>(),
                            vec![ref_id.as_deref().unwrap_or("").to_string()],
                        ]
                        .concat()
                    } else {
                        [
                            prefix_key
                                .iter()
                                .map(|s| s.to_string())
                                .collect::<Vec<String>>(),
                            vec![gw.to_string()],
                            suffix_key
                                .iter()
                                .map(|s| s.to_string())
                                .collect::<Vec<String>>(),
                        ]
                        .concat()
                    };
                    acc.insert(
                        gw.clone(),
                        intercalate_without_empty_string("_", &final_key),
                    );
                    acc
                },
            );
            result_keys
        }
        ScoreKeyType::SrV2Key => {
            let key = get_unified_sr_key(&gateway_scoring_data, false, enforce1d).await;
            let gri_sr_v2_cutover = gateway_scoring_data.isGriEnabledForSrRouting;

            if gri_sr_v2_cutover {
                gateway_ref_id_map.iter().fold(
                    GatewayRedisKeyMap::new(),
                    |mut acc, (gateway, ref_id)| {
                        acc.insert(
                            gateway.clone(),
                            intercalate_without_empty_string(
                                "_",
                                &vec![key.clone(), ref_id.as_deref().unwrap_or("").to_string()],
                            ),
                        );
                        acc
                    },
                )
            } else {
                let mut map = GatewayRedisKeyMap::new();
                map.insert("".to_string(), key);
                map
            }
        }
        ScoreKeyType::SrV3Key => {
            let base_key = get_unified_sr_key(&gateway_scoring_data, true, enforce1d).await;
            let gri_sr_v2_cutover = gateway_scoring_data.isGriEnabledForSrRouting;

            if gri_sr_v2_cutover {
                gateway_ref_id_map.iter().fold(
                    GatewayRedisKeyMap::new(),
                    |mut acc, (gateway, ref_id)| {
                        let key = intercalate_without_empty_string(
                            "_",
                            &vec![
                                base_key.clone(),
                                ref_id.as_deref().unwrap_or("").to_string(),
                                gateway.to_string(),
                            ],
                        );
                        acc.insert(gateway.clone(), key);
                        acc
                    },
                )
            } else {
                gateway_ref_id_map.iter().fold(
                    GatewayRedisKeyMap::new(),
                    |mut acc, (gateway, _)| {
                        acc.insert(
                            gateway.clone(),
                            intercalate_without_empty_string(
                                "_",
                                &vec![base_key.clone(), gateway.to_string()],
                            ),
                        );
                        acc
                    },
                )
            }
        }
        ScoreKeyType::OutageGlobalKey => {
            let key_prefix = C::GLOBAL_LEVEL_OUTAGE_KEY_PREFIX;
            let base_key = if payment_method_type == CARD {
                vec![
                    key_prefix,
                    &payment_method_type,
                    &payment_method,
                    gateway_scoring_data.bankCode.as_deref().unwrap_or(""),
                    gateway_scoring_data.cardType.as_deref().unwrap_or(""),
                ]
            } else if payment_method_type == UPI {
                vec![
                    key_prefix,
                    &payment_method_type,
                    &payment_method,
                    gateway_scoring_data.paymentSource.as_deref().unwrap_or(""),
                ]
            } else {
                vec![key_prefix, &payment_method_type, &payment_method]
            };

            let mut map = GatewayRedisKeyMap::new();
            map.insert(
                "".to_string(),
                intercalate_without_empty_string(
                    "_",
                    &base_key
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                ),
            );
            map
        }
        ScoreKeyType::OutageMerchantKey => {
            let key_prefix = C::MERCHANT_LEVEL_OUTAGE_KEY_PREFIX;
            let base_key = if payment_method_type == CARD {
                vec![
                    key_prefix,
                    &merchant_id,
                    &payment_method_type,
                    &payment_method,
                    gateway_scoring_data.bankCode.as_deref().unwrap_or(""),
                    gateway_scoring_data.cardType.as_deref().unwrap_or(""),
                ]
            } else if payment_method_type == UPI {
                vec![
                    key_prefix,
                    &merchant_id,
                    &payment_method_type,
                    &payment_method,
                    gateway_scoring_data.paymentSource.as_deref().unwrap_or(""),
                ]
            } else {
                vec![
                    key_prefix,
                    &merchant_id,
                    &payment_method_type,
                    &payment_method,
                ]
            };

            let mut map = GatewayRedisKeyMap::new();
            map.insert(
                "".to_string(),
                intercalate_without_empty_string(
                    "_",
                    &base_key
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                ),
            );
            map
        }
    };

    let gateway_key_log = gateway_redis_key_map
        .iter()
        .map(|(gw, key)| format!("{} :{}", gw, key))
        .collect::<Vec<_>>()
        .join(" ");
    crate::logger::info!("GatewayRedisKeyMap: {}", gateway_key_log);

    gateway_redis_key_map
}

pub async fn get_unified_sr_key(
    gateway_scoring_data: &GatewayScoringData,
    is_sr_v3_metric_enabled: bool,
    enforce1d: bool,
) -> String {
    let is_legacy_decider_flow = gateway_scoring_data.is_legacy_decider_flow;
    if is_legacy_decider_flow {
        return get_legacy_unified_sr_key(gateway_scoring_data, is_sr_v3_metric_enabled, enforce1d)
            .await;
    }
    let merchant_id = gateway_scoring_data.merchantId.clone();
    let order_type = gateway_scoring_data.orderType.clone();
    let payment_method_type = gateway_scoring_data.paymentMethodType.clone();
    let payment_method = gateway_scoring_data.paymentMethod.clone();
    let card_network = gateway_scoring_data.cardSwitchProvider.clone();
    let card_isin = gateway_scoring_data.cardIsIn.clone();
    let currency = gateway_scoring_data
        .currency
        .as_ref()
        .map(|c| c.to_string());
    let country = gateway_scoring_data.country.as_ref().map(|c| c.to_string());
    let auth_type = gateway_scoring_data.authType.clone();
    let key_prefix = if is_sr_v3_metric_enabled {
        C::GATEWAY_SELECTION_V3_ORDER_TYPE_KEY_PREFIX.to_string()
    } else {
        C::GATEWAY_SELECTION_ORDER_TYPE_KEY_PREFIX.to_string()
    };

    // Base key components that are always present
    let mut key_components = vec![
        key_prefix,
        merchant_id.clone(),
        order_type,
        payment_method_type,
        payment_method,
    ];

    let name = format!("SR_DIMENSION_CONFIG_{}", merchant_id);

    let service_config = find_config_by_name(name.clone())
        .await
        .change_context(EuclidErrors::StorageError)
        .and_then(|opt_config| {
            opt_config.and_then(|config| config.value).ok_or_else(|| {
                error_stack::report!(EuclidErrors::InvalidSrDimensionConfig(
                    "SR dimension config not found".to_string()
                ))
            })
        })
        .and_then(|config| {
            serde_json::from_str::<SrDimensionConfig>(&config).change_context(
                EuclidErrors::InvalidSrDimensionConfig(
                    "Failed to parse SR dimension config".to_string(),
                ),
            )
        });

    let fields = service_config
        .map(|config| config.fields)
        .unwrap_or_default();

    for field in fields {
        if let Some(suffix) = field.strip_prefix("paymentInfo.") {
            match suffix {
                "card_network" => {
                    if let Some(cn) = card_network.clone() {
                        key_components.push(cn.peek().to_string());
                    }
                }
                "card_is_in" => {
                    if let Some(ci) = card_isin.clone() {
                        key_components.push(ci);
                    }
                }
                "currency" => {
                    if let Some(cu) = currency.clone() {
                        key_components.push(cu);
                    }
                }
                "country" => {
                    if let Some(co) = country.clone() {
                        key_components.push(co);
                    }
                }
                "auth_type" => {
                    if let Some(at) = auth_type.clone() {
                        key_components.push(at);
                    }
                }
                _ => {
                    // Unknown field under payment_info
                }
            }
        }
    }

    intercalate_without_empty_string("_", &key_components)
}

async fn get_legacy_unified_sr_key(
    gateway_scoring_data: &GatewayScoringData,
    is_sr_v3_metric_enabled: bool,
    enforce1d: bool,
) -> String {
    let merchant_id = gateway_scoring_data.merchantId.clone();
    let order_type = gateway_scoring_data.orderType.clone();
    let payment_method_type = gateway_scoring_data.paymentMethodType.clone();
    let payment_method = gateway_scoring_data.paymentMethod.clone();
    let key_prefix = if is_sr_v3_metric_enabled {
        C::GATEWAY_SELECTION_V3_ORDER_TYPE_KEY_PREFIX.to_string()
    } else {
        C::GATEWAY_SELECTION_ORDER_TYPE_KEY_PREFIX.to_string()
    };
    let base_key = vec![
        key_prefix.clone(),
        merchant_id.clone(),
        order_type.clone(),
        payment_method_type.clone(),
        payment_method.clone(),
    ];

    if enforce1d && payment_method_type == CARD {
        let res = &[
            base_key.clone(),
            vec![gateway_scoring_data.cardType.clone().unwrap_or_default()],
        ]
        .concat();
        intercalate_without_empty_string("_", res)
    } else if enforce1d {
        intercalate_without_empty_string("_", &base_key)
    } else if payment_method_type == UPI {
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
                    intercalate_without_empty_string(
                        "_",
                        &[base_key.clone(), vec![append_handle.to_string()]].concat(),
                    )
                }
                "UPI_PAY" | "PAY" => {
                    let package_list = get_upi_package_list().await;
                    let upi_package = gateway_scoring_data.paymentSource.as_deref().unwrap_or("");
                    let append_package = if package_list.contains(&upi_package.to_string()) {
                        upi_package
                    } else {
                        ""
                    };
                    intercalate_without_empty_string(
                        "_",
                        &[base_key.clone(), vec![append_package.to_string()]].concat(),
                    )
                }
                _ => intercalate_without_empty_string("_", &base_key),
            }
        } else {
            intercalate_without_empty_string("_", &base_key)
        }
    } else if payment_method_type == CARD {
        let v = &[
            base_key.clone(),
            vec![
                gateway_scoring_data.cardType.clone().unwrap_or_default(),
                gateway_scoring_data.authType.clone().unwrap_or_default(),
            ],
        ]
        .concat();
        if gateway_scoring_data.isAuthLevelEnabledForSrRouting {
            intercalate_without_empty_string("_", v)
        } else if gateway_scoring_data.isBankLevelEnabledForSrRouting {
            let top_bank_list = get_routing_top_bank_list().await;
            let bank_code = gateway_scoring_data
                .bankCode
                .as_deref()
                .unwrap_or("UNKNOWN");
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
            intercalate_without_empty_string("_", v)
        } else {
            let v = &[
                base_key.clone(),
                vec![gateway_scoring_data.cardType.clone().unwrap_or_default()],
            ]
            .concat();
            intercalate_without_empty_string("_", v)
        }
    } else {
        intercalate_without_empty_string("_", &base_key)
    }
}

pub async fn get_consumer_key(
    decider_flow: &mut DeciderFlow<'_>,
    gateway_scoring_data: GatewayScoringData,
    score_key_type: ScoreKeyType,
    enforce1d: bool,
    gateway_list: GatewayList,
) -> GatewayRedisKeyMap {
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
        let gw_ref_ids = gateway_list.iter().fold(HashMap::new(), |acc, gateway| {
            let mut map = acc;
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
            map.insert(gateway.clone(), Some(val));
            map
        });
        set_gw_ref_id(decider_flow, gw_ref_ids.values().next().cloned().flatten());
        logger::debug!("gwRefId {:?}", gw_ref_ids);
        gw_ref_ids
    } else {
        gateway_list
            .iter()
            .fold(HashMap::new(), |mut acc, gateway| {
                acc.insert(gateway.clone(), None);
                acc
            })
    };
    let gateway_redis_key_map = get_unified_key(
        gateway_scoring_data,
        score_key_type,
        enforce1d,
        gw_ref_id_map,
    )
    .await;
    gateway_redis_key_map
}

pub fn get_gateway_list(gwsm: GatewayScoreMap) -> Vec<String> {
    gwsm.keys().cloned().collect()
}

async fn set_routing_dimension_and_reference(
    decider_flow: &mut DeciderFlow<'_>,
    gateway_scoring_data: GatewayScoringData,
) {
    let base_dimension = vec![
        gateway_scoring_data.orderType,
        gateway_scoring_data.paymentMethodType.clone(),
        gateway_scoring_data.paymentMethod.clone(),
    ];
    let (final_dimension, routing_dimension_level) =
        if gateway_scoring_data.paymentMethodType == UPI {
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
        } else if gateway_scoring_data.paymentMethodType == CARD {
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
    set_routing_dimension(decider_flow, final_dimension.clone());
    set_routing_dimension_level(decider_flow, routing_dimension_level.clone());

    logger::info!(
        "Routing dimension: {:?}, Routing reference: {:?}",
        final_dimension,
        routing_dimension_level
    );
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
    let dimension = if gateway_scoring_data.paymentMethodType == CARD {
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
) {
    let base_dimension = vec![
        gateway_scoring_data.paymentMethodType.clone(),
        gateway_scoring_data.paymentMethod,
    ];
    let dimension = if gateway_scoring_data.paymentMethodType == CARD {
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
    } else if gateway_scoring_data.paymentMethodType == UPI {
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

pub fn route_random_traffic_to_explore(
    hedging_percent: f64,
    functional_gateways: Vec<String>,
    tag: String,
) -> bool {
    let num = generate_random_number(
        format!("GatewayDecider::routeRandomTrafficToExplore::{}", tag),
        (0.0, 100.0),
    );
    let explore_hedging_percent = hedging_percent * (functional_gateways.len() as f64);
    num < explore_hedging_percent
}

pub fn is_reset_eligibile(
    soft_ttl: Option<f64>,
    current_time_in_millis: u128,
    threshold: f64,
    cached_gateway_score: GatewayScore,
) -> bool {
    cached_gateway_score.score < threshold
        && cached_gateway_score.lastResetTimestamp
            < (current_time_in_millis - soft_ttl.unwrap_or(0.0) as u128)
                .try_into()
                .unwrap()
}
pub fn get_reset_score(min_threshold: f64, penalty_factor: f64, max_allowed_failures: i32) -> f64 {
    let reduction_factor = 1.0 - (penalty_factor / 100.0);
    let power = (max_allowed_failures - 1) as f64;
    let denominator = reduction_factor.powf(power);
    let result = min_threshold / denominator;
    result.min(1.0)
}

pub async fn writeToCacheWithTTL(
    key: String,
    cached_gateway_score: GatewayScore,
    ttl: i64,
) -> Result<i32, StorageError> {
    //from CachedGatewayScore comvert encoded_score to a encoded jasson that can be used as a value for redis sextx
    let encoded_score =
        serde_json::to_string(&cached_gateway_score).unwrap_or_else(|_| "".to_string());

    let primary_write =
        addToCacheWithExpiry("kv_redis".to_string(), key.clone(), encoded_score, ttl).await;

    match primary_write {
        Ok(_) => Ok(0),
        Err(err) => Err(err),
    }
}

// Original Haskell function: addToCacheWithExpiry
pub async fn addToCacheWithExpiry(
    redis_name: String,
    key: String,
    value: String,
    ttl: i64,
) -> Result<(), StorageError> {
    let app_state = get_tenant_app_state().await;
    let cached_resp = app_state.redis_conn.setx(&key, &value, ttl).await;
    match cached_resp {
        Ok(_) => Ok(()),
        Err(error) => Err(StorageError::InsertError),
    }
}

pub async fn get_penality_factor_(decider_flow: &mut DeciderFlow<'_>) -> f64 {
    let merchant = decider_flow.get().dpMerchantAccount.clone();
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let txn_card_info = decider_flow.get().dpTxnCardInfo.clone();
    let merchant_id = get_m_id(merchant.merchantId);
    let is_elimination_v2_enabled = isFeatureEnabled(
        C::EnableEliminationV2.get_key(),
        merchant_id.clone(),
        feedback::constants::kvRedis(),
    )
    .await;
    if is_elimination_v2_enabled {
        let m_reward_factor =
            eliminationV2RewardFactor(&merchant_id, &txn_card_info, &txn_detail).await;
        match m_reward_factor {
            Some(reward_factor) => return (1.0 - reward_factor),
            None => {
                return getPenaltyFactor(ScoreKeyType::EliminationMerchantKey).await;
            }
        }
    } else {
        return getPenaltyFactor(ScoreKeyType::EliminationMerchantKey).await;
    }
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
