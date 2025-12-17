// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 12:10:32

// Local Imports
use crate::{
    app::get_tenant_app_state,
    feedback::constants::{defaultGWScoringPenaltyFactor, defaultGWScoringRewardFactor},
    redis::types::ServiceConfigKey,
    types::{
        card::txn_card_info::TxnCardInfo, merchant, merchant_config::types::PfMcConfig,
        txn_details::types::TxnDetail,
    },
};

#[cfg(feature = "mysql")]
#[warn(unused_imports)]
use crate::storage::schema::gateway_bank_emi_support::gateway;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::gateway_bank_emi_support::gateway;

use crate::feedback::constants as C;

use crate::decider::gatewaydecider::constants::{EnableEliminationV2, EnableOutageV2};

// use crate::feedback::types as F_TYPES;

use crate::decider::gatewaydecider::utils::decode_and_log_error;

use crate::decider::gatewaydecider::gw_scoring::get_sr1_and_sr2_and_n;

//use crate::storage::types::MerchantAccount;
use crate::feedback::utils as EulerTransforms;
use crate::feedback::utils::GatewayScoringType as GatewayScoreType;
use crate::types::merchant::id as MID;
use crate::types::merchant::merchant_account as MA;
use crate::types::merchant::merchant_account::MerchantAccount;
use crate::types::payment_flow as PaymentFlow;
use crate::types::tenant::tenant_config::ModuleName as ModuleEnum;

use crate::redis::cache::findByNameFromRedis;

use crate::feedback::types::{
    // TxnCardInfo,
    // TxnDetail,
    CachedGatewayScore,
    MerchantScoringDetails,
};

use crate::logger;
use crate::redis::feature::RedisCompressionConfigCombined;
// use eulerhs::language::get_current_date_in_millis;
// use eulerhs::language as EL;

use crate::redis::feature::is_feature_enabled;

use crate::types::merchant::id as Merchant;
// use crate::types::txn_details::types::TxnDetail::
// use types::tenant_config as TenantConfig;

// use db::common::types::payment_flows as PF;
// use crate::utils::config::merchant_config as MerchantConfig;

use crate::merchant_config_util as MCU;

use crate::decider::gatewaydecider::types::{GatewayScoringData, ScoreKeyType};

use crate::utils as CUTILS;

// Prelude functions like fromIntegral, Foldable::length, and mapM are part of Rust's standard traits and methods.

// Haskell's Double corresponds to Rust's f64, which is built into the language.

// use encoding_rs as TE;

// use lens::set;
// use lens::view;

// Converted functions
// Original Haskell function: updateKeyScoreForKeysFromConsumer
pub async fn updateKeyScoreForKeysFromConsumer(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    gateway_scoring_type: GatewayScoreType,
    gateway_scoring_data: GatewayScoringData,
    mer_acc_p_id: merchant::id::MerchantPId,
    mer_acc: MerchantAccount,
    gateway_scoring_key: (ScoreKeyType, Option<String>),
    redis_compression_config: Option<RedisCompressionConfigCombined>,
) -> Option<((ScoreKeyType, String), CachedGatewayScore)> {
    let merchant_id = Merchant::merchant_id_to_text(txn_detail.merchantId.clone());
    let (score_key_type, m_key) = gateway_scoring_key;
    match m_key {
        Some(key) => {
            // let gateway = txn_detail.gateway.unwrap_or_else(|| "".to_string());
            let hard_key_ttl = getTTLForKey(score_key_type).await;
            let timestamp = CUTILS::get_current_date_in_millis();
            // let should_enforce_kv_redis = isFeatureEnabled(C::GatewayScoreThirdDimensionTtl.get_key(), merchant_id, C::kvRedis()).await;
            // let should_disable_fallback = isFeatureEnabled(C::SrScoreRedisFallbackLookupDisable.get_key(), merchant_id, C::kvRedis()).await;
            // let m_cached_gateway_score: Option<CachedGatewayScore> = readFromCacheWithFallback(should_enforce_kv_redis, should_disable_fallback, key);
            let m_cached_gateway_score = readGatewayScoreFromRedis(&key).await;
            let gw_score_to_be_updated: CachedGatewayScore = match m_cached_gateway_score {
                None => getNewCachedGatewayScore(
                    key.clone(),
                    gateway_scoring_type.clone(),
                    score_key_type,
                    txn_detail.clone(),
                    txn_card_info.clone(),
                ),
                Some(cached_gateway_score) => {
                    if (timestamp.clone() - cached_gateway_score.timestamp.clone())
                        > (hard_key_ttl.clone()) - 1000
                    {
                        logger::debug!(
                            action = "updateKeyScore",
                            tag = "updateKeyScore",
                            "{} has persisted longer than hardTTL",
                            key
                        );
                        getNewCachedGatewayScore(
                            key.clone(),
                            gateway_scoring_type.clone(),
                            score_key_type,
                            txn_detail.clone(),
                            txn_card_info.clone(),
                        )
                    } else {
                        cached_gateway_score
                    }
                }
            };
            let updated_cached_gateway_score = {
                let updated_merchant_details_array = getUpdatedMerchantDetailsForGlobalKey(
                    gw_score_to_be_updated.clone(),
                    score_key_type,
                    gateway_scoring_type.clone(),
                    txn_detail.clone(),
                    txn_card_info.clone(),
                    gateway_scoring_data.clone(),
                )
                .await;
                let updated_score = match gw_score_to_be_updated.score {
                    None => None,
                    Some(score) => Some(
                        updateKeyScoreForTxnStatus(
                            &txn_detail,
                            &txn_card_info,
                            &merchant_id,
                            gateway_scoring_type.clone(),
                            score,
                            score_key_type,
                            gateway_scoring_data,
                        )
                        .await,
                    ),
                };
                let transaction_count = getTransactionCount(
                    gw_score_to_be_updated.transactionCount,
                    score_key_type,
                    gateway_scoring_type,
                );
                CachedGatewayScore {
                    score: updated_score,
                    timestamp: gw_score_to_be_updated.timestamp,
                    merchants: updated_merchant_details_array,
                    lastResetTimestamp: gw_score_to_be_updated.lastResetTimestamp,
                    transactionCount: transaction_count,
                }
            };
            let encoded_json = serde_json::to_string(&updated_cached_gateway_score).unwrap();
            let elapsed_time =
                timestamp.saturating_sub(updated_cached_gateway_score.timestamp as u128);
            let remaining_ttl = (hard_key_ttl as u128).saturating_sub(elapsed_time);
            let safe_remaining_ttl = if remaining_ttl < 1000 {
                hard_key_ttl as i64
            } else {
                remaining_ttl as i64
            };
            let app_state: std::sync::Arc<crate::app::TenantAppState> =
                get_tenant_app_state().await;
            let result = EulerTransforms::writeToCacheWithTTL(
                key.clone(),
                updated_cached_gateway_score.clone(),
                safe_remaining_ttl,
                redis_compression_config,
            )
            .await;
            //To Do: add Ok & Err
            match result {
                Ok(_) => {
                    logger::debug!(
                        action = "updateKeyScore",
                        tag = "updateKeyScore",
                        "Updated score for key {}",
                        key
                    );
                }
                Err(_) => {
                    logger::debug!(
                        action = "updateKeyScore",
                        tag = "updateKeyScore",
                        "Unable to update score for key {}",
                        key
                    );
                }
            }
            Some(((score_key_type, key), updated_cached_gateway_score))
        }
        None => None,
    }
}

fn getTransactionCount(
    previous_transaction_count: Option<i32>,
    score_key_type: ScoreKeyType,
    gateway_scoring_type: GatewayScoreType,
) -> Option<i32> {
    if isGlobalKey(score_key_type) {
        None
    } else {
        match previous_transaction_count {
            None => Some(1),
            Some(transaction_count) => {
                if gateway_scoring_type == GatewayScoreType::Penalise {
                    Some(transaction_count + 1)
                } else {
                    Some(transaction_count)
                }
            }
        }
    }
}

// Original Haskell function: updateKeyScoreForTxnStatus
pub async fn updateKeyScoreForTxnStatus(
    txn_detail: &TxnDetail,
    txn_card_info: &TxnCardInfo,
    merchant_id: &String,
    gateway_scoring_type: GatewayScoreType,
    current_key_score: f64,
    score_key_type: ScoreKeyType,
    gateway_scoring_data: GatewayScoringData,
) -> f64 {
    let is_elimination_v2_enabled = is_feature_enabled(
        EnableEliminationV2.get_key(),
        merchant_id.clone(),
        C::kvRedis(),
    )
    .await;
    let is_elimination_v2_enabled_for_outage =
        is_feature_enabled(EnableOutageV2.get_key(), merchant_id.clone(), C::kvRedis()).await;
    let is_outage_key = isKeyOutage(score_key_type);
    logger::debug!(
        action = "updateKeyScore",
        tag = "IS_ELIMINATION_V2_ENABLED",
        "{}",
        is_elimination_v2_enabled
    );

    match gateway_scoring_type {
        GatewayScoreType::Penalise => {
            return updateScoreWithPenalty(
                is_elimination_v2_enabled,
                is_outage_key,
                is_elimination_v2_enabled_for_outage,
                &merchant_id,
                &txn_card_info,
                &txn_detail,
                current_key_score,
                &score_key_type,
                gateway_scoring_data.clone(),
            )
            .await;
        }
        GatewayScoreType::Reward => {
            return updateScoreWithReward(
                is_elimination_v2_enabled,
                is_outage_key,
                is_elimination_v2_enabled_for_outage,
                &merchant_id,
                &txn_card_info,
                &txn_detail,
                current_key_score,
                &score_key_type,
                gateway_scoring_data.clone(),
            )
            .await;
        }
        _ => return current_key_score,
    }
}

async fn updateScoreWithPenalty(
    is_elimination_v2_enabled: bool,
    is_outage_key: bool,
    is_elimination_v2_enabled_for_outage: bool,
    merchant_id: &str,
    txn_card_info: &TxnCardInfo,
    txn_detail: &TxnDetail,
    current_key_score: f64,
    score_key_type: &ScoreKeyType,
    gateway_scoring_data: GatewayScoringData,
) -> f64 {
    match (
        is_elimination_v2_enabled,
        is_outage_key,
        is_elimination_v2_enabled_for_outage,
    ) {
        (true, true, true) | (true, _, _) => {
            let m_reward_factor = eliminationV2RewardFactor(
                merchant_id,
                txn_card_info,
                txn_detail,
                gateway_scoring_data.isGriEnabledForElimination,
                gateway_scoring_data.gatewayReferenceId,
            )
            .await;
            match m_reward_factor {
                None => {
                    getFailureKeyScore(
                        false,
                        current_key_score,
                        getPenaltyFactor(score_key_type.clone()).await,
                    )
                    .await
                }
                Some(factor) => getFailureKeyScore(true, current_key_score, 1.0 - factor).await,
            }
        }
        _ => {
            getFailureKeyScore(
                false,
                current_key_score,
                getPenaltyFactor(score_key_type.clone()).await,
            )
            .await
        }
    }
}

async fn updateScoreWithReward(
    is_elimination_v2_enabled: bool,
    is_outage_key: bool,
    is_elimination_v2_enabled_for_outage: bool,
    merchant_id: &str,
    txn_card_info: &TxnCardInfo,
    txn_detail: &TxnDetail,
    current_key_score: f64,
    score_key_type: &ScoreKeyType,
    gateway_scoring_data: GatewayScoringData,
) -> f64 {
    match (
        is_elimination_v2_enabled,
        is_outage_key,
        is_elimination_v2_enabled_for_outage,
    ) {
        (true, true, true) | (true, _, _) => {
            let m_reward_factor = eliminationV2RewardFactor(
                merchant_id,
                txn_card_info,
                txn_detail,
                gateway_scoring_data.isGriEnabledForElimination,
                gateway_scoring_data.gatewayReferenceId,
            )
            .await;
            match m_reward_factor {
                None => getSuccessKeyScore(
                    false,
                    current_key_score,
                    getRewardFactor(score_key_type.clone()).await,
                ),
                Some(factor) => getSuccessKeyScore(true, current_key_score, factor),
            }
        }
        _ => getSuccessKeyScore(
            false,
            current_key_score,
            getRewardFactor(score_key_type.clone()).await,
        ),
    }
}

// Original Haskell function: getSuccessKeyScore
pub fn getSuccessKeyScore(use_elimination_v2: bool, current_score: f64, reward_factor: f64) -> f64 {
    let score = if use_elimination_v2 {
        current_score + reward_factor
    } else {
        current_score + (current_score * (reward_factor / 100.0))
    };
    if score > 1.0 {
        1.0
    } else {
        score
    }
}

// Original Haskell function: getFailureKeyScore
pub async fn getFailureKeyScore(
    use_elimination_v2: bool,
    current_score: f64,
    penalty_factor: f64,
) -> f64 {
    let m_score: Option<f64> = findByNameFromRedis(C::MinimumGatewayScore.get_key())
        .await
        .unwrap_or_default();
    let minimum_failure_score = m_score.unwrap_or(C::defaultMinimumGatewayScore());
    let score = if use_elimination_v2 {
        current_score * penalty_factor
    } else {
        current_score - (current_score * (penalty_factor / 100.0))
    };
    if score < minimum_failure_score {
        minimum_failure_score
    } else {
        score
    }
}

// Original Haskell function: getPenaltyFactor
pub async fn getPenaltyFactor(scoreKeyType: ScoreKeyType) -> f64 {
    let penalty_factor = if isKeyOutage(scoreKeyType) {
        findByNameFromRedis(C::OutagePenaltyFactor.get_key())
            .await
            .unwrap_or_else(|| defaultGWScoringPenaltyFactor())
    } else {
        findByNameFromRedis(C::GatewayPenaltyFactor.get_key())
            .await
            .unwrap_or_else(|| defaultGWScoringPenaltyFactor())
    };
    penalty_factor
}

// Original Haskell function: getRewardFactor
pub async fn getRewardFactor(scoreKeyType: ScoreKeyType) -> f64 {
    let reward_factor = if isKeyOutage(scoreKeyType) {
        findByNameFromRedis(C::OutageRewardFactor.get_key())
            .await
            .unwrap_or_else(|| defaultGWScoringRewardFactor())
    } else {
        findByNameFromRedis(C::OutageRewardFactor.get_key())
            .await
            .unwrap_or_else(|| defaultGWScoringRewardFactor())
    };
    reward_factor
}

// Original Haskell function: getUpdatedMerchantDetailsForGlobalKey
pub async fn getUpdatedMerchantDetailsForGlobalKey(
    cached_gateway_score: CachedGatewayScore,
    score_key_type: ScoreKeyType,
    gateway_scoring_type: GatewayScoreType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    gateway_scoring_data: GatewayScoringData,
) -> Option<Vec<MerchantScoringDetails>> {
    let merchant_id = Merchant::merchant_id_to_text(txn_detail.merchantId.clone());
    if isGlobalKey(score_key_type) {
        match cached_gateway_score.merchants {
            Some(merchant_details_array) => {
                let filtered_merchant_details_array =
                    findMerchantFromMerchantArray(&merchant_id, &merchant_details_array);
                if filtered_merchant_details_array.is_empty() {
                    let arr_max_length = getMerchantArrMaxLength().await;
                    if merchant_details_array.len() as i32 >= arr_max_length {
                        return Some(merchant_details_array);
                    } else {
                        let merchant_detail =
                            getDefaultMerchantScoringDetailsArray(merchant_id, 1.0, 1, None);
                        return Some([merchant_details_array, vec![merchant_detail]].concat());
                    }
                } else {
                    let mut results = Vec::new();
                    for merchant_scoring_details in merchant_details_array.iter() {
                        let result = replaceTransactionCount(
                            merchant_scoring_details.clone(),
                            &txn_detail,
                            &txn_card_info,
                            gateway_scoring_type.clone(),
                            score_key_type,
                            gateway_scoring_data.clone(),
                        )
                        .await;
                        results.push(result);
                    }
                    return Some(results);
                }
            }
            None => {
                let merchant_scoring_details =
                    getDefaultMerchantScoringDetailsArray(merchant_id, 1.0, 1, None);
                return Some(vec![merchant_scoring_details]);
            }
        }
    } else {
        return None;
    }
}

pub async fn replaceTransactionCount(
    merchant_scoring_details: MerchantScoringDetails,
    txn_detail: &TxnDetail,
    txn_card_info: &TxnCardInfo,
    gateway_scoring_type: GatewayScoreType,
    score_key_type: ScoreKeyType,
    gateway_scoring_data: GatewayScoringData,
) -> MerchantScoringDetails {
    let merchant_id = Merchant::merchant_id_to_text(txn_detail.merchantId.clone());
    if merchant_scoring_details.merchantId == merchant_id {
        let updated_score = updateKeyScoreForTxnStatus(
            txn_detail,
            txn_card_info,
            &merchant_scoring_details.merchantId,
            gateway_scoring_type.clone(),
            merchant_scoring_details.score,
            score_key_type,
            gateway_scoring_data.clone(),
        )
        .await;
        let new_count = if gateway_scoring_type == GatewayScoreType::Penalise {
            merchant_scoring_details.transactionCount + 1
        } else {
            merchant_scoring_details.transactionCount
        };
        MerchantScoringDetails {
            score: updated_score,
            transactionCount: new_count,
            ..merchant_scoring_details
        }
    } else {
        merchant_scoring_details
    }
}

// Original Haskell function: getNewCachedGatewayScore
pub fn getNewCachedGatewayScore(
    key: String,
    gateway_scoring_type: GatewayScoreType,
    score_key_type: ScoreKeyType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) -> CachedGatewayScore {
    let merchant_id = Merchant::merchant_id_to_text(txn_detail.merchantId);
    let current_date: u128 = CUTILS::get_current_date_in_millis();
    if isGlobalKey(score_key_type) {
        let merchant_scoring_details =
            getDefaultMerchantScoringDetailsArray(merchant_id, 1.0, 0, None);
        CachedGatewayScore {
            score: None,
            timestamp: current_date.clone(),
            lastResetTimestamp: None,
            merchants: Some(vec![merchant_scoring_details]),
            transactionCount: None,
        }
    } else {
        CachedGatewayScore {
            score: Some(1.0),
            timestamp: current_date.clone(),
            lastResetTimestamp: Some(current_date.clone()),
            merchants: None,
            transactionCount: Some(0),
        }
    }
}

// Original Haskell function: getDefaultMerchantScoringDetailsArray
pub fn getDefaultMerchantScoringDetailsArray(
    merchant_id: String,
    score: f64,
    transaction_count: i32,
    m_last_reset_timestamp: Option<i32>,
) -> MerchantScoringDetails {
    let current_date = CUTILS::get_current_date_in_millis();
    MerchantScoringDetails {
        score: score,
        merchantId: merchant_id,
        transactionCount: transaction_count,
        lastResetTimestamp: m_last_reset_timestamp.unwrap_or(current_date as i32),
    }
}

// Original Haskell function: getAllUnifiedKeys
pub async fn getAllUnifiedKeys(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    mer_acc_p_id: Merchant::MerchantPId,
    m_pf_mc_config: Option<PfMcConfig>,
    mer_acc: MerchantAccount,
    gateway_scoring_data: GatewayScoringData,
    gateway_reference_id: Option<String>,
) -> Vec<(ScoreKeyType, Option<String>)> {
    let merchant_id = Merchant::merchant_id_to_text(txn_detail.merchantId.clone());
    let is_key_enabled_for_global_gateway_scoring = is_feature_enabled(
        C::GlobalGatewayScoringEnabledMerchants.get_key(),
        merchant_id.clone(),
        C::kvRedis(),
    )
    .await;
    let is_key_enabled_for_merchant_gateway_scoring = gateway_scoring_data.eliminationEnabled
        || MCU::isPaymentFlowEnabledWithHierarchyCheck(
            mer_acc_p_id,
            mer_acc.tenantAccountId.clone(),
            ModuleEnum::MerchantConfig,
            PaymentFlow::PaymentFlow::EliminationBasedRouting,
            crate::types::country::country_iso::text_db_to_country_iso(
                mer_acc.country.as_deref().unwrap_or_default(),
            )
            .ok(),
        )
        .await;
    let is_gateway_scoring_enabled_for_global_outage = is_feature_enabled(
        C::GlobalOutageGatewayScoringEnabledMerchants.get_key(),
        merchant_id.clone(),
        C::kvRedis(),
    )
    .await;
    let is_gateway_scoring_enabled_for_merchant_outage =
        MCU::isPaymentFlowEnabledWithHierarchyCheck(
            mer_acc_p_id,
            mer_acc.tenantAccountId,
            ModuleEnum::MerchantConfig,
            PaymentFlow::PaymentFlow::Outage,
            crate::types::country::country_iso::text_db_to_country_iso(
                mer_acc.country.as_deref().unwrap_or_default(),
            )
            .ok(),
        )
        .await;

    let global_key = if is_key_enabled_for_global_gateway_scoring {
        let key = EulerTransforms::getProducerKey(
            txn_detail.clone(),
            Some(gateway_scoring_data.clone()),
            ScoreKeyType::EliminationGlobalKey,
            false,
            gateway_reference_id.clone(),
        )
        .await;
        vec![(ScoreKeyType::EliminationGlobalKey, key)]
    } else {
        logger::debug!(
            action = "getGlobalKeys",
            tag = "getGlobalKeys",
            "Global gateway scoring not enabled for merchant {:?}",
            merchant_id
        );
        vec![(ScoreKeyType::EliminationGlobalKey, None)]
    };

    let merchant_key = if is_key_enabled_for_merchant_gateway_scoring {
        let key = EulerTransforms::getProducerKey(
            txn_detail.clone(),
            Some(gateway_scoring_data.clone()),
            ScoreKeyType::EliminationMerchantKey,
            false,
            gateway_reference_id.clone(),
        )
        .await;
        vec![(ScoreKeyType::EliminationMerchantKey, key)]
    } else {
        logger::debug!(
            action = "getMerchantBasedKeys",
            tag = "getMerchantBasedKeys",
            "Merchant gateway scoring not enabled for merchant {:?}",
            merchant_id
        );
        vec![(ScoreKeyType::EliminationMerchantKey, None)]
    };

    let global_outage_keys = if is_gateway_scoring_enabled_for_global_outage {
        let key = EulerTransforms::getProducerKey(
            txn_detail.clone(),
            Some(gateway_scoring_data.clone()),
            ScoreKeyType::OutageGlobalKey,
            false,
            gateway_reference_id.clone(),
        )
        .await;
        vec![(ScoreKeyType::OutageGlobalKey, key)]
    } else {
        logger::debug!(
            action = "getGlobalKeys",
            tag = "getGlobalKeys",
            "Global gateway scoring not enabled for merchant {:?}",
            merchant_id
        );
        vec![(ScoreKeyType::OutageGlobalKey, None)]
    };

    let merchant_outage_keys = if is_gateway_scoring_enabled_for_merchant_outage {
        let key = EulerTransforms::getProducerKey(
            txn_detail.clone(),
            Some(gateway_scoring_data),
            ScoreKeyType::OutageMerchantKey,
            false,
            gateway_reference_id.clone(),
        )
        .await;
        vec![(ScoreKeyType::OutageMerchantKey, key)]
    } else {
        logger::debug!(
            action = "getMerchantScopedOutageKeys",
            tag = "getMerchantScopedOutageKeys",
            "Outage scoring not enabled for merchant {:?}",
            merchant_id
        );
        vec![(ScoreKeyType::OutageMerchantKey, None)]
    };

    global_key
        .into_iter()
        .chain(merchant_key)
        .chain(global_outage_keys)
        .chain(merchant_outage_keys)
        .collect()
}

// Original Haskell function: getTTLForKey
pub async fn getTTLForKey(score_key_type: ScoreKeyType) -> u128 {
    let is_key_global = isGlobalKey(score_key_type);
    let is_outage_key = isKeyOutage(score_key_type);
    let key: Option<f64> = match (is_key_global, is_outage_key) {
        (true, true) => findByNameFromRedis(C::GatewayScoreGlobalOutageTtl.get_key()).await,
        (false, true) => findByNameFromRedis(C::GatewayScoreOutageTtl.get_key()).await,
        (true, false) => findByNameFromRedis(C::GatewayScoreGlobalTtl.get_key()).await,
        _ => findByNameFromRedis(C::GatewayScoreThirdDimensionTtl.get_key()).await,
    };
    key.map_or_else(|| getDefaultTTL(score_key_type), |k| k.floor() as u128)
}

fn getDefaultTTL(score_key_type: ScoreKeyType) -> u128 {
    if isGlobalKey(score_key_type) {
        C::defaultScoreGlobalKeysTTL()
    } else {
        C::defaultScoreKeysTTL()
    }
}

pub async fn readGatewayScoreFromRedis(key: &str) -> Option<CachedGatewayScore> {
    let app_state = get_tenant_app_state().await;
    app_state
        .redis_conn
        .get_key::<CachedGatewayScore>(&key, "gateway_score_key")
        .await
        .map_or_else(|_| None, Some)
}

// pub async fn readFromCacheWithFallback<T>(
//     enforce_kv_redis: bool,
//     disable_fallback: bool,
//     key: str,
// ) -> Option<T> {
//     if enforce_kv_redis {
//         let m_kv_val =   getCachedVal(C.ecRedis, C.ecRedis2, &key)
//         let app_state = get_tenant_app_state().await;
//         app_state.redis_conn.get_key(&key, type_name)
//         app_state.(&key, str).await;
//         match m_kv_val {
//             Some(kv_val) => Some(kv_val),
//             None => {
//                 if disable_fallback {
//                     None
//                 } else {
//                     getCachedVal(C.ecRedis, C.ecRedis2, &key)
//                 }
//             }
//         }
//     } else {
//         let m_ec_val = getCachedVal(C.ecRedis, C.ecRedis2, &key);
//         match m_ec_val {
//             Some(ec_val) => Some(ec_val),
//             None => {
//                 if disable_fallback {
//                     None
//                 } else {
//                     getCachedVal(C.kvRedis, C.kvRedis2, &key)
//                 }
//             }
//         }
//     }
// }

// Original Haskell function: getMerchantScore
// pub fn getMerchantScore(
//     merchant_id: str,
//     merchants_array: Vec<MerchantScoringDetails>,
// ) -> Option<f64> {
//     let details = merchants_array.into_iter().find(|msd| msd.merchantId == merchant_id)?;
//     Some(details.score)
// }

// Original Haskell function: eliminationV2RewardFactor
pub async fn eliminationV2RewardFactor(
    merchant_id: &str,
    txn_card_info: &TxnCardInfo,
    txn_detail: &TxnDetail,
    is_gri_enabled_for_elimination: bool,
    gateway_reference_id: Option<String>,
) -> Option<f64> {
    let merch_acc: MerchantAccount =
        MA::load_merchant_by_merchant_id(MID::merchant_id_to_text(txn_detail.clone().merchantId))
            .await
            .expect("Merchant account not found");

    let error_tag = "Gateway Decider Input Decode Error";
    let m_gateway_success_rate_merchant_input =
        decode_and_log_error(error_tag, &merch_acc.gatewaySuccessRateBasedDeciderInput);
    // let m_gateway_success_rate_merchant_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput> = decodeAndLogError(
    //     "Gateway Decider Input Decode Error",
    //     &BSL::from_slice(&TE::encode_utf8(&merch_acc.gateway_success_rate_based_decider_input.unwrap_or_default())),
    // );

    // let txn_card_info = EulerTransforms::transform_ectxncard_info_to_eulertxncard_info(txn_card_info);
    // let txn_detail = EulerTransforms::transform_ectxn_detail_to_euler_txn_detail(txn_detail);

    let sr1_and_sr2_and_n = get_sr1_and_sr2_and_n(
        m_gateway_success_rate_merchant_input,
        merchant_id.to_string(),
        txn_card_info.clone(),
        txn_detail.clone(),
        is_gri_enabled_for_elimination,
        gateway_reference_id.clone(),
    )
    .await;

    match sr1_and_sr2_and_n {
        Some((sr1, sr2, n, n_, m_pmt, m_pm, m_txn_object_type, source)) => {
            logger::debug!(
                    "CALCULATING_ALPHA:SR1_SR2_N_PMT_PM_TXNOBJECTTYPE_CONFIGSOURCE {} {} {} {:?} {} {} {} {:?}",
                    sr1,
                    sr2,
                    n,
                    n_,
                    m_pmt.unwrap_or_else(|| "Nothing".to_string()),
                    m_pm.unwrap_or_else(|| "Nothing".to_string()),
                    m_txn_object_type.unwrap_or_else(|| "Nothing".to_string()),
                    source,
                );
            logger::debug!(
                action = "calculateAlpha",
                tag = "ALPHA_VALUE",
                alpha_value = calculate_alpha(sr1, sr2, n, n_),
            );

            Some(calculate_alpha(sr1, sr2, n, n_))
        }
        None => {
            logger::debug!("ELIMINATION_V2_VALUES_NOT_FOUND:ALPHA:PMT_PM_TXNOBJECTTYPE_SOURCEOBJECT {:?} {:?} {:?} {:?}",
                    txn_card_info.paymentMethodType,
                    if txn_card_info.paymentMethod.is_empty() { "Nothing".to_string() } else { txn_card_info.paymentMethod.clone() },
                    txn_detail.txnObjectType,
                    txn_detail.sourceObject.as_ref().map_or_else(|| "Nothing".to_string(), |s| s.clone()),
                );
            None
        }
    }
}

fn calculate_alpha(sr1: f64, sr2: f64, n: f64, n_prime: Option<f64>) -> f64 {
    match n_prime {
        None => ((sr1 - sr2) * (sr1 - sr2)) / ((n * n) * (sr1 * (100.0 - sr1))),
        Some(n_val) => {
            // These weights should be fetched from Env or config as per your environment
            let sr1_th_weight = 0.29;
            let sr2_th_weight = 0.71;
            let threshold = ((sr1_th_weight * sr1) + (sr2_th_weight * sr2)) / 100.0;
            let val1 = ((sr1 - sr2) * (sr1 - sr2)) / ((n * n) * (sr1 * (100.0 - sr1)));
            let val2 = (threshold / (sr1 / 100.0)).powf(1.0 / n_val);
            val1.min(val2)
        }
    }
}

// Original Haskell function: findMerchantFromMerchantArray
pub fn findMerchantFromMerchantArray(
    merchant_id: &str,
    merchants_array: &[MerchantScoringDetails],
) -> Vec<MerchantScoringDetails> {
    merchants_array
        .iter()
        .filter(|msd| msd.merchantId == merchant_id)
        .cloned()
        .collect()
}

// Original Haskell function: getMerchantArrMaxLength
pub async fn getMerchantArrMaxLength() -> i32 {
    let max_length = findByNameFromRedis(C::GatewayScoreMerchantArrMaxLength.get_key())
        .await
        .unwrap_or_else(|| C::defaultMerchantArrMaxLength());
    max_length
}

// Original Haskell function: isGlobalKey
pub fn isGlobalKey(scoreKeyType: ScoreKeyType) -> bool {
    scoreKeyType == ScoreKeyType::EliminationGlobalKey
        || scoreKeyType == ScoreKeyType::OutageGlobalKey
}

// Original Haskell function: isKeyOutage
pub fn isKeyOutage(scoreKeyType: ScoreKeyType) -> bool {
    scoreKeyType == ScoreKeyType::OutageGlobalKey || scoreKeyType == ScoreKeyType::OutageMerchantKey
}

// Original Haskell function: filterAndTransformOutageKeys
// pub fn filterAndTransformOutageKeys(
//     txn_detail: TxnDetail,
//     updated_scores: Vec<((ScoreKeyType, String), CachedGatewayScore)>,
// ) -> Vec<(GatewayScoringKeyType, CachedGatewayScore)> {
//     let outage_scores: Vec<_> = updated_scores
//         .into_iter()
//         .filter(|((score_key_type, _), _)| isKeyOutage(score_key_type))
//         .collect();

//     let transformed_scores: Vec<_> = outage_scores
//         .into_iter()
//         .map(|(key_type, score)| {
//             let transformed_key = transformOutageKey(key_type, txn_detail);
//             (transformed_key, score)
//         })
//         .collect();

//     transformed_scores
// }

// Original Haskell function: transformOutageKey
// pub fn transformOutageKey(
//     key_type: (ScoreKeyType, String),
//     txn_detail: TxnDetail,
// ) -> GatewayScoringKeyType {
//     let (score_key_type, key) = key_type;
//     let ttl = getTTLForKey(score_key_type);
//     GatewayScoringKeyType {
//         key: Some(key),transformOutageKey
//         ttl: Some(ttl),
//         downThreshold: None,
//         eliminationMaxCount: None,
//         dimension: None,
//         merchantId: txn_detail.merchantId,
//         gateway: txn_detail.gateway.unwrap_or_else(|| "".to_string()),
//         authType: None,
//         cardBin: None,
//         cardIssuerBankName: None,
//         paymentMethodType: None,
//         paymentMethod: None,
//         sourceObject: None,
//         paymentSource: None,
//         cardType: None,
//         keyType: if isGlobalKey(score_key_type) {
//             KeyType::Global
//         } else {
//             KeyType::Merchant
//         },
//         scoreType: ScoreType::Outage,
//         softTTL: None,
//     }
// }
