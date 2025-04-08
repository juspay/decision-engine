// Automatically converted from Haskell to Rust
// Generated on 2025-03-24 14:35:34

// // Converted imports
// use std::string::String as Text;
// use eulerhs::prelude::*;
// use std::vec::Vec;
// use serde_json as A;
// use std::string::String as T;
// use types::merchant::{MerchantPId, merchantPId};
// use eulerhs::language as L;
// use utils::config::envvars as EnvVars;
// use utils::redis as Redis;
// use std::collections::HashMap as HM;
// use types::merchantconfig::merchantconfig as MerchantConfig;
// use types::merchantconfig::types as MCTypes;
// use std::prelude::v1 as Prelude;
// use db::common::types::paymentflows as PF;
// use utils::config::types::{MAConfigs, SurchargeConfig, AutoRefundConfig, TenantConfigValueType, ConfigStatus};
// use types::tenantconfig as TC;
// use std::option::Option;
// use utils::config::serviceconfiguration as SC;
// use types::country::countryiso as ETCC;

use josekit::Value;

use crate::{
    decider::{
        configs::env_vars::enable_merchant_config_entity_lookup,
        gatewaydecider::constants::MERCHANT_CONFIG_ENTITY_LEVEL_LOOKUP_CUTOVER,
    },
    redis::{cache::findByNameFromRedis, types::ServiceConfigKey},
    types::{
        country::country_iso::CountryISO,
        merchant::id::MerchantPId,
        merchant_config::{
            merchant_config::{
                load_merchant_config_by_mpid_category_and_name,
                load_merchant_config_by_mpid_category_name_and_status, MerchantConfig,
            },
            types::{
                config_category_to_text, config_status_to_text, to_config_status, ConfigCategory,
                ConfigStatus, PfMcConfig,
            },
        },
        payment_flow::{payment_flows_to_text, PaymentFlow},
        tenant::tenant_config::{ConfigType, ModuleName},
        tenant_config::{
            get_arr_active_tenant_config_by_tenant_id_module_name_module_key_and_arr_type,
            get_arr_active_tenant_config_by_tenant_id_module_name_module_key_and_arr_type_and_country,
            TenantConfig,
        },
    },
};

// Converted data types
// Original Haskell data type: CONFIG_VALUE_STATUS
// #[derive(Debug, Serialize, Deserialize, PartialEq)]
// pub enum CONFIG_VALUE_STATUS<A> {
//     #[serde(rename = "DECODE_ERROR")]
//     DecodeError(String),

//     #[serde(rename = "CONFIG_DISABLED")]
//     ConfigDisabled,

//     #[serde(rename = "CONFIG_ENABLED")]
//     ConfigEnabled(A),

//     #[serde(rename = "NULL")]
//     Null,
// }

// // Converted functions
// // Original Haskell function: getMerchantConfigEntityLevelLookupConfig
pub async fn getMerchantConfigEntityLevelLookupConfig() -> Option<PfMcConfig> {
    if enable_merchant_config_entity_lookup() {
        findByNameFromRedis(MERCHANT_CONFIG_ENTITY_LEVEL_LOOKUP_CUTOVER.get_key()).await
    } else {
        None
    }
}

// // Original Haskell function: filterPaymentFlowsEnabledForMerchantConfig
// pub fn filterPaymentFlowsEnabledForMerchantConfig(
//     merchant_id: String,
//     payment_flows: Vec<String>,
//     key: impl SC::ServiceConfigKey,
//     should_query_pf_mc_config: Redis::ShouldQueryPfMcConfig,
// ) -> Vec<String> {
//     match should_query_pf_mc_config {
//         Redis::Skip(config) => filterPaymentFlows(config, &merchant_id, &payment_flows, &key),
//         Redis::Enforce => {
//             let config = getMerchantConfigEntityLevelLookupConfig();
//             filterPaymentFlows(config, &merchant_id, &payment_flows, &key)
//         }
//     }
// }

// fn filterPaymentFlows(
//     config: Option<Redis::PfMcConfig>,
//     merchant_id: &str,
//     payment_flows: &[String],
//     key: &impl SC::ServiceConfigKey,
// ) -> Vec<String> {
//     match config {
//         Some(config) => payment_flows
//             .iter()
//             .filter(|pf| Redis::checkMerchantEnabled(config.get(*pf), merchant_id, key))
//             .cloned()
//             .collect(),
//         None => Vec::new(),
//     }
// }

// Original Haskell function: isPaymentFlowEnabledForMerchantConfig
// pub fn isPaymentFlowEnabledForMerchantConfig(
//     pf: PaymentFlow,
//     merchant_id: String,
//     should_query_pf_mc_config: Redis.ShouldQueryPfMcConfig,
// ) -> bool {
//     match should_query_pf_mc_config {
//         Redis.ShouldQueryPfMcConfig::Skip(config) => is_payment_flow_enabled("::Skip", config),
//         Redis.ShouldQueryPfMcConfig::Enforce => {
//             let config = getMerchantConfigEntityLevelLookupConfig();
//             is_payment_flow_enabled("::Enforce", config)
//         }
//     }
// }

// fn is_payment_flow_enabled(
//     pf_type: &str,
//     m_pf_config: Option<Redis.PfMcConfig>,
// ) -> bool {
//     Redis.checkMerchantEnabled(
//         m_pf_config.and_then(|config| config.get(&pf.to_string())),
//         merchant_id,
//         SC.IS_PAYMENT_FLOW_ENABLED_FOR_MERCHANT_CONFIG(&pf.to_string(), pf_type),
//     )
// }

// Original Haskell function: isMerchantEnabledForPaymentFlows
pub async fn isMerchantEnabledForPaymentFlows(
    merchant_id: MerchantPId,
    payment_flows: Vec<PaymentFlow>,
) -> bool {
    let mc_arr = load_merchant_config_by_mpid_category_and_name(
        merchant_id,
        config_category_to_text(ConfigCategory::PAYMENT_FLOW),
        payment_flows
            .iter()
            .map(|pf: &PaymentFlow| payment_flows_to_text(pf))
            .collect::<Vec<_>>()
            .join(","),
    )
    .await;
    let is_valid_length = mc_arr.iter().count() == payment_flows.len();
    let are_all_pfs_enabled = mc_arr.iter().all(|mc| mc.status == ConfigStatus::ENABLED);
    if !is_valid_length {
        logMerConfigLengthMisMatchError(
            payment_flows
                .iter()
                .map(payment_flows_to_text)
                .collect(),
            mc_arr
                .into_iter()
                .filter_map(|mc| Some(mc.clone()))
                .collect(),
        );
    }
    is_valid_length && are_all_pfs_enabled
}

// // Original Haskell function: isMerchantEnabledForPaymentFlow
// pub fn isMerchantEnabledForPaymentFlow(
//     mer_acc_id: MerchantAccountId,
//     merchant_id: MerchantId,
//     payment_flow: PF::PaymentFlow,
//     should_query_pf_mc_config:  Redis::ShouldQueryPfMcConfig,
//     is_flow_enabled_at_default_entity_level: bool,
// ) -> bool {
//     if isPaymentFlowEnabledForMerchantConfig(payment_flow, &merchant_id, should_query_pf_mc_config) {
//         verifyIfPfIsEnabledAtMc(
//             &mer_acc_id,
//             payment_flow,
//             is_flow_enabled_at_default_entity_level,
//         )
//     } else {
//         is_flow_enabled_at_default_entity_level
//     }
// }

// fn verifyIfPfIsEnabledAtMc(
//     mer_acc_id: &MerchantAccountId,
//     payment_flow: PaymentFlow,
//     is_flow_enabled_at_default_entity_level: bool,
// ) -> bool {
//     let res = isPaymentFlowEnabledForMerchant(mer_acc_id, payment_flow);
//     if !res && is_flow_enabled_at_default_entity_level {
//         L.logErrorT(
//             "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//             &format!(
//                 "Payment flow {} is not enabled at merchant config for merchant account id {}",
//                 payment_flow,
//                 merchantPId(mer_acc_id),
//             ),
//         );
//     }
//     res
// }

// // Original Haskell function: getMerchantConfigStatusForPaymentFlows
// pub fn getMerchantConfigStatusForPaymentFlows(
//     mer_acc_id: MerchantAccountId,
//     merchant_id: MerchantId,
//     payment_flows: Vec<String>,
//     should_query_pf_mc_config: Redis.ShouldQueryPfMcConfi,
//     key: ServiceConfigKey,
// ) -> Vec<(String, RedisMerchantConfigStatus)> {
//     let enabled_pfs = filterPaymentFlowsEnabledForMerchantConfig(
//         merchant_id,
//         &payment_flows,
//         &key,
//         should_query_pf_mc_config,
//     );
//     let mc_status_for_enabled_pfs = getMerchantConfigStatus(&enabled_pfs);
//     let mc_status_for_not_enabled_pfs: Vec<(String, RedisMerchantConfigStatus)> = payment_flows
//         .iter()
//         .filter(|x| !enabled_pfs.contains(x))
//         .map(|x| (x.clone(), RedisMerchantConfigStatus::PaymentFlowNotEligible))
//         .collect();
//     mc_status_for_enabled_pfs
//         .into_iter()
//         .chain(mc_status_for_not_enabled_pfs.into_iter())
//         .collect()
// }

// fn getMerchantConfigStatus(
//     enabled_pfs: &[String],
// ) -> Vec<(String, RedisMerchantConfigStatus)> {
//     if enabled_pfs.is_empty() {
//         return vec![];
//     }
//     let mc_arr = MerchantConfig::loadArrMerchantConfigByMPidCategoryAndName(
//         mer_acc_id,
//         MCTypes::PAYMENT_FLOW,
//         &enabled_pfs.iter().map(|pf| MCTypes::ConfigName(pf.clone())).collect::<Vec<_>>(),
//     );
//     let result: Vec<(String, RedisMerchantConfigStatus)> = enabled_pfs
//         .iter()
//         .map(|pf| {
//             let status = mc_arr
//                 .iter()
//                 .find(|mc| pf == &mc.configName.configName)
//                 .map(|mc| mc.status.clone());
//             match status {
//                 Some(MCTypes::ENABLED) => (pf.clone(), RedisMerchantConfigStatus::Enabled),
//                 _ => (pf.clone(), RedisMerchantConfigStatus::Disabled),
//             }
//         })
//         .collect();
//     if mc_arr.len() != enabled_pfs.len() {
//         logMerConfigLengthMisMatchError(&enabled_pfs, &mc_arr);
//     }
//     result
// }

// // Original Haskell function: findAndVerifyIfPaymentFlowIsEnabled
// pub fn findAndVerifyIfPaymentFlowIsEnabled(
//     arr_flows_t: Vec<(String, RedisMerchantConfigStatus)>,
//     payment_flow: String,
// ) -> Option<bool> {
//     match arr_flows_t.iter().find(|(pf, _)| *pf == payment_flow) {
//         None => Some(false),
//         Some((_, status)) => match status {
//             RedisMerchantConfigStatus::PaymentFlowNotEligible => None,
//             RedisMerchantConfigStatus::Enabled => Some(true),
//             RedisMerchantConfigStatus::Disabled => Some(false),
//         },
//     }
// }

// Original Haskell function: isPaymentFlowEnabledForMerchant
pub async fn isPaymentFlowEnabledForMerchant(
    merchant_p_id: MerchantPId,
    payment_flow: PaymentFlow,
) -> bool {
    let config_name = payment_flows_to_text(&payment_flow);
    let config_category = config_category_to_text(ConfigCategory::PAYMENT_FLOW);
    let config_status = config_status_to_text(ConfigStatus::ENABLED);

    load_merchant_config_by_mpid_category_name_and_status(
        merchant_p_id,
        config_category,
        config_name,
        config_status,
    )
    .await
    .is_some()
}

// // Original Haskell function: isMerchantEnabledForFeature
// pub fn isMerchantEnabledForFeature(
//     feature_name: ServiceConfigKey,
//     merchant_id: String,
//     pf: PaymentFlow,
//     mer_acc_id: MerchantPId,
//     redis_name: String,
//     m_pf_mc_config: Redis.ShouldQueryPfMcConfig,
// ) -> bool {
//     let is_pf_eligible_for_mc = isPaymentFlowEnabledForMerchantConfig(&pf, &merchant_id, &m_pf_mc_config);
//     if is_pf_eligible_for_mc {
//         verifyIfPfIsEnabledAtMc(&pf, &mer_acc_id)
//     } else {
//         Redis::isFeatureEnabled(&feature_name, &merchant_id, &redis_name)
//     }
// }

// fn verifyIfPfIsEnabledAtMc(pf: &PaymentFlow, mer_acc_id: &MerchantPId) -> bool {
//     let res = isPaymentFlowEnabledForMerchant(mer_acc_id, pf);
//     if !res {
//         L::logErrorT(
//             "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//             &format!(
//                 "Payment flow {} is not enabled at merchant config for merchant account id {}",
//                 pf,
//                 merchantPId(mer_acc_id)
//             ),
//         );
//     }
//     res
// }

// // Original Haskell function: logMerConfigLengthMisMatchError
pub fn logMerConfigLengthMisMatchError(pfs: Vec<String>, mc_arr: Vec<MerchantConfig>) {
    println!(
        "Merchant config length mismatch for payment flows: {:?} and merchant config: {:?}",
        pfs, mc_arr
    );
}

// // Original Haskell function: getMerchantConfigValueForPaymentFlow
pub async fn getMerchantConfigValueForPaymentFlow(
    merchant_p_id_val: MerchantPId,
    pf: PaymentFlow,
) -> Option<Value> {
    let m_mer_config = load_merchant_config_by_mpid_category_name_and_status(
        merchant_p_id_val,
        config_category_to_text(ConfigCategory::PAYMENT_FLOW),
        payment_flows_to_text(&pf),
        config_status_to_text(ConfigStatus::ENABLED),
    )
    .await;

    match m_mer_config {
        Some(mer_config) => decodeConfigValue(&mer_config),
        None => {
            println!(
                "Merchant Config entry isn't present for Payment flow for merchant account id "
            );
            None
        }
    }
}

// fn getMerchantConfigValue<Val: FromJSON>(
//     merchant_p_id_val: MerchantPIdVal,
//     pf: PaymentFlow,
// ) -> Option<Val> {
//     let m_mer_config = load_merchant_config_by_mpid_category_name_and_status(
//         merchant_p_id_val,
//         ConfigCategory::PAYMENT_FLOW,
//         payment_flows_to_text(&pf),
//         ConfigStatus::ENABLED,
//     );

//     match m_mer_config {
//         Some(mer_config) => decodeConfigValue::<Val>(mer_config),
//         None => {
//             L.logErrorT(
//                 "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//                 &format!(
//                     "Payment flow {} is not enabled at merchant config for merchant account id {}",
//                     pf, merchant_p_id_val.merchantPId
//                 ),
//             );
//             None
//         }
//     }
// }

// fn decodeConfigValue<Val: FromJSON>(
//     mer_config: MerchantConfig,
// ) -> Option<Val> {
//     match mer_config.configValue.as_ref().and_then(|v| A.eitherDecodeStrict(&encodeUtf8(v)).ok()) {
//         Some(v) => Some(v),
//         None => {
//             L.logErrorT(
//                 "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//                 &format!(
//                     "Decoding merchant_config.config_value failed for ID: {}",
//                     MCTypes::merchantConfigPId(&mer_config.id)
//                 ),
//             );
//             None
//         }
//     }
// }

// // Original Haskell function: getMerchantConfigStatusAndvalueForPaymentFlow
pub async fn getMerchantConfigStatusAndvalueForPaymentFlow(
    merchant_p_id: MerchantPId,
    merchant_id: MerchantPId,
    pf: PaymentFlow,
    m_pf_mc_config: Option<MerchantConfig>,
) -> (ConfigStatus, Option<Value>) {
    let m_mer_config: Option<MerchantConfig> = load_merchant_config_by_mpid_category_and_name(
        merchant_p_id,
        config_category_to_text(ConfigCategory::PAYMENT_FLOW),
        payment_flows_to_text(&pf),
    )
    .await;
    match m_mer_config {
        Some(mer_config) => {
            //decode mer_config into jason using srde_json
            let decode_response = decodeConfigValue(&mer_config);
            match mer_config.status {
                ConfigStatus::ENABLED => (ConfigStatus::ENABLED, decode_response),
                ConfigStatus::DISABLED => (ConfigStatus::DISABLED, decode_response),
            }
        }
        None => {
            println!(
                "Merchant Config entry isn't present for Payment flow for merchant account id "
            );
            (ConfigStatus::DISABLED, None)
        }
    }
}

// async fn getMerchantConfigValue(
//     merchant_p_id: MerchantPId,
//     pf: PaymentFlow,
// ) -> (Option<MerchantConfig>) {
//     let m_mer_config = load_merchant_config_by_mpid_category_and_name(
//         merchant_p_id,
//         config_category_to_text(ConfigCategory::PAYMENT_FLOW),
//         payment_flows_to_text(&pf),
//     ).await;
//     match m_mer_config {
//         Some(mer_config) => {
//             let decode_response = decodeConfigValue(&mer_config);
//             match mer_config.status {
//                 ConfigStatus::ENABLED => (ConfigStatus::ENABLED, decode_response),
//                 ConfigStatus::DISABLED => (ConfigStatus::DISABLED, decode_response),
//                 ConfigStatus::PAYMENT_FLOW_NOT_ELIGIBLE => todo!(),
//             }
//         }
//         None => {
//             println!(
//                 "Merchant Config entry isn't present for Payment flow {} for merchant account id {}",
//                 pf.to_string(),
//                 merchant_p_id.to_string()
//             );
//             (Redis::Disabled, None)
//         }
//     }
// }

fn decodeConfigValue(mer_config: &MerchantConfig) -> Option<Value> {
    match mer_config
        .config_value
        .as_ref()
        .map(|v| serde_json::from_str(v))
    {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => {
            println!("Decoding merchant_config.config_value failed for Payment flow");
            None
        }
        None => {
            println!("Decoding merchant_config.config_value failed for Payment flow ");
            None
        }
    }
}

// Original Haskell function: isPaymentFlowEnabledWithHierarchyCheck
pub async fn isPaymentFlowEnabledWithHierarchyCheck(
    merchant_p_id: MerchantPId,
    m_tenant_account_id: Option<String>,
    module_name: ModuleName,
    payment_flow: PaymentFlow,
    m_iso_country_code: Option<CountryISO>,
) -> bool {
    let tenant_configs = getPaymentFlowInfoFromTenantConfig(
        m_tenant_account_id,
        module_name,
        payment_flows_to_text(&payment_flow),
        m_iso_country_code,
    )
    .await;

    if tenant_configs.is_empty() {
        return isPaymentFlowEnabledForMerchant(merchant_p_id, payment_flow).await;
    }

    let (override_config_arr, fallback_config_arr): (Vec<_>, Vec<_>) = tenant_configs
        .into_iter()
        .partition(|config| config._type == ConfigType::OVERRIDE);

    if let Some(override_tenant_config) = override_config_arr.into_iter().next() {
        return checkIfEnabledByTenant(
            &override_tenant_config.configValue,
            "FETCH_TENANT_CONFIG_WITH_OVERRIDE_DECODE_ERR",
        );
    }

    let category = config_category_to_text(ConfigCategory::PAYMENT_FLOW);

    let m_mer_config = load_merchant_config_by_mpid_category_and_name(
        merchant_p_id,
        category,
        payment_flows_to_text(&payment_flow),
    )
    .await;

    match m_mer_config {
        Some(mer_config) => match mer_config.status {
            ConfigStatus::ENABLED => true,
            ConfigStatus::DISABLED => false,
        },
        None => fallback_config_arr
            .into_iter()
            .next()
            .is_some_and(|fallback_config| {
                checkIfEnabledByTenant(
                    &fallback_config.configValue,
                    "FETCH_TENANT_CONFIG_WITH_FALLBACK_DECODE_ERR",
                )
            }),
    }
}

fn checkIfEnabledByTenant(config_value: &str, error_tag: &str) -> bool {
    let config_status = to_config_status(config_value);
    match config_status {
        Ok(ConfigStatus::ENABLED) => true,
        Ok(ConfigStatus::DISABLED) => false,
        Err(_) => {
            println!("Error: {}", error_tag);
            false
        }
    }
}

// // Original Haskell function: getPaymentFlowInfoFromTenantConfig
pub async fn getPaymentFlowInfoFromTenantConfig(
    m_tenant_account_id: Option<String>,
    module_name: ModuleName,
    payment_flow: String,
    m_iso_country_code: Option<CountryISO>,
) -> Vec<TenantConfig> {
    match m_tenant_account_id {
        None => vec![],
        Some(tenant_account_id) => match m_iso_country_code {
            None => get_arr_active_tenant_config_by_tenant_id_module_name_module_key_and_arr_type(
                tenant_account_id,
                module_name,
                payment_flow,
                vec![ConfigType::FALLBACK, ConfigType::OVERRIDE],

            ).await,
            Some(iso_country_code) => get_arr_active_tenant_config_by_tenant_id_module_name_module_key_and_arr_type_and_country(
                tenant_account_id,
                module_name,
                payment_flow,
                vec![ConfigType::FALLBACK, ConfigType::OVERRIDE],
                iso_country_code,
            ).await,
        },
    }
}

// // Original Haskell function: getMerchantConfigStatusAndValueForMAPfs
// pub fn getMerchantConfigStatusAndValueForMAPfs(
//     merchant_p_id: String,
//     merchant_id: String,
//     pf: PaymentFlow,
//     m_pf_mc_config: Option<MCPfMcConfig>,
//     ma_configs: MAConfigs,
// ) -> (Redis::MerchantConfigStatus, MAConfigs) {
//     let is_pf_eligible_for_mc = isPaymentFlowEnabledForMerchantConfig(&pf, &merchant_id, &m_pf_mc_config);
//     if is_pf_eligible_for_mc {
//         getMerchantConfigValue(merchant_p_id, pf, ma_configs)
//     } else {
//         (Redis::PaymentFlowNotEligible, ma_configs)
//     }
// }

// fn getMerchantConfigValue(
//     merchant_p_id: String,
//     pf: PaymentFlow,
//     ma_configs: MAConfigs,
// ) -> (Redis::MerchantConfigStatus, MAConfigs) {
//     let m_mer_config = MerchantConfig::loadMerchantConfigByMPidCategoryAndName(
//         &merchant_p_id,
//         MCTypes::PAYMENT_FLOW,
//         MCTypes::ConfigName(pf.to_string()),
//     );

//     match m_mer_config {
//         Some(mer_config) => {
//             let decode_response = decodeConfigValue(&mer_config, pf, &ma_configs);
//             match mer_config.status {
//                 MCTypes::ENABLED => (Redis::Enabled, decode_response),
//                 MCTypes::DISABLED => (Redis::Disabled, decode_response),
//             }
//         }
//         None => match ma_configs {
//             MAConfigs::ARC(arc_value) => {
//                 if vec![
//                     arc_value.auto_refund_conflict_transactions.is_some(),
//                     arc_value.auto_refund_multiple_charged_transactions.is_some(),
//                     arc_value.auto_refund_conflict_threshold_in_mins.is_some(),
//                 ]
//                 .iter()
//                 .any(|&x| x)
//                 {
//                     logErrorForConfigsEnabledAtMA(&pf, &merchant_p_id);
//                 }
//                 (Redis::Disabled, ma_configs)
//             }
//             MAConfigs::SC(surcharge_value) => {
//                 if vec![
//                     surcharge_value.include_surcharge_amount_for_refund.is_some(),
//                     surcharge_value.show_surcharge_breakup_screen.is_some(),
//                 ]
//                 .iter()
//                 .any(|&x| x)
//                 {
//                     logErrorForConfigsEnabledAtMA(&pf, &merchant_p_id);
//                 }
//                 (Redis::Disabled, ma_configs)
//             }
//             MAConfigs::NotExist => (Redis::Disabled, ma_configs),
//         },
//     }
// }

// fn logErrorForConfigsEnabledAtMA(pf: &PaymentFlow, merchant_p_id: &str) {
//     L::logErrorV::<String>(
//         "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//         &format!(
//             " Merchant Config entry isn't present for Payment flow {} for merchant account id {}",
//             pf, merchant_p_id
//         ),
//     );
// }

// fn decodeConfigValue(
//     mer_config: &MerchantConfig,
//     pf: PaymentFlow,
//     ma_configs: &MAConfigs,
// ) -> MAConfigs {
//     match pf {
//         PaymentFlow::SURCHARGE => match mer_config.config_value.as_ref().and_then(|v| {
//             A::eitherDecodeStrict::<SurchargeConfig>(&encodeUtf8(v)).ok()
//         }) {
//             Some(v) => MAConfigs::SC(v),
//             None => {
//                 L::logErrorT(
//                     "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//                     &format!(
//                         "Decoding merchant_config.config_value failed for Payment flow {}",
//                         pf
//                     ),
//                 );
//                 ma_configs.clone()
//             }
//         },
//         PaymentFlow::AUTO_REFUND => match mer_config.config_value.as_ref().and_then(|v| {
//             A::eitherDecodeStrict::<AutoRefundConfig>(&encodeUtf8(v)).ok()
//         }) {
//             Some(v) => MAConfigs::ARC(v),
//             None => {
//                 L::logErrorT(
//                     "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//                     &format!(
//                         "Decoding merchant_config.config_value failed for Payment flow {}",
//                         pf
//                     ),
//                 );
//                 ma_configs.clone()
//             }
//         },
//         _ => {
//             L::logErrorV::<String>(
//                 "MERCHANT_CONFIG_LEVEL_LOOKUP_FLOW",
//                 &format!(
//                     "Payment flow {} not supported for merchant_config.config_value decode",
//                     pf
//                 ),
//             );
//             MAConfigs::NotExist
//         }
//     }
// }

// // Original Haskell function: getConfigValueWithGCCatagory
// pub fn getConfigValueWithGCCatagory<T>(
//     arg1: i64,
//     arg2: String,
//     arg3: String,
//     arg4: Option<ETCC::CountryISO>,
// ) -> Option<T>
// where
//     T: serde::de::DeserializeOwned,
// {
//     getConfigValueFromMerchantConfig(MCTypes::GENERAL_CONFIG, arg1, arg2, arg3, arg4)
// }

// // Original Haskell function: getConfigValueWithPFCatagory
// pub fn getConfigValueWithPFCatagory(
//     arg1: i64,
//     arg2: String,
//     arg3: String,
//     arg4: Option<ETCC.CountryISO>,
// ) -> Option<A> {
//     getConfigValueFromMerchantConfig(MCTypes::PAYMENT_FLOW, arg1, arg2, arg3, arg4)
// }

// // Original Haskell function: getConfigValueFromMerchantConfig
// pub fn getConfigValueFromMerchantConfig<T: serde::de::DeserializeOwned>(
//     config_category: MCTypes::ConfigCategory,
//     m_acc_id: i64,
//     merchant_config_key: String,
//     tenant_acc_id: String,
//     m_iso_country_code: Option<ETCC::CountryISO>,
// ) -> Option<T> {
//     let tenant_config = getPaymentFlowInfoFromTenantConfig(
//         Some(&tenant_acc_id),
//         TC::MERCHANT_CONFIG,
//         &merchant_config_key,
//         m_iso_country_code,
//     );

//     match fetchConfigValueFromTenantConfig(&getOverrideEntry(&tenant_config)) {
//         ConfigResult::DECODE_ERROR(err) => {
//             L::logErrorT("FETCH_TENANT_CONFIG_WITH_OVERRIDE_DECODE_ERR", &err);
//             None
//         }
//         ConfigResult::CONFIG_DISABLED => None,
//         ConfigResult::CONFIG_ENABLED(val) => Some(val),
//         ConfigResult::NULL => {
//             match fetchConfigValueFromMerchantConfig(m_acc_id, &merchant_config_key, config_category) {
//                 ConfigResult::DECODE_ERROR(err) => {
//                     L::logErrorT("FETCH_MERCHANT_CONFIG_DECODE_ERR", &err);
//                     None
//                 }
//                 ConfigResult::CONFIG_DISABLED => None,
//                 ConfigResult::CONFIG_ENABLED(val) => Some(val),
//                 ConfigResult::NULL => match fetchConfigValueFromTenantConfig(&getFallbackEntry(&tenant_config)) {
//                     ConfigResult::DECODE_ERROR(err) => {
//                         L::logInfoT("FETCH_TENANT_CONFIG_WITH_FALLBACK_DECODE_ERR", &err);
//                         None
//                     }
//                     ConfigResult::CONFIG_DISABLED => None,
//                     ConfigResult::CONFIG_ENABLED(val) => Some(val),
//                     ConfigResult::NULL => None,
//                 },
//             }
//         }
//     }
// }

// fn getOverrideEntry<'a>(configs: &'a [Config]) -> Option<&'a Config> {
//     configs.iter().find(|cnf| cnf._type == TC::OVERRIDE)
// }

// fn getFallbackEntry<'a>(configs: &'a [Config]) -> Option<&'a Config> {
//     configs.iter().find(|cnf| cnf._type == TC::FALLBACK)
// }

// // Original Haskell function: decodingFunc
// pub fn decodingFunc<T: FromJSON>(config: Option<String>) -> CONFIG_VALUE_STATUS<T> {
//     match config {
//         None => CONFIG_VALUE_STATUS::NULL,
//         Some(config) => match A::eitherDecodeStrict(&encodeUtf8(&config)) {
//             Ok(val) => CONFIG_VALUE_STATUS::CONFIG_ENABLED(val),
//             Err(err) => CONFIG_VALUE_STATUS::DECODE_ERROR(T::from(err.to_string())),
//         },
//     }
// }

// // Original Haskell function: fetchConfigValueFromTenantConfig
// pub fn fetchConfigValueFromTenantConfig<T: serde::de::DeserializeOwned>(
//     tenant_config: Option<TC::TenantConfig>,
// ) -> CONFIG_VALUE_STATUS<T> {
//     match tenant_config {
//         None => CONFIG_VALUE_STATUS::NULL,
//         _ => unimplemented!(),
//     }
// }

// // Original Haskell function: fetchConfigValueFromMerchantConfig
// pub fn fetchConfigValueFromMerchantConfig<A>(
//     m_acc_id: i64,
//     merchant_config_key: String,
//     config_category: MCTypes::ConfigCategory,
// ) -> impl L::MonadFlow<CONFIG_VALUE_STATUS<A>>
// where
//     A: serde::de::DeserializeOwned,
// {
//     match getMerchantConfig(m_acc_id, &merchant_config_key, &config_category) {
//         None => CONFIG_VALUE_STATUS::NULL,
//         Some(cnf) => match cnf.status {
//             MCTypes::DISABLED => CONFIG_VALUE_STATUS::CONFIG_DISABLED,
//             MCTypes::ENABLED => decodingFunc(cnf.config_value),
//         },
//     }
// }

// fn getMerchantConfig(
//     m_acc_id: i64,
//     merchant_config_key: &str,
//     config_category: &MCTypes::ConfigCategory,
// ) -> Option<MerchantConfig::MerchantConfig> {
//     MerchantConfig::loadMerchantConfigByMPidCategoryAndName(
//         MerchantPId(m_acc_id),
//         config_category,
//         &MCTypes::ConfigName(merchant_config_key.to_string()),
//     )
// }
