// use eulerhs::prelude::*;
// use optics::core::{review, Field1};
use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::gw_filter::{getGws, setGws};
use crate::decider::gatewaydecider::types::{
    toListOfGatewayScore, DeciderFlow, DeciderScoringName, GatewayDeciderApproach, GatewayScoreMap,
    SRMetricLogData,
};
use crate::merchant_config_util::{isMerchantEnabledForPaymentFlows, isPaymentFlowEnabledWithHierarchyCheck};
use crate::redis::types::ServiceConfigKey;
use crate::storage::schema::txn_detail;
use crate::types::gateway_routing_input::{
    EliminationLevel, EliminationSuccessRateInput, GatewaySuccessRateBasedRoutingInput, GatewayWiseSuccessRateBasedRoutingInput, GlobalGatewayScore, GlobalScore, GlobalScoreLog, SelectionLevel
};
use crate::types::payment_flow::PaymentFlow;
use crate::types::tenant::tenant_config::ModuleName;
use crate::types::transaction::id::TransactionId;
use crate::utils::{generate_random_number, get_current_date_in_millis};
use diesel::dsl::update;
use rand::prelude::*;
use rand_distr::{Beta, Binomial, Distribution};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, PrimitiveDateTime};
// use crate::types::card_brand_routes as ETCBR;
use crate::types::gateway as ETG;
use crate::redis::feature::{self as M, isFeatureEnabled};
use crate::types::gateway_routing_input as ETGRI;
// use crate::types::gateway_health as ETGH;
use crate::types::card as ETCT;
use crate::types::payment as ETP;
// use crate::types::issuer_routes as ETIssuerR;
use crate::types::merchant as ETM;
use crate::types::txn_details::types as ETTD;
// use configs::env_vars as ENV;
// use utils::redis as Redis;
use crate::redis::cache::{self as RService, findByNameFromRedis};
// use utils::config::merchant_config as MerchantConfig;
// use utils::api_tag::*;
// use utils::wai::middleware::options as Options;
// use eulerhs::art::v2::types::ArtRecordable;
use crate::decider::gatewaydecider::constants as C;
use crate::decider::gatewaydecider::utils as Utils;
use crate::types::gateway_outage::{self as ETGO, GatewayOutage};
use crate::types::bank_code as ETJ;
// use juspay::extra::secret::unsafe_extract_secret;
// use juspay::extra::list as EList;
// use juspay::extra::env as Env;
// use servant::client as Client;
// use eulerhs::types as T;
// use eulerhs::api_helpers as T;
// use eulerhs::language as L;
// use eulerhs::tenant_redis_layer as RC;
// use data_random::rvar::run_rvar;
// use data_random::distribution::binomial::binomial;
// use data_random::distribution::beta::beta;
// use system_random::stateful::{init_std_gen, new_io_gen_m, IOGenM};
// use system_random::internal::StdGen;
use std::collections::HashMap as MP;
use std::iter::Iterator;
use std::option::Option;
use std::primitive;
use std::string::String as T;
use std::time::{SystemTime, UNIX_EPOCH};
use std::vec::Vec;

use super::types::{transform_gateway_wise_success_rate_based_routing, ConfigSource, DebugScoringEntry, DeciderGatewayWiseSuccessRateBasedRoutingInput, Dimension, DownTime, FilterLevel, Gateway, GatewayRedisKeyMap, GatewayScoringData, GlobalSREvaluationScoreLog, LogCurrScore, RankingAlgorithm, RedisKey, ResetApproach, ResetGatewayInput, ScoreKeyType, SrV3InputConfig, SuccessRate1AndNConfig};

// #[derive(Debug, ArtRecordable)]
// pub struct IOGenMStdGen;

pub fn get_gwsm(decider_flow: &DeciderFlow<'_>) -> GatewayScoreMap {
    decider_flow.writer.gwScoreMap.clone()
}

pub fn set_gwsm(decider_flow: &mut DeciderFlow<'_>, gwsm: GatewayScoreMap) {
    decider_flow.writer.gwScoreMap = gwsm;
}

pub fn get_decider_approach(decider_flow: &DeciderFlow<'_>) -> GatewayDeciderApproach {
    decider_flow.writer.gwDeciderApproach.clone()
}

pub fn set_decider_approach(decider_flow: &mut DeciderFlow<'_>, approach: GatewayDeciderApproach) {
    decider_flow.writer.gwDeciderApproach = approach;
}

pub fn set_is_scheduled_outage(decider_flow: &mut DeciderFlow<'_>, is_scheduled_outage: bool) {
    decider_flow.writer.isScheduledOutage = is_scheduled_outage;
}

pub fn get_sr_elimination_approach_info(decider_flow: &DeciderFlow<'_>) -> Vec<String> {
    decider_flow.writer.srElminiationApproachInfo.clone()
}

pub fn set_sr_elimination_approach_info(decider_flow: &mut DeciderFlow<'_>, approach: Vec<T>) {
    decider_flow.writer.srElminiationApproachInfo = approach;
}

pub fn set_metric_log_data(decider_flow: &mut DeciderFlow<'_>, log_data: SRMetricLogData) {
    decider_flow.writer.srMetricLogData = log_data;
}

pub fn reset_metric_log_data(decider_flow: &mut DeciderFlow<'_>) {
    decider_flow.writer.srMetricLogData = SRMetricLogData {
        gatewayAfterEvaluation: None,
        gatewayBeforeEvaluation: None,
        merchantGatewayScore: None,
        downtimeStatus: vec![],
        dateCreated: decider_flow
            .get()
            .dpTxnDetail
            .dateCreated
            .clone()
            .to_string()
            .replace(" UTC", "Z")
            .replace(" ", "T"),
        gatewayBeforeDowntimeEvaluation: decider_flow
            .writer
            .topGatewayBeforeSRDowntimeEvaluation
            .clone(),
    };
}

pub fn make_first_letter_small(s: String) -> String {
    if !s.is_empty() {
        let (first, rest) = s.split_at(1);
        format!("{}{}", first.to_lowercase(), rest)
    } else {
        s
    }
}

pub fn return_sm_with_log(
    decider_flow: &mut DeciderFlow<'_>,
    s_name: DeciderScoringName,
    do_or_not: bool,
) -> GatewayScoreMap {
    let sr = decider_flow.writer.gwScoreMap.clone();
    let txn_id = decider_flow.get().dpTxnDetail.txnId.clone();
    // log_debug!(
    //     "GW_Scoring",
    //     format!(
    //         "Gateway scores after {} for {} : {:?}",
    //         s_name,
    //         txn_id,
    //         sr.to_list_of_gateway_score()
    //     )
    // );
    if do_or_not {
        decider_flow
            .writer
            .debugScoringList
            .push(DebugScoringEntry {
                scoringName: make_first_letter_small(format!("{:?}", s_name)),
                gatewayScores: toListOfGatewayScore(sr.clone()),
            });
    }
    sr
}

pub async fn scoring_flow(
    decider_flow: &mut DeciderFlow<'_>,
    functional_gateways: Vec<ETG::Gateway>,
    gateway_priority_list: Vec<ETG::Gateway>,
    ranking_algorithm: Option<RankingAlgorithm>,
    elimination_enabled:Option<bool>,
) -> GatewayScoreMap {
    let merchant = decider_flow.get().dpMerchantAccount.clone();
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let txn_card_info = decider_flow.get().dpTxnCardInfo.clone();

    setGws(decider_flow, functional_gateways.clone());

    let gateway_scoring_data = decider_flow.writer.gateway_scoring_data.clone();

    if functional_gateways.len() == 1 {
        set_gwsm(
            decider_flow,
            create_score_map(functional_gateways.clone()),
        );
        set_decider_approach(decider_flow, GatewayDeciderApproach::DEFAULT);
        Utils::set_top_gateway_before_sr_downtime_evaluation(decider_flow, functional_gateways.first().cloned());
        let current_gateway_score_map = get_gwsm(decider_flow);
        update_gateway_score_based_on_success_rate(decider_flow, false, current_gateway_score_map, gateway_scoring_data, elimination_enabled).await;
        // log_info_t(
        //     "scoringFlow",
        //     format!(
        //         "Intelligent routing not triggered due to 1 gateway eligible for merchant {} and for txn Id {}",
        //         Utils::get_m_id(&merchant.merchant_id),
        //         review(ETTD::transaction_id_text, txn_detail.txn_id.clone())
        //     ),
        // );
    } else {
        let pmt = decider_flow.get().dpTxnCardInfo.paymentMethodType.clone();
        let pm = decider_flow.get().dpTxnCardInfo.paymentMethod.clone();
        let maybe_source_object = decider_flow.get().dpTxnDetail.sourceObject.clone();

        let pmt_str = pmt.to_text().to_string();
        let pm_str = Utils::get_payment_method(pmt_str.clone(), pm.clone(), maybe_source_object.unwrap_or_default());

        let is_merchant_enabled_for_sr_based_routing = isMerchantEnabledForPaymentFlows(
            merchant.id.clone(),
            vec![PaymentFlow::SR_BASED_ROUTING],
        ).await || ranking_algorithm == Some(RankingAlgorithm::SR_BASED_ROUTING);

        let is_sr_v3_metric_enabled = if is_merchant_enabled_for_sr_based_routing {
            let is_sr_v3_metric_enabled = isFeatureEnabled(
                C::enable_gateway_selection_based_on_sr_v3_input(pmt_str.clone()).get_key(),
                Utils::get_m_id(merchant.merchantId.clone()),
                "kv_redis".to_string(),
            ).await || ranking_algorithm == Some(RankingAlgorithm::SR_BASED_ROUTING);

            if is_sr_v3_metric_enabled {
                // log_info_t(
                //     "scoringFlow",
                //     format!(
                //         "Deciding Gateway based on SR V3 Routing for merchant {} and for txn Id {}",
                //         Utils::get_m_id(&merchant.merchant_id),
                //         review(ETTD::transaction_id_text, txn_detail.txn_id.clone())
                //     ),
                // );

                let merchant_sr_v3_input_config = findByNameFromRedis(
                    C::srV3InputConfig(Utils::get_m_id(merchant.merchantId.clone())).get_key(),
                ).await;
                let default_sr_v3_input_config = findByNameFromRedis(C::srV3DefaultInputConfig.get_key()).await;

                // log_info_v(
                //     "scoringFlow_Sr_V3_Input_Config",
                //     format!(
                //         "Sr V3 Input Config {:?}",
                //         merchant_sr_v3_input_config
                //     ),
                // );
                // log_info_v(
                //     "scoringFlow_Sr_V3_Default_Input_Config",
                //     format!(
                //         "Sr V3 Default Input Config {:?}",
                //         default_sr_v3_input_config
                //     ),
                // );

                let hedging_percent = Utils::get_sr_v3_hedging_percent(merchant_sr_v3_input_config.clone(), &pmt_str, pm.clone().as_str())
                .or_else(|| Utils::get_sr_v3_hedging_percent(default_sr_v3_input_config.clone(), &pmt_str, pm.clone().as_str()))
                .unwrap_or(C::defaultSrV3BasedHedgingPercent);

                Utils::set_sr_v3_hedging_percent(decider_flow, hedging_percent);

                let is_explore_and_exploit_enabled = isFeatureEnabled(
                    C::enableExploreAndExploitOnSrV3(pmt_str).get_key(),
                    Utils::get_m_id(merchant.merchantId.clone()),
                    "kv_redis".to_string(),
                ).await;

                let should_explore = if is_explore_and_exploit_enabled {
                    Utils::route_random_traffic_to_explore(
                        hedging_percent,
                        functional_gateways.clone(),
                        "SR_BASED_V3_ROUTING".to_string(),
                    )
                } else {
                    false
                };

                let initial_sr_gw_scores = if should_explore {
                    create_score_map(functional_gateways.clone())
                } else {
                    get_cached_scores_based_on_srv3(
                        decider_flow,
                        merchant_sr_v3_input_config,
                        default_sr_v3_input_config,
                        pm_str,
                        gateway_scoring_data.clone(),
                    ).await
                };

                let initial_sr_gw_scores_list = toListOfGatewayScore(initial_sr_gw_scores.clone());

                // log_info_v(
                //     "scoringFlow",
                //     format!(
                //         "Gateway Scores based on SR V3 Routing for txn id : {} is {:?}",
                //         review(ETTD::transaction_id_text, txn_detail.txn_id.clone()),
                //         initial_sr_gw_scores_list
                //     ),
                // );

                if !initial_sr_gw_scores.is_empty() {
                    Utils::set_sr_gateway_scores(decider_flow, initial_sr_gw_scores_list);

                    // log_info_t(
                    //     "scoringFlow",
                    //     format!(
                    //         "Considering Gateway Scores based on SR V3 for txn id : {}",
                    //         review(ETTD::transaction_id_text, txn_detail.txn_id.clone())
                    //     ),
                    // );

                    if should_explore {
                        set_decider_approach(decider_flow, GatewayDeciderApproach::SR_V3_HEDGING);
                    } else {
                        set_decider_approach(decider_flow, GatewayDeciderApproach::SR_SELECTION_V3_ROUTING);
                    }

                    let is_route_random_traffic_enabled = isFeatureEnabled(
                        C::routeRandomTrafficSrV3EnabledMerchant.get_key(),
                        Utils::get_m_id(merchant.merchantId.clone()),
                        "kv_redis".to_string(),
                    ).await;

                    let sr_gw_score = if is_route_random_traffic_enabled && !is_explore_and_exploit_enabled {
                        route_random_traffic(
                            decider_flow,
                            initial_sr_gw_scores.clone(),
                            hedging_percent,
                            true,
                            "SR_BASED_V3_ROUTING".to_string(),
                        )
                    } else {
                        initial_sr_gw_scores.clone()
                    };

                    set_gwsm(decider_flow, sr_gw_score.clone());
                    return_sm_with_log(decider_flow, DeciderScoringName::GetCachedScoresBasedOnSrV3, true);

                    if sr_gw_score.len() > 1 && (!is_explore_and_exploit_enabled || should_explore) {
                        let is_debug_mode_enabled = isFeatureEnabled(
                            C::enableDebugModeOnSrV3.get_key(), 
                            Utils::get_m_id(merchant.merchantId.clone()),
                            "kv_redis".to_string(),
                        );

                        // Utils::add_txn_to_hash_map_if_debug_mode(
                        //     is_debug_mode_enabled,
                        //     Utils::get_m_id(&merchant.merchant_id),
                        //     txn_detail.clone(),
                        // );
                    }

                    true
                } else {
                    // log_info_t(
                    //     "scoringFlow",
                    //     format!(
                    //         "Gateway Scores based on SR V3 for txn id : {} and for merchant : {} is null, So falling back to priorityLogic",
                    //         review(ETTD::transaction_id_text, txn_detail.txn_id.clone()),
                    //         Utils::get_m_id(&merchant.merchant_id)
                    //     ),
                    // );

                    return_sm_with_log(decider_flow, DeciderScoringName::GetCachedScoresBasedOnSrV3, true);
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        Utils::set_is_sr_v3_metric_enabled(decider_flow, is_sr_v3_metric_enabled);
        Utils::set_is_optimized_based_on_sr_metric_enabled(decider_flow, false);

        if !is_sr_v3_metric_enabled {
            // log_info_t(
            //     "scoringFlow",
            //     format!(
            //         "---- Ordering gateways available based on PRIORITY for merchant {} ----",
            //         Utils::get_m_id(&merchant.merchantId)
            //     ),
            // );
            set_decider_approach(decider_flow, GatewayDeciderApproach::PRIORITY_LOGIC);
            let gateway_score = get_score_with_priority(functional_gateways.clone(), gateway_priority_list.clone());
            set_gwsm(decider_flow, gateway_score.clone());
            return_sm_with_log(decider_flow, DeciderScoringName::GetScoreWithPriority, true);
            // log_info_v(
            //     "scoringFlow",
            //     format!(
            //         "Gateway scores after considering priority for {} : {:?}",
            //         review(ETTD::transaction_id_text, txn_detail.txn_id.clone()),
            //         toListOfGatewayScore(gateway_score.clone())
            //     ),
            // );
            // update_score_for_issuer(decider_flow);
            // update_score_for_isin(decider_flow);
            // update_score_for_card_brand(decider_flow);
        } else {
            // log_info_t("scoringFlow", "skipped priority for merchant");
        }

        update_score_for_outage(decider_flow).await;
        let current_gateway_score_map = get_gwsm(decider_flow);
        let top_gateway_before_sr_downtime_evaluation = Utils::get_max_score_gateway(&current_gateway_score_map.clone()).map(|(gw, _)| gw);
        Utils::set_top_gateway_before_sr_downtime_evaluation(decider_flow, top_gateway_before_sr_downtime_evaluation);
        update_gateway_score_based_on_success_rate(decider_flow, is_sr_v3_metric_enabled, current_gateway_score_map, gateway_scoring_data.clone(), elimination_enabled).await;
    }
    log_final_gateways_scoring(decider_flow)
}

pub async fn get_cached_scores_based_on_srv3(
    decider_flow: &mut DeciderFlow<'_>,
    merchant_srv3_input_config: Option<SrV3InputConfig>,
    default_srv3_input_config: Option<SrV3InputConfig>,
    pm: String,
    gateway_scoring_data: GatewayScoringData,
) -> GatewayScoreMap {
    let merchant = decider_flow.get().dpMerchantAccount.clone();
    let pmt = decider_flow.get().dpTxnCardInfo.paymentMethodType.clone();
    let order_ref = decider_flow.get().dpOrder.clone();
    let pmt_str = pmt.to_text();
    let functional_gateways = getGws(decider_flow);
    // log_debug_v("get_cached_scores_based_on_srv3", format!("my scoring flow functionalGateways {:?}", functional_gateways));

    let sr_gateway_redis_key_map: GatewayRedisKeyMap = Utils::get_consumer_key(
        decider_flow,
        gateway_scoring_data,
        super::types::ScoreKeyType::SR_V3_KEY,
        false,
        functional_gateways.clone(),
    )
    .await;

    let merchant_bucket_size =
        Utils::get_sr_v3_bucket_size(merchant_srv3_input_config.clone(), pmt_str, &pm)
            .or_else(|| {
                Utils::get_sr_v3_bucket_size(default_srv3_input_config.clone(), pmt_str, &pm)
            })
            .unwrap_or(C::DEFAULT_SR_V3_BASED_BUCKET_SIZE);

    // log_debug_t("Sr_V3_Bucket_Size", format!("{}", merchant_bucket_size));
    Utils::delete_score_key_if_bucket_size_changes(
        decider_flow,
        merchant_bucket_size,
        sr_gateway_redis_key_map.clone(),
    )
    .await;
    Utils::set_srv3_bucket_size(decider_flow, merchant_bucket_size);

    let mut score_map = GatewayScoreMap::new();
    for gw in functional_gateways.clone() {
        if let Some(key) = sr_gateway_redis_key_map.get(&format!("{:?}", gw)) {
            let score = get_score_from_redis(merchant_bucket_size, key).await;
            score_map.insert(gw, score);
        }
    }
    // log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation {:?}", score_map));
    reset_and_log_metrics(
        decider_flow,
        score_map.clone(),
        "SR_SELECTION_V3_EVALUATION".to_string(),
    ).await;

    let is_srv3_reset_enabled = M::isFeatureEnabled(
        C::ENABLE_RESET_ON_SR_V3.get_key(),
        Utils::get_m_id(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let updated_score_map_after_reset =
        if is_srv3_reset_enabled {
            let upper_reset_factor = Utils::get_sr_v3_upper_reset_factor(
                merchant_srv3_input_config.clone(),
                pmt_str,
                &pm,
            )
            .or_else(|| {
                Utils::get_sr_v3_upper_reset_factor(default_srv3_input_config.clone(), pmt_str, &pm)
            })
            .unwrap_or(C::defaultSrV3BasedUpperResetFactor);
            let lower_reset_factor = Utils::get_sr_v3_lower_reset_factor(
                merchant_srv3_input_config.clone(),
                pmt_str,
                &pm,
            )
            .or_else(|| {
                Utils::get_sr_v3_lower_reset_factor(default_srv3_input_config.clone(), pmt_str, &pm)
            })
            .unwrap_or(C::defaultSrV3BasedLowerResetFactor);
            // log_debug_t("Sr_V3_Upper_Reset_Factor", format!("{}", upper_reset_factor));
            // log_debug_t("Sr_V3_Lower_Reset_Factor", format!("{}", lower_reset_factor));
            let (updated_score_map_after_reset, is_reset_done) = reset_sr_v3_score(
                score_map.clone(),
                merchant_bucket_size,
                sr_gateway_redis_key_map.clone(),
                upper_reset_factor,
                lower_reset_factor,
            )
            .await;
            if is_reset_done {
                // log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Reset {:?}", updated_score_map_after_reset));
                reset_and_log_metrics(
                    decider_flow,
                    updated_score_map_after_reset.clone(),
                    "SR_SELECTION_V3_EVALUATION_AFTER_RESET".to_string(),
                ).await;
                Utils::set_reset_approach(decider_flow, ResetApproach::SRV3_RESET);
            }
            updated_score_map_after_reset
        } else {
            score_map
        };

    let is_srv3_extra_score_enabled = M::isFeatureEnabled(
        C::enable_extra_score_on_sr_v3.get_key(),
        Utils::get_m_id(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let final_score_map = if is_srv3_extra_score_enabled {
        let mut final_score_map = GatewayScoreMap::new();
        for gw in functional_gateways.clone() {
            let extra_score = add_extra_score(
                updated_score_map_after_reset.clone(),
                merchant_bucket_size,
                merchant_srv3_input_config.clone(),
                default_srv3_input_config.clone(),
                pmt_str.to_string(),
                pm.clone(),
                gw.clone(),
            );
            final_score_map.insert(gw, extra_score);
        }
        // log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Extra Score {:?}", final_score_map));
        reset_and_log_metrics(
            decider_flow,
            final_score_map.clone(),
            "SR_SELECTION_V3_EVALUATION_AFTER_EXTRA_SCORE".to_string(),
        ).await;
        final_score_map
    } else {
        updated_score_map_after_reset
    };

    let is_srv3_binomial_distribution_enabled = M::isFeatureEnabled(
        C::enable_binomial_distribution_on_sr_v3.get_key(),
        Utils::get_m_id(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let is_srv3_beta_distribution_enabled = M::isFeatureEnabled(
        C::enable_beta_distribution_on_sr_v3.get_key(),
        Utils::get_m_id(merchant.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;
    let final_score_map_after_distribution = match (
        is_srv3_binomial_distribution_enabled,
        is_srv3_beta_distribution_enabled,
    ) {
        (true, _) => {
            let mut final_score_map_after_distribution = GatewayScoreMap::new();
            for gw in functional_gateways.clone() {
                let final_score_after_distribution = sample_from_binomial_distribution(
                    final_score_map.clone(),
                    merchant_bucket_size,
                    gw.clone(),
                );
                final_score_map_after_distribution.insert(gw, final_score_after_distribution);
            }
            // log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Binomial Distribution {:?}", final_score_map_after_distribution));
            reset_and_log_metrics(
                decider_flow,
                final_score_map_after_distribution.clone(),
                "SR_SELECTION_V3_EVALUATION_AFTER_BINOMIAL_DISTRIBUTION".to_string(),
            ).await;
            final_score_map_after_distribution
        }
        (_, true) => {
            let mut final_score_map_after_distribution = GatewayScoreMap::new();
            for gw in functional_gateways {
                let final_score_map_distribution = sample_from_beta_distribution(
                    final_score_map.clone(),
                    merchant_bucket_size,
                    gw.clone(),
                );
                final_score_map_after_distribution.insert(gw, final_score_map_distribution);
            }
            // log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Beta Distribution {:?}", final_score_map_after_distribution));
            reset_and_log_metrics(
                decider_flow,
                final_score_map_after_distribution.clone(),
                "SR_SELECTION_V3_EVALUATION_AFTER_BETA_DISTRIBUTION".to_string(),
            ).await;
            final_score_map_after_distribution
        }
        (_, _) => final_score_map.clone(),
    };
    reset_and_log_metrics(
        decider_flow,
        final_score_map_after_distribution.clone(),
        "SR_SELECTION_V3_EVALUATION_FINAL".to_string(),
    ).await;
    final_score_map_after_distribution
}

pub fn sample_from_binomial_distribution(
    final_score_map: GatewayScoreMap,
    merchant_bucket_size: i32,
    gw: ETG::Gateway,
) -> f64 {
    let gw_score = final_score_map.get(&gw).copied().unwrap_or(1.0);
    let mut rng = rand::thread_rng();
    let binomial = Binomial::new(merchant_bucket_size as u64, gw_score).unwrap();
    let sample_value = binomial.sample(&mut rng);
    sample_value as f64 / merchant_bucket_size as f64
}

pub fn sample_from_beta_distribution(
    final_score_map: GatewayScoreMap,
    merchant_bucket_size: i32,
    gw: ETG::Gateway,
) -> f64 {
    let gw_score = final_score_map.get(&gw).copied().unwrap_or(1.0);
    let mut rng = rand::thread_rng();
    let gw_success = merchant_bucket_size as f64 * gw_score;
    let gw_failure = merchant_bucket_size as f64 - gw_success;
    let beta = Beta::new(gw_success, gw_failure).unwrap();
    beta.sample(&mut rng)
}

pub fn add_extra_score(
    updated_score_map_after_reset: GatewayScoreMap,
    merchant_bucket_size: i32,
    merchant_sr_v3_input_config: Option<SrV3InputConfig>,
    default_sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: String,
    pm: String,
    gw: ETG::Gateway,
) -> f64 {
    let gateway_sigma_factor =
        Utils::get_sr_v3_gateway_sigma_factor(merchant_sr_v3_input_config, &pmt, &pm, &gw)
            .or_else(|| {
                Utils::get_sr_v3_gateway_sigma_factor(default_sr_v3_input_config, &pmt, &pm, &gw)
            })
            .unwrap_or(C::DEFAULT_SR_V3_BASED_GATEWAY_SIGMA_FACTOR);
    // log_debug_t(
    //     "Sr_V3_Gateway_Sigma_Factor",
    //     format!(
    //         "Gateway: {:?}, Sigma Factor: {:?}",
    //         gw, gateway_sigma_factor
    //     ),
    // );
    let score = updated_score_map_after_reset.get(&gw).unwrap_or(&1.0);
    let float_bucket_size = merchant_bucket_size as f64;
    let var = (score * (1.0 - score)) / float_bucket_size;
    let sigma = var.sqrt();
    let extra_score = sigma * gateway_sigma_factor;
    (score + extra_score).clamp(0.0, 1.0)
}

pub async fn reset_sr_v3_score(
    score_map: GatewayScoreMap,
    bucket_size: i32,
    sr_gateway_redis_key_map: GatewayRedisKeyMap,
    upper_reset_factor: f64,
    lower_reset_factor: f64,
) -> (GatewayScoreMap, bool) {
    let max_score = Utils::get_max_score_gateway(&score_map)
        .map(|(_, score)| score)
        .unwrap_or(1.0);
    let float_bucket_size = bucket_size as f64;
    let var = (max_score * (1.0 - max_score)).max(0.09) / float_bucket_size;
    let sigma = var.sqrt();
    let score_reset_threshold = max_score - (upper_reset_factor * sigma);
    let number_of_zeros = (((1.0 - max_score + lower_reset_factor * sigma) * float_bucket_size)
        .floor() as i32)
        .clamp(2, bucket_size);
    let interval_between_zeros = (float_bucket_size - 1.0) / (number_of_zeros as f64 - 1.0);
    let mut score_list = (1..=bucket_size)
        .rev()
        .fold((0, Vec::new()), |(zc, mut acc), i| {
            if float_bucket_size - i as f64 >= zc as f64 * interval_between_zeros {
                acc.push("0".to_string());
                (zc + 1, acc)
            } else {
                acc.push("1".to_string());
                (zc, acc)
            }
        })
        .1;
    score_list.reverse();
    let score_reset_value = bucket_size - number_of_zeros;
    let key_score_map: Vec<_> = score_map
        .iter()
        .filter_map(|(gw, score)| {
            sr_gateway_redis_key_map
                .get(&format!("{:?}", gw))
                .map(|key| (key.clone(), *score))
        })
        .collect();
    let keys_for_reset: Vec<_> = key_score_map
        .iter()
        .filter(|(_, score)| *score < score_reset_threshold)
        .map(|(key, _)| key.clone())
        .collect();
    let updated_score_map = score_map
        .iter()
        .map(|(gw, score)| {
            (
                gw.clone(),
                if *score < score_reset_threshold {
                    score_reset_value as f64 / float_bucket_size
                } else {
                    *score
                },
            )
        })
        .collect();
    for key in keys_for_reset.clone() {
        reset_gateway_for_sr_v3(score_reset_value, &score_list, key.clone()).await;
    };
    (updated_score_map, !keys_for_reset.is_empty())
}

pub async fn reset_gateway_for_sr_v3(
    score_reset_value: i32,
    score_list: &Vec<String>,
    redis_key: String,
) {
    let score_key = format!("{}{}", redis_key, "_}score");
    let queue_key = format!("{}{}", redis_key, "_}queue");
    Utils::create_moving_window_and_score("kv_redis".to_string(),queue_key, score_key, score_reset_value, score_list.to_vec())
        .await
}

pub async fn get_score_from_redis(bucket_size: i32, redis_key: &RedisKey) -> f64 {
    let score_key = format!("{}{}", redis_key, "_}score");
    let app_state = get_tenant_app_state().await;
    let success_count = app_state
        .redis_conn
        .get_key::<i32>(&score_key, "sr_v3_score_key")
        .await
        .unwrap_or(bucket_size);
    (success_count as f64 / bucket_size as f64).clamp(0.0, 1.0)
}

pub fn create_score_map(gateways: Vec<ETG::Gateway>) -> GatewayScoreMap {
    gateways.iter().map(|gw| (gw.clone(), 1.0)).collect()
}

pub fn prepare_log_curr_score(
    acc: &mut Vec<LogCurrScore>,
    gw: ETG::Gateway,
    score: f64,
) -> &Vec<LogCurrScore> {
    acc.push(LogCurrScore {
        gateway: format!("{:?}", gw),
        current_score: score,
    });
    acc
}

pub async fn reset_and_log_metrics(
    decider_flow: &mut DeciderFlow<'_>,
    final_updated_gateway_score_maps: GatewayScoreMap,
    metric_title: String,
) {
    reset_metric_log_data(decider_flow);
    decider_flow.writer.srMetricLogData.merchantGatewayScore = Some(
        serde_json::to_value(final_updated_gateway_score_maps.iter().fold(
            Vec::new(),
            |mut acc, (gw, score)| {
                prepare_log_curr_score(&mut acc, gw.clone(), *score);
                acc
            },
        ))
        .unwrap_or_default(),
    );
    Utils::metric_tracker_log(
        metric_title.clone().as_str(),
        "GW_SCORING",
        Utils::get_metric_log_format(decider_flow, metric_title.as_str()),
    )
    .await;
}

pub fn get_score_with_priority(
    functional_gateways: Vec<ETG::Gateway>,
    gateway_priority_list: Vec<ETG::Gateway>,
) -> GatewayScoreMap {
    let (p1, im1) = gateway_priority_list
        .iter()
        .fold((1.0, MP::new()), |(p, m), gw| {
            if functional_gateways.contains(gw) {
                (p - 0.1, {
                    let mut m = m;
                    m.insert(gw.clone(), p);
                    m
                })
            } else {
                (p, m)
            }
        });

    functional_gateways
        .iter()
        .fold((p1, im1), |(p, m), gw| {
            if !gateway_priority_list.contains(gw) {
                (p, {
                    let mut m = m;
                    m.insert(gw.clone(), p);
                    m
                })
            } else {
                (p, m)
            }
        })
        .1
}

pub async fn update_score_for_outage(decider_flow: &mut DeciderFlow<'_>) -> GatewayScoreMap {
    let old_sm: MP<ETG::Gateway, f64> = get_gwsm(decider_flow);
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let txn_card_info = decider_flow.get().dpTxnCardInfo.clone();
    let merchant = decider_flow.get().dpMerchantAccount.clone();
    let scheduled_outage_validation_duration = RService::findByNameFromRedis(C::SCHEDULED_OUTAGE_VALIDATION_DURATION.get_key())
        .await.unwrap_or(86400);

    let potential_outages = get_scheduled_outage(scheduled_outage_validation_duration).await;
    // log_debug_v("updated score for outage", &potential_outages);

    let juspay_bank_code = Utils::fetch_juspay_bank_code(&txn_card_info);
    let bank_code =  match juspay_bank_code {
        None => None,
        Some(code) => ETJ::find_bank_code(code).await
    };
    let out_gws: Vec<_> = potential_outages
        .into_iter()
        .filter(|outage| check_scheduled_outtage(&txn_detail, &txn_card_info, &merchant.merchantId, &bank_code, outage.clone()))
        .collect();

    // log_debug_v("updated score for outage filtered", &out_gws);
    // log_debug_v(
    //     "updated score for outage info",
    //     format!(
    //         "{:?}, {:?}, {:?}",
    //         txn_detail.txn_object_type,
    //         txn_detail.source_object,
    //         Utils::fetch_juspay_bank_code(&txn_card_info)
    //     ),
    // );

    let new_sm = out_gws.iter().fold(old_sm, |mut acc, gw| {
        if let Some(gw) = gw.gateway.clone() {
            acc.insert(gw.clone(), acc.get(&gw).unwrap_or(&1.0) / 10.0);
        }
        acc
    });

    if !out_gws.is_empty() {
        set_is_scheduled_outage(decider_flow, true);
    }

    set_gwsm(decider_flow, new_sm);
    return_sm_with_log(decider_flow,DeciderScoringName::UpdateScoreForOutage, true)
}

// checkScheduledOutage :: ETTD.TxnDetail -> TxnCardInfo -> ETM.MerchantId -> Maybe ETJ.JuspayBankCode -> ETGO.GatewayOutage -> Bool
// checkScheduledOutage txnDetail txnCardInfo merchantId juspayBankCode scheduledOutage =
//   (scheduleEqualTo (==) (Just merchantId) scheduledOutage.merchantId)
//   && (if (txnCardInfo.paymentMethod == "UPI")
//         then (scheduleEqualTo (==) txnDetail.sourceObject scheduledOutage.paymentMethod)
//         else scheduleEqualTo (==)  (Just txnCardInfo.paymentMethod) scheduledOutage.paymentMethod)
//   && (scheduleEqualTo (==) (Just txnCardInfo.paymentMethodType) scheduledOutage.paymentMethodType)
//   && (scheduleEqualTo (\jbc outageBank -> jbc.bankCode == outageBank || jbc.bankName == outageBank ) juspayBankCode  scheduledOutage.bank)
//   && (checkScheduledOutageMetadata txnDetail txnCardInfo scheduledOutage.metadata)

//convert the above haskell code to rust code

// scheduleEqualTo :: (a -> b -> Bool) -> Maybe a -> Maybe b -> Bool
// scheduleEqualTo _   _            Nothing               = True
// scheduleEqualTo _   Nothing      (Just _)              = False
// scheduleEqualTo cmp (Just input) (Just scheduleOutage) = cmp input scheduleOutage

fn schedule_equal_to<F, A, B>(cmp: F, input: Option<A>, schedule_outage: Option<B>) -> bool
where
    F: Fn(A, B) -> bool,
{
    match (input, schedule_outage) {
        (_, None) => true,
        (None, _) => false,
        (Some(input), Some(schedule_outage)) => cmp(input, schedule_outage),
    }
}

fn check_scheduled_outtage(
    txn_detail: &ETTD::TxnDetail,
    txn_card_info: &ETCT::txn_card_info::TxnCardInfo,
    merchant_id: &ETM::id::MerchantId,
    juspay_bank_code: &Option<ETJ::BankCode>,
    scheduled_outage: ETGO::GatewayOutage,
) -> bool {
    schedule_equal_to(
        |x: ETM::id::MerchantId, y: ETM::id::MerchantId| x == y,
        Some(merchant_id.clone()),
        scheduled_outage.merchantId.clone(),
    ) && if txn_card_info.paymentMethodType == ETP::payment_method::PaymentMethodType::UPI {
        schedule_equal_to(
            |x, y| x == y,
            txn_detail.sourceObject.clone(),
            scheduled_outage.paymentMethod.clone(),
        )
    } else {
        schedule_equal_to(
            |x, y| x == y,
            Some(txn_card_info.paymentMethod.clone()),
            scheduled_outage.paymentMethod.clone(),
        )
    } && schedule_equal_to(
        |x, y| x == y,
        Some(txn_card_info.paymentMethodType.clone()),
        scheduled_outage.paymentMethodType.clone(),
    ) && schedule_equal_to(
        |jbc, outage_bank| {
            if let Some(jbc) = jbc {
                jbc.bank_code == outage_bank || jbc.bank_name == outage_bank
            } else {
                false
            }
        },
        Some(juspay_bank_code.clone()),
        scheduled_outage.bank.clone(),
    ) && check_scheduled_outage_metadata(txn_detail, txn_card_info, scheduled_outage.metadata.clone())
}

// checkScheduledOutageMetadata :: ETTD.TxnDetail -> TxnCardInfo -> Maybe ETGO.ScheduledOutageMetadata -> Bool
//     checkScheduledOutageMetadata _         _           Nothing                        = True
//     checkScheduledOutageMetadata txnDetail txnCardInfo (Just scheduledOutageMetadata) = 
//       (scheduleEqualTo (==) (Just txnDetail.txnObjectType) scheduledOutageMetadata.txnObjectType)
//       && (scheduleEqualTo (==) txnDetail.sourceObject scheduledOutageMetadata.sourceObject)
//       && (scheduleEqualTo (==) Nothing scheduledOutageMetadata.flowType) 
//       && (case txnCardInfo.paymentMethodType of
//         ETP.Card -> scheduleEqualTo (==) txnCardInfo.cardType scheduledOutageMetadata.cardType
//         ETP.UPI  -> maybe False (\paymentSource -> bool
//                   (scheduleEqualTo (==) (Just paymentSource) scheduledOutageMetadata.app)
//                   (scheduleEqualTo (==) (Just paymentSource) scheduledOutageMetadata.handle)
//                   (T.any (== '@') paymentSource)) txnCardInfo.paymentSource
//         _    -> True)
//     getScheduledOutage scheduledOutageValidationDuration = do
//           currentTime <- getCurrentTimeUTC
//           scheduledOutages <- ETGO.getPotentialGwOutages currentTime
//           let validatedOutages = filter (validateScheduledOutage scheduledOutageValidationDuration) scheduledOutages
//           L.logDebugV  @Text "scheduled Outages length" $ length scheduledOutages
//           if (length validatedOutages /= length scheduledOutages) then L.logDebugV  @Text "scheduled Outages filtered" (length validatedOutages, length scheduledOutages) else pure ()
//           pure validatedOutages
// convert the above haskell code to rust code

fn check_scheduled_outage_metadata(
    txn_detail: &ETTD::TxnDetail,
    txn_card_info: &ETCT::txn_card_info::TxnCardInfo,
    scheduled_outage_metadata: Option<ETGO::ScheduledOutageMetadata>,
) -> bool {
    match scheduled_outage_metadata {
        None => true,
        Some(scheduled_outage_metadata) => {
            schedule_equal_to(
                |x, y| x == y,
                Some(txn_detail.txnObjectType.clone()),
                scheduled_outage_metadata.txnObjectType.clone(),
            ) && schedule_equal_to(
                |x, y| x == y,
                txn_detail.sourceObject.clone(),
                scheduled_outage_metadata.sourceObject.clone(),
            ) && schedule_equal_to(
                |x: _, y| x == Some(y),
                Some(None),
                scheduled_outage_metadata.flowType.clone(),
            ) && match txn_card_info.paymentMethodType {
                ETP::payment_method::PaymentMethodType::Card => schedule_equal_to(
                    |x, y| x == y,
                    txn_card_info.card_type.clone(),
                    scheduled_outage_metadata.cardType.clone(),
                ),
                ETP::payment_method::PaymentMethodType::UPI => txn_card_info
                    .paymentSource
                    .as_ref()
                    .map_or(false, |payment_source| {
                        if payment_source.contains('@') {
                            false
                        } else {
                            schedule_equal_to(
                                |x, y| x == y,
                                Some(payment_source.clone()),
                                scheduled_outage_metadata.app.clone(),
                            ) && schedule_equal_to(
                                |x, y| x == y,
                                Some(payment_source.clone()),
                                scheduled_outage_metadata.handle.clone(),
                            )
                        }
                    }),
                _ => true,
            }
        }
    }
}



async fn get_scheduled_outage(scheduled_outage_validation_duration: i64) -> Vec<GatewayOutage> {
    let current_time = OffsetDateTime::from(SystemTime::now());
    let primitive_time = PrimitiveDateTime::new(current_time.date(), current_time.time());
    let scheduled_outages = ETGO::getPotentialGwOutages(primitive_time).await;
    let validated_outages = scheduled_outages
        .into_iter()
        .filter(|outage| validate_scheduled_outage(scheduled_outage_validation_duration, outage.clone()))
        .collect();
    // log_debug_v("scheduled Outages length", &scheduled_outages.len());
    // if validated_outages.len() != scheduled_outages.len() {
    //     log_debug_v(
    //         "scheduled Outages filtered",
    //         (validated_outages.len(), scheduled_outages.len()),
    //     );
    // }
    validated_outages
}

fn validate_scheduled_outage(
    scheduled_outage_validation_duration: i64,
    scheduled_outage: ETGO::GatewayOutage,
) -> bool {
    check_duration(scheduled_outage.clone(), scheduled_outage_validation_duration)
        && check_pmt_outage(scheduled_outage)
}

fn check_duration(
    scheduled_outage: ETGO::GatewayOutage,
    scheduled_outage_validation_duration: i64,
) -> bool {
    let duration = scheduled_outage.endTime - scheduled_outage.startTime;
    duration.abs().as_seconds_f64() < scheduled_outage_validation_duration as f64
}

fn check_pmt_outage(scheduled_outage: ETGO::GatewayOutage) -> bool {
    match scheduled_outage.paymentMethodType {
        None => true,
        Some(ETP::payment_method::PaymentMethodType::UPI) => true,
        _ => scheduled_outage.gateway.is_some()
            || scheduled_outage.bank.is_some()
            || scheduled_outage.paymentMethod.is_some()
            || scheduled_outage.juspayBankCodeId.is_some()
            || scheduled_outage.metadata.is_some(),
    }
}

pub async fn get_global_gateway_score(
    redis_key: String,
    max_count: Option<i64>,
    score_threshold: Option<f64>,
) -> Option<(Vec<GlobalScoreLog>, f64)> {
    if let (Some(max_count), Some(score_threshold)) = (max_count, score_threshold) {
        let app_state = get_tenant_app_state().await;
        let m_value: Option<GlobalGatewayScore> = app_state.redis_conn.get_key(&redis_key, "global_gateway_score_key").await.unwrap_or(None);
        match m_value {
            None => None,
            Some(global_gateway_score) => {
                let sorted_filtered_merchants: Vec<GlobalScore> = global_gateway_score
                    .merchants
                    .iter()
                    .cloned()
                    .take(max_count as usize)
                    .collect::<Vec<_>>();
                let should_penalize = sorted_filtered_merchants.len() >= max_count as usize
                    && sorted_filtered_merchants
                        .iter()
                        .all(|x| x.score < score_threshold);
                let filtered_merchants: Vec<GlobalScoreLog> = sorted_filtered_merchants
                    .into_iter()
                    .map(|gs| mk_gsl(gs, score_threshold, max_count))
                    .collect();
                Some((
                    filtered_merchants,
                    if should_penalize {
                        score_threshold - 0.1
                    } else {
                        score_threshold
                    },
                ))
            }
        }
    } else {
        // log_warning_t(
        //     "get_global_gateway_score",
        //     format!(
        //         "max_count is {:?}, score_threshold is {:?}",
        //         max_count, score_threshold
        //     ),
        // );
        None
    }
}

fn mk_gsl(gs: GlobalScore, score_threshold: f64, max_count: i64) -> GlobalScoreLog {
    GlobalScoreLog {
        currentScore: Utils::round_off_to_3(gs.score),
        transactionCount: gs.transactionCount,
        merchantId: gs.merchantId,
        eliminationThreshold: Utils::round_off_to_3(score_threshold),
        eliminationMaxCountThreshold: max_count,
    }
}

pub fn get_gateway_wise_routing_inputs_for_global_sr(
    gateway: ETG::Gateway,
    merchant_wise_global_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    global_success_rate_based_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    global_routing_defaults: SRGlobalRoutingDefaults,
) -> GatewayWiseSuccessRateBasedRoutingInput {
    let global_gateway_wise_inputs = global_success_rate_based_routing_input.clone()
        .and_then(|input| input.globalGatewayWiseInputs)
        .unwrap_or_default();
    let merchant_gateway_wise_inputs = merchant_wise_global_routing_input.clone()
        .and_then(|input| input.globalGatewayWiseInputs)
        .unwrap_or_default();

    let get_gateway_threshold_input_given_by_global_config =
        |gw: &ETG::Gateway| global_gateway_wise_inputs.iter().find(|ri| ri.gateway == *gw);

    let get_merchant_gateway_threshold_input_given_by_global_config =
        |gw: &ETG::Gateway| merchant_gateway_wise_inputs.iter().find(|ri| ri.gateway == *gw);

    let mk_new_entry = |gw: ETG::Gateway| GatewayWiseSuccessRateBasedRoutingInput {
        gateway: gw,
        eliminationThreshold: global_routing_defaults.defaultGlobalEliminationThreshold,
        eliminationMaxCountThreshold: global_routing_defaults.defaultGlobalEliminationMaxCountThreshold,
        eliminationLevel: global_routing_defaults.defaultGlobalEliminationLevel,
        currentScore: None,
        selectionMaxCountThreshold: None,
        softTxnResetCount: None,
        gatewayLevelEliminationThreshold: None,
        lastResetTimeStamp: None,
    };

    let adjust_defs = |mut gri: GatewayWiseSuccessRateBasedRoutingInput| {
        gri.eliminationLevel = gri.eliminationLevel.or_else(|| {
            global_success_rate_based_routing_input
                .as_ref()
                .and_then(|input| input.defaultGlobalEliminationLevel.clone())
        }).or(Some(ETGRI::EliminationLevel::PAYMENT_METHOD));
        gri.eliminationMaxCountThreshold = gri
            .eliminationMaxCountThreshold
            .or(global_routing_defaults.defaultGlobalEliminationMaxCountThreshold);
        gri.eliminationThreshold = gri
            .eliminationThreshold
            .or(global_routing_defaults.defaultGlobalEliminationThreshold);
        gri
    };

    get_merchant_gateway_threshold_input_given_by_global_config(&gateway)
        .or_else(|| get_gateway_threshold_input_given_by_global_config(&gateway))
        .map(|gri| adjust_defs(gri.clone()))
        .unwrap_or_else(|| adjust_defs(mk_new_entry(gateway)))
}

pub async fn get_global_elimination_gateway_score(
    gateway_key_map: GatewayRedisKeyMap,
    gsri: GatewayWiseSuccessRateBasedRoutingInput,
) -> Option<(Vec<GlobalScoreLog>, f64)> {
    if gsri.eliminationLevel != Some(ETGRI::EliminationLevel::NONE) {
        let redis_key = gateway_key_map
            .get(&gsri.gateway.to_string())
            .cloned()
            .unwrap_or_default();
        get_global_gateway_score(
            redis_key,
            gsri.eliminationMaxCountThreshold,
            gsri.eliminationThreshold,
        ).await
    } else {
        None
    }
}

pub async fn update_gateway_score_based_on_global_success_rate(
    decider_flow: &mut DeciderFlow<'_>,
    merchant_wise_global_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    global_success_rate_based_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    gateway_scoring_data: GatewayScoringData,
) -> (GatewayScoreMap, Option<Vec<GatewayWiseSuccessRateBasedRoutingInput>>, bool) {
    let gateway_score = get_gwsm(decider_flow);
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let merchant_id = txn_detail.merchantId.clone();

    let (global_elimination_occurred, global_elimination_gateway_score_map) = match check_sr_global_routing_defaults(
        global_success_rate_based_routing_input.clone(),
        merchant_wise_global_routing_input.clone(),
    ) {
        Ok(global_routing_defaults) => {
            let gateway_success_rate_inputs = gateway_score.clone()
                .iter()
                .map(|(k, _)| {
                    get_gateway_wise_routing_inputs_for_global_sr(
                        k.clone(),
                        merchant_wise_global_routing_input.clone(),
                        global_success_rate_based_routing_input.clone(),
                        global_routing_defaults.clone(),
                    )
                })
                .collect::<Vec<_>>();

            let gateway_list = Utils::get_gateway_list(gateway_score.clone());
            let gateway_redis_key_map = Utils::get_consumer_key(
                decider_flow,
                gateway_scoring_data,
                ScoreKeyType::ELIMINATION_GLOBAL_KEY,
                false,
                gateway_list,
            ).await;

            let mut upd_gateway_success_rate_inputs = Vec::new();
            let mut global_gateway_scores = Vec::new();
            for gsri in gateway_success_rate_inputs {
                let global_elimination_gateway_score = get_global_elimination_gateway_score(gateway_redis_key_map.clone(), gsri.clone()).await;
                match global_elimination_gateway_score{
                    Some((global_gateway_score, s)) => {
                        let new_gsri = GatewayWiseSuccessRateBasedRoutingInput {
                            currentScore: Some(s),
                            ..gsri.clone()
                        };
                        upd_gateway_success_rate_inputs.push(new_gsri);
                        global_gateway_scores.push(update_global_score_log(gsri.gateway.clone(), global_gateway_score));
                    }
                    None => {}
                }
            }

            let filtered_gateway_success_rate_inputs: Vec<GatewayWiseSuccessRateBasedRoutingInput> = upd_gateway_success_rate_inputs
                .into_iter()
                .filter(|x| {
                    if let (Some(cs), Some(et)) = (x.currentScore, x.eliminationThreshold) {
                        cs < et
                    } else {
                        false
                    }
                })
                .collect();

            reset_metric_log_data(decider_flow);
            let init_metric_log_data = decider_flow.writer.srMetricLogData.clone();
            let before_gwsm = get_gwsm(decider_flow);
            set_metric_log_data(decider_flow, SRMetricLogData {
                gatewayBeforeEvaluation: Utils::get_max_score_gateway(&before_gwsm).map(|x| x.0),
                downtimeStatus: filtered_gateway_success_rate_inputs
                    .iter()
                    .map(|x| x.gateway.clone())
                    .collect(),
                ..init_metric_log_data.clone()
            });

            if !filtered_gateway_success_rate_inputs.is_empty() {
                let new_gateway_score = filtered_gateway_success_rate_inputs.iter().fold(
                    gateway_score.clone(),
                    |acc, x| penalize_gsr(txn_detail.txnId.clone(), acc, x.clone()),
                );
                set_gwsm(decider_flow, new_gateway_score.clone());
                let old_sr_metric_log_data = decider_flow.writer.srMetricLogData.clone();
                set_metric_log_data(decider_flow, SRMetricLogData {
                    gatewayAfterEvaluation: Utils::get_max_score_gateway(&new_gateway_score)
                        .map(|x| x.0),
                    ..old_sr_metric_log_data.clone()
                });
            } else {
                // log_info_t(
                //     "scoringFlow",
                //     format!(
                //         "No gateways are eligible for penalties & fallback {} based on global score",
                //         txn_detail.txn_id
                //     ),
                // );
                let old_sr_metric_log_data = decider_flow.writer.srMetricLogData.clone();
                set_metric_log_data(decider_flow, SRMetricLogData {
                    gatewayAfterEvaluation: Utils::get_max_score_gateway(&gateway_score)
                        .map(|x| x.0),
                    ..old_sr_metric_log_data.clone()
                });
            }

            let old_sr_metric_log_data = decider_flow.writer.srMetricLogData.clone();
            // log_debug_v("MetricData-GLOBAL-ELIMINATION", old_sr_metric_log_data.clone());

            let global_elimination_occurred = old_sr_metric_log_data
                .gatewayBeforeEvaluation
                .is_some()
                && old_sr_metric_log_data.gatewayBeforeEvaluation
                    != old_sr_metric_log_data.gatewayAfterEvaluation;

            if !global_gateway_scores.is_empty() {
                set_metric_log_data(decider_flow, SRMetricLogData {
                    merchantGatewayScore: Some(serde_json::json!(global_gateway_scores)),
                    ..old_sr_metric_log_data.clone()
                });
                Utils::metric_tracker_log(
                    "GLOBAL_SR_EVALUATION",
                    "GW_SCORING",
                    Utils::get_metric_log_format(decider_flow, "GLOBAL_SR_EVALUATION"),
                ).await;
            } else {
                // log_info_v(
                //     "scoringFlow",
                //     format!(
                //         "Global scores not available for {} {}",
                //         merchant_id, txn_detail.txn_id
                //     ),
                // );
            }

            // log_info_v(
            //     "scoringFlow",
            //     format!(
            //         "Gateway scores after considering global SR based elimination for {} : {}",
            //         txn_detail.txn_id, global_gateway_scores
            //     ),
            // );

            (global_elimination_occurred, Some(filtered_gateway_success_rate_inputs))
        }
        Err(reason) => {
            // log_debug_t("Global SR routing", reason.clone());
            // log_info_t(
            //     "scoringFlow",
            //     format!(
            //         "Global SR routing not enabled for merchant {} txn {}",
            //         ETM::to_text(&merchant_id),
            //         txn_detail.txn_id
            //     ),
            // );
            (false, None)
        }
    };

    let sm = return_sm_with_log(decider_flow, DeciderScoringName::ScoringByGatewayScoreBasedOnGlobalSuccessRate, false);
    (sm, global_elimination_gateway_score_map, global_elimination_occurred)
}

pub fn update_global_score_log(
    gateway: ETG::Gateway,
    score_list: Vec<GlobalScoreLog>,
) -> Vec<GlobalSREvaluationScoreLog> {
    score_list.into_iter().fold(vec![], |mut blank_score_log, global_score_list| {
        blank_score_log.push(update_global_score(gateway.clone(), global_score_list));
        blank_score_log
    })
}

pub fn update_global_score(
    gateway: ETG::Gateway,
    list: GlobalScoreLog,
) -> GlobalSREvaluationScoreLog {
    GlobalSREvaluationScoreLog {
        transactionCount: list.transactionCount,
        currentScore: list.currentScore,
        merchantId: list.merchantId,
        eliminationThreshold: list.eliminationThreshold,
        eliminationMaxCountThreshold: list.eliminationMaxCountThreshold,
        gateway,
    }
}

pub fn penalize_gsr(
    txn_id: TransactionId,
    gs: GatewayScoreMap,
    sri: GatewayWiseSuccessRateBasedRoutingInput,
) -> GatewayScoreMap {
    let mut new_gs = gs.clone();
    new_gs.entry(sri.gateway.clone()).and_modify(|v| *v /= 5.0);
    // log_info_t(
    //     "scoringFlow",
    //     format!(
    //         "Penalizing gateway {:?} for {:?} based on global score",
    //         sri.gateway,
    //         review(ETTD::transaction_id_text, txn_id)
    //     ),
    // )
    // .await;
    new_gs
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SRGlobalRoutingDefaults {
    pub defaultGlobalEliminationThreshold: Option<f64>,
    pub defaultGlobalEliminationMaxCountThreshold: Option<i64>,
    pub defaultGlobalEliminationLevel: Option<EliminationLevel>,
}

pub fn check_sr_global_routing_defaults(
    global_input: Option<GatewaySuccessRateBasedRoutingInput>,
    merchant_input: Option<GatewaySuccessRateBasedRoutingInput>,
) -> Result<SRGlobalRoutingDefaults, String> {
    match (global_input, merchant_input) {
        (Some(global), Some(merchant)) => {
            if (global_elim_lvl_not_none(&global) && global_elim_lvl_not_none(&merchant))
                || is_forced_pm(&merchant)
            {
                Ok(SRGlobalRoutingDefaults {
                    defaultGlobalEliminationThreshold: merchant.defaultGlobalEliminationThreshold.or(global.defaultGlobalEliminationThreshold),
                    defaultGlobalEliminationMaxCountThreshold: merchant.defaultGlobalEliminationMaxCountThreshold.or(global.defaultGlobalEliminationMaxCountThreshold),
                    defaultGlobalEliminationLevel: merchant.defaultGlobalEliminationLevel.or(global.defaultGlobalEliminationLevel),
                })
            } else {
                Err(format!(
                    "Global and merchant inputs are present, global defaultGlobalEliminationLevel = {:?}, merchant defaultGlobalEliminationLevel = {:?}",
                    global.defaultGlobalEliminationLevel, merchant.defaultGlobalEliminationLevel
                ))
            }
        }
        (Some(global), None) => {
            if global_elim_lvl_not_none(&global) {
                Ok(SRGlobalRoutingDefaults {
                    defaultGlobalEliminationThreshold: global.defaultGlobalEliminationThreshold,
                    defaultGlobalEliminationMaxCountThreshold: global.defaultGlobalEliminationMaxCountThreshold,
                    defaultGlobalEliminationLevel: global.defaultGlobalEliminationLevel,
                })
            } else {
                Err(format!(
                    "Only global input is present, global defaultGlobalEliminationLevel = {:?}",
                    global.defaultGlobalEliminationLevel
                ))
            }
        }
        (None, Some(merchant)) => {
            if is_forced_pm(&merchant) {
                Ok(SRGlobalRoutingDefaults {
                    defaultGlobalEliminationThreshold: merchant.defaultGlobalEliminationThreshold,
                    defaultGlobalEliminationMaxCountThreshold: merchant.defaultGlobalEliminationMaxCountThreshold,
                    defaultGlobalEliminationLevel: merchant.defaultGlobalEliminationLevel,
                })
            } else {
                Err(format!(
                    "Only merchant input is present, merchant defaultGlobalEliminationLevel = {:?}",
                    merchant.defaultGlobalEliminationLevel
                ))
            }
        }
        (None, None) => Err("SR routing inputs not present.".to_string()),
    }
}

pub fn is_forced_pm(v: &GatewaySuccessRateBasedRoutingInput) -> bool {
    v.defaultGlobalEliminationLevel == Some(EliminationLevel::FORCED_PAYMENT_METHOD)
}

pub fn global_elim_lvl_not_none(v: &GatewaySuccessRateBasedRoutingInput) -> bool {
    v.defaultGlobalEliminationLevel != Some(EliminationLevel::NONE)
}

pub async fn get_gateway_wise_routing_inputs_for_merchant_sr(
    merchant_acc: ETM::merchant_account::MerchantAccount,
    txn_detail: ETTD::TxnDetail,
    txn_card_info: ETCT::txn_card_info::TxnCardInfo,
    gateway: ETG::Gateway,
    gateway_success_rate_merchant_input: Option<GatewaySuccessRateBasedRoutingInput>,
    default_success_rate_based_routing_input: Option<GatewaySuccessRateBasedRoutingInput>,
) -> GatewayWiseSuccessRateBasedRoutingInput {
    let m_option = RService::findByNameFromRedis(C::SR_BASED_GATEWAY_ELIMINATION_THRESHOLD.get_key()).await;
    let default_soft_txn_reset_count = RService::findByNameFromRedis(C::srBasedTxnResetCount.get_key())
        .await
        .unwrap_or(C::gwDefaultTxnSoftResetCount);
    let is_elimination_v2_enabled = isFeatureEnabled(
        C::ENABLE_ELIMINATION_V2.get_key(),
        merchant_acc.merchantId.0.clone(),
        "kv_redis".to_string(),
    ).await;

    let default_elimination_threshold = m_option.unwrap_or(C::defaultSrBasedGatewayEliminationThreshold);
    let merchant_given_default_threshold = gateway_success_rate_merchant_input.clone().map(|input| input.defaultEliminationThreshold);
    let merchant_given_default_gateway_sr_threshold = gateway_success_rate_merchant_input.clone().map(|input| input.defaultGatewayLevelEliminationThreshold);
    let merchant_given_default_elimination_level = gateway_success_rate_merchant_input.clone().map(|input| input.defaultEliminationLevel);
    let merchant_given_default_soft_txn_reset_count = gateway_success_rate_merchant_input.clone().map(|input| input.defaultGlobalSoftTxnResetCount);

    let default_merchant_elimination_threshold = default_success_rate_based_routing_input.clone().map(|input| input.defaultEliminationThreshold);
    let default_gateway_level_sr_elimination_threshold = default_success_rate_based_routing_input.clone().map(|input| input.defaultGatewayLevelEliminationThreshold);
    let default_merchant_elimination_level = default_success_rate_based_routing_input.clone().map(|input| input.defaultEliminationLevel);
    let default_merchant_soft_txn_reset_count = default_success_rate_based_routing_input.map(|input| input.defaultGlobalSoftTxnResetCount);

    let gateway_wise_inputs_list = gateway_success_rate_merchant_input.map(|input| input.gatewayWiseInputs.unwrap_or_default()).unwrap_or_default();

    let elimination_threshold = merchant_given_default_threshold.unwrap_or(default_merchant_elimination_threshold.unwrap_or(default_elimination_threshold));

    let elimination_threshold_updated = if is_elimination_v2_enabled {
        get_elimination_v2_threshold(&merchant_acc, &txn_card_info, &txn_detail).await.unwrap_or(elimination_threshold)
    } else {
        elimination_threshold
    };

    gateway_wise_inputs_list
        .iter().find(|ri| ri.gateway == gateway)
        .map(|e| GatewayWiseSuccessRateBasedRoutingInput {
            eliminationLevel: e.eliminationLevel.clone().or(merchant_given_default_elimination_level.clone()).or(Some(EliminationLevel::GATEWAY)),
            ..e.clone()
        })
        .unwrap_or(GatewayWiseSuccessRateBasedRoutingInput{
            gateway,
            eliminationThreshold: Some(elimination_threshold_updated),
            selectionMaxCountThreshold: None,
            eliminationMaxCountThreshold: None,
            softTxnResetCount: merchant_given_default_soft_txn_reset_count.unwrap_or(default_merchant_soft_txn_reset_count.unwrap_or(Some(default_soft_txn_reset_count))),
            gatewayLevelEliminationThreshold: merchant_given_default_gateway_sr_threshold.unwrap_or(default_gateway_level_sr_elimination_threshold.unwrap_or(Some(C::defSRBasedGwLevelEliminationThreshold))),
            eliminationLevel: merchant_given_default_elimination_level.or(default_merchant_elimination_level).or(Some(EliminationLevel::PAYMENT_METHOD)),
            currentScore: None,
            lastResetTimeStamp: None,
        })
}

async fn get_elimination_v2_threshold(
    merchant_acc: &ETM::merchant_account::MerchantAccount,
    txn_card_info: &ETCT::txn_card_info::TxnCardInfo,
    txn_detail: &ETTD::TxnDetail,
) -> Option<f64> {
    let m_gateway_success_rate_merchant_input = Utils::decode_and_log_error(
        "Gateway Decider Input Decode Error",
        &merchant_acc.gatewaySuccessRateBasedDeciderInput.clone(),
    );

    // let sr1_th_weight_env = Env::JuspayEnv {
    //     key: C::THRESHOLD_WEIGHT_SR1,
    //     action_left: Env::mk_default_env_action(0.29),
    //     decrypt_func: Box::new(|x| async { x }),
    //     log_when_throw_exception: None,
    // };
    // let sr1_th_weight = Env::lookup_env(sr1_th_weight_env).await;
    let sr1_th_weight = 0.29;
    let sr2_th_weight = 0.71;

    // let sr2_th_weight_env = Env::JuspayEnv {
    //     key: C::THRESHOLD_WEIGHT_SR2,
    //     action_left: Env::mk_default_env_action(0.71),
    //     decrypt_func: Box::new(|x| async { x }),
    //     log_when_throw_exception: None,
    // };
    // let sr2_th_weight = Env::lookup_env(sr2_th_weight_env).await;

    if let Some((sr1, sr2, n, m_pmt, m_pm, m_txn_object_type, source)) =
        get_sr1_and_sr2_and_n(m_gateway_success_rate_merchant_input, merchant_acc.merchantId.0.clone(), txn_card_info.clone(), txn_detail.clone()).await
    {
        // log_info_t(
        //     "CALCULATING_THRESHOLD:SR1_SR2_N_PMT_PM_TXNOBJECTTYPE_CONFIGSOURCE",
        //     format!(
        //         "{} {} {} {} {} {} {}",
        //         sr1,
        //         sr2,
        //         n,
        //         m_pmt.unwrap_or_else(|| "Nothing".to_string()),
        //         m_pm.unwrap_or_else(|| "Nothing".to_string()),
        //         m_txn_object_type.unwrap_or_else(|| "Nothing".to_string()),
        //         source
        //     ),
        // )
        // .await;

        // log_info_t(
        //     "THRESHOLD_VALUE",
        //     format!("{}", ((sr1_th_weight * sr1) + (sr2_th_weight * sr2)) / 100.0),
        // )
        // .await;

        Some(((sr1_th_weight * sr1) + (sr2_th_weight * sr2)) / 100.0)
    } else {
        // log_info_t(
        //     "ELIMINATION_V2_VALUES_NOT_FOUND:THRESHOLD:PMT_PM_TXNOBJECTTYPE_SOUCREOBJECT",
        //     format!(
        //         "{} {} {} {}",
        //         txn_card_info.payment_method_type,
        //         txn_card_info.payment_method,
        //         txn_detail.txn_object_type,
        //         txn_detail.source_object.unwrap_or_else(|| "Nothing".to_string())
        //     ),
        // )
        // .await;

        None
    }
}

pub async fn get_sr1_and_sr2_and_n(
    m_gateway_success_rate_merchant_input: Option<GatewaySuccessRateBasedRoutingInput>,
    merchant_id: String,
    txn_card_info: ETCT::txn_card_info::TxnCardInfo,
    txn_detail: ETTD::TxnDetail,
) -> Option<(
    f64,
    f64,
    f64,
    Option<String>,
    Option<String>,
    Option<String>,
    ConfigSource,
)> {
    if let Some(gateway_success_rate_merchant_input) = m_gateway_success_rate_merchant_input {
        if let Some(inputs) = gateway_success_rate_merchant_input.eliminationV2SuccessRateInputs {
            let pmt = txn_card_info.paymentMethodType.to_text();
            let source_obj = if txn_card_info.paymentMethod == "UPI" {
                txn_detail.sourceObject.clone()
            } else {
                Some(txn_card_info.paymentMethod.clone())
            };
            let pm =
                if txn_card_info.paymentMethodType == ETP::payment_method::PaymentMethodType::UPI {
                    source_obj.clone()
                } else {
                    Some(txn_card_info.paymentMethod.clone())
                };
            let txn_obj_type = txn_detail.txnObjectType.to_string();

            filter_using_service_config(merchant_id, pmt.to_string(), pm, txn_obj_type, inputs)
                .await
        } else {
            None
            // fetch_default_sr1_and_sr2_and_n(&gateway_success_rate_merchant_input).await
        }
    } else {
        None
    }
}

// async fn fetch_default_sr1_and_sr2_and_n(
//     gateway_success_rate_merchant_input: &GatewayWiseSuccessRateBasedRoutingInput,
// ) -> Option<(f64, f64, f64, Option<String>, Option<String>, Option<String>, ConfigSource)> {
//     if let Some(sr2) = gateway_success_rate_merchant_input.default_elimination_v2_success_rate {
//         fetch_default_sr1_and_n_and_mk_result(sr2).await
//     } else {
//         None
//     }
// }

// async fn fetch_default_sr1_and_n_and_mk_result(sr2: f64) -> Option<(f64, f64, f64, Option<String>, Option<String>, Option<String>, ConfigSource)> {
//     let m_default_sr1 = RC::r_hget(Config::EC_REDIS, construct_sr1_key(merchant_id), C::DEFAULT_FIELD_NAME_FOR_SR1_AND_N).await;
//     let m_default_n = RC::r_hget(Config::EC_REDIS, construct_n_key(merchant_id), C::DEFAULT_FIELD_NAME_FOR_SR1_AND_N).await;

//     if let (Some(sr1), Some(n)) = (m_default_sr1, m_default_n) {
//         Some((sr1, sr2, n, None, None, None, ConfigSource::MERCHANT_DEFAULT))
//     } else {
//         let m_s_config_sr1 = RService::find_by_name_from_redis(C::DEFAULT_SR1_S_CONFIG_PREFIX(merchant_id)).await;
//         let m_s_config_n = RService::find_by_name_from_redis(C::DEFAULT_N_S_CONFIG_PREFIX(merchant_id)).await;

//         if let (Some(sr1), Some(n)) = (m_s_config_sr1, m_s_config_n) {
//             Some((sr1, sr2, n, None, None, None, ConfigSource::GLOBAL_DEFAULT))
//         } else {
//             None
//         }
//     }
// }

async fn filter_using_service_config(
    merchant_id: String,
    pmt: String,
    pm: Option<String>,
    txn_obj_type: String,
    inputs: Vec<EliminationSuccessRateInput>,
) -> Option<(
    f64,
    f64,
    f64,
    Option<String>,
    Option<String>,
    Option<String>,
    ConfigSource,
)> {
    let m_configs = RService::findByNameFromRedis(
        C::internalDefaultEliminationV2SuccessRate1AndNPrefix(merchant_id.clone()).get_key(),
    )
    .await;
    let configs = m_configs.unwrap_or_else(Vec::new);

    fetch_sr1_and_n_from_service_config_upto(
        FilterLevel::TXN_OBJECT_TYPE,
        merchant_id.clone(),
        pmt.clone(),
        pm.clone(),
        txn_obj_type.clone(),
        inputs.clone(),
        configs.clone(),
    )
    .or_else(|| {
        fetch_sr1_and_n_from_service_config_upto(
            FilterLevel::PAYMENT_METHOD,
            merchant_id.clone(),
            pmt.clone(),
            pm.clone(),
            txn_obj_type.clone(),
            inputs.clone(),
            configs.clone(),
        )
    })
    .or_else(|| {
        fetch_sr1_and_n_from_service_config_upto(
            FilterLevel::PAYMENT_METHOD_TYPE,
            merchant_id,
            pmt,
            pm,
            txn_obj_type,
            inputs,
            configs,
        )
    })
}

pub fn filter_inputs_upto(
    level: FilterLevel,
    pmt: String,
    pm: Option<String>,
    txn_obj_type: String,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    match level {
        FilterLevel::TXN_OBJECT_TYPE => {
            filter_inputs_upto_txn_object_type(pmt, pm, txn_obj_type, inputs)
        }
        FilterLevel::PAYMENT_METHOD => filter_inputs_upto_payment_method(pmt, pm, inputs),
        FilterLevel::PAYMENT_METHOD_TYPE => filter_inputs_upto_payment_method_type(pmt, inputs),
    }
}

// pub async fn filter_using_redis_upto(
//     level: FilterLevel,
//     merchant_id: T,
//     pmt: T,
//     pm: Option<T>,
//     txn_obj_type: T,
//     inputs: Vec<ETGRI::EliminationSuccessRateInput>,
// ) -> Option<(f64, f64, f64, Option<T>, Option<T>, Option<T>, ConfigSource)> {
//     let m_input = filter_inputs_upto(level, pmt.clone(), pm.clone(), txn_obj_type.clone(), inputs);
//     let m_sr1_and_n = get_sr1_and_n_from_redis_upto(level, merchant_id.clone(), pmt.clone(), pm.clone(), txn_obj_type.clone()).await;
//     match (m_input, m_sr1_and_n) {
//         (Some(input), Some((sr1, n))) => Some((
//             sr1,
//             input.success_rate,
//             n,
//             Some(input.payment_method_type),
//             input.payment_method.clone(),
//             input.txn_object_type.clone(),
//             ConfigSource::Redis,
//         )),
//         _ => None,
//     }
// }

// pub async fn get_sr1_and_n_from_redis_upto(
//     level: FilterLevel,
//     merchant_id: T,
//     pmt: T,
//     m_pm: Option<T>,
//     txn_obj_type: T,
// ) -> Option<(f64, f64)> {
//     let sr1_key = construct_sr1_key(&merchant_id);
//     let n_key = construct_n_key(&merchant_id);
//     let dim_key = construct_dimension_key(level, &pmt, m_pm.as_ref(), &txn_obj_type);

//     let redis_sr1 = fetch_from_redis(&sr1_key, &dim_key).await;
//     let redis_n = fetch_from_redis(&n_key, &dim_key).await;

//     match (redis_sr1, redis_n) {
//         (Some(sr1), Some(n)) => Some((sr1, n)),
//         _ => None,
//     }
// }

// fn construct_sr1_key(merchant_id: &T) -> T {
//     format!("{}{}", C::SR1_KEY_PREFIX, merchant_id)
// }

// fn construct_n_key(merchant_id: &T) -> T {
//     format!("{}{}", C::N_KEY_PREFIX, merchant_id)
// }

// fn construct_dimension_key(
//     level: FilterLevel,
//     pmt: &T,
//     pm: Option<&T>,
//     txn_obj_type: &T,
// ) -> Option<T> {
//     match level {
//         FilterLevel::TxnObjectType => pm.map(|pm| format!("{}|{}|{}", pmt, pm, txn_obj_type)),
//         FilterLevel::PaymentMethod => pm.map(|pm| format!("{}|{}", pmt, pm)),
//         FilterLevel::PaymentMethodType => Some(pmt.clone()),
//     }
// }

// async fn fetch_from_redis(key: &T, dim_key: &Option<T>) -> Option<f64> {
//     match dim_key {
//         None => None,
//         Some(dkey) => RC::r_hget(Config::EC_REDIS, key, dkey).await,
//     }
// }

pub fn fetch_sr1_and_n_from_service_config_upto(
    level: FilterLevel,
    merchant_id: String,
    pmt: String,
    pm: Option<String>,
    txn_object_type: String,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
    configs: Vec<SuccessRate1AndNConfig>,
) -> Option<(f64, f64, f64, Option<T>, Option<T>, Option<T>, ConfigSource)> {
    let m_input = filter_inputs_upto(
        level.clone(),
        pmt.clone(),
        pm.clone(),
        txn_object_type.clone(),
        inputs,
    );
    let m_config = match level {
        FilterLevel::TXN_OBJECT_TYPE => {
            filter_configs_upto_txn_object_type(&pmt, pm.as_ref(), &txn_object_type, &configs)
        }
        FilterLevel::PAYMENT_METHOD => {
            filter_configs_upto_payment_method(&pmt, pm.as_ref(), &configs)
        }
        FilterLevel::PAYMENT_METHOD_TYPE => filter_configs_upto_payment_method_type(&pmt, &configs),
    };

    match (m_input, m_config) {
        (Some(input), Some(config)) => Some((
            config.successRate,
            input.successRate,
            config.nValue,
            Some(input.paymentMethodType),
            input.paymentMethod.clone(),
            input.txnObjectType.clone(),
            ConfigSource::SERVICE_CONFIG,
        )),
        _ => None,
    }
}

fn filter_configs_upto_txn_object_type(
    pmt: &String,
    pm: Option<&String>,
    txn_object_type: &String,
    configs: &[SuccessRate1AndNConfig],
) -> Option<SuccessRate1AndNConfig> {
    pm.and_then(|pm| {
        configs
            .iter()
            .find(|x| {
                x.paymentMethodType == *pmt
                    && x.paymentMethod.as_ref() == Some(pm)
                    && x.txnObjectType.as_ref() == Some(txn_object_type)
            })
            .cloned()
    })
}

fn filter_configs_upto_payment_method(
    pmt: &String,
    pm: Option<&String>,
    configs: &[SuccessRate1AndNConfig],
) -> Option<SuccessRate1AndNConfig> {
    pm.and_then(|pm| {
        configs
            .iter()
            .find(|x| {
                x.paymentMethodType == *pmt
                    && x.paymentMethod.as_ref() == Some(pm)
                    && x.txnObjectType.is_none()
            })
            .cloned()
    })
}

fn filter_configs_upto_payment_method_type(
    pmt: &String,
    configs: &[SuccessRate1AndNConfig],
) -> Option<SuccessRate1AndNConfig> {
    configs
        .iter()
        .find(|x| {
            x.paymentMethodType == *pmt && x.paymentMethod.is_none() && x.txnObjectType.is_none()
        })
        .cloned()
}

fn filter_inputs_upto_txn_object_type(
    pmt: String,
    pm: Option<String>,
    txn_obj_type: String,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    pm.and_then(|pm| {
        inputs.into_iter().find(|x| {
            x.paymentMethodType == pmt
                && x.paymentMethod.as_ref() == Some(&pm)
                && x.txnObjectType.as_ref() == Some(&txn_obj_type)
        })
    })
}

fn filter_inputs_upto_payment_method(
    pmt: String,
    pm: Option<String>,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    pm.and_then(|pm| {
        inputs.into_iter().find(|x| {
            x.paymentMethodType == pmt
                && x.paymentMethod.as_ref() == Some(&pm)
                && x.txnObjectType.is_none()
        })
    })
}

fn filter_inputs_upto_payment_method_type(
    pmt: String,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    inputs.into_iter().find(|x| {
        x.paymentMethodType == pmt && x.paymentMethod.is_none() && x.txnObjectType.is_none()
    })
}

pub async fn get_success_rate_routing_inputs(
    merchant_acc: ETM::merchant_account::MerchantAccount,
) -> (
    Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
) {
    let redis_input =
        findByNameFromRedis(C::DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT.get_key()).await;
    let decoded_input = Utils::decode_and_log_error(
        "Gateway Decider Input Decode Error",
        &merchant_acc.gatewaySuccessRateBasedDeciderInput,
    );
    (redis_input, decoded_input)
}

// pub async fn evaluate_and_trigger_reset(
//     gateway_wise_success_rate_inputs: Vec<GatewayWiseSuccessRateBasedRoutingInput>,
// ) -> DeciderFlow<()> {
//     let txn_detail = DeciderFlow::get_txn_detail().await;
//     let reset_gateway_list = evaluate_reset_gateway_score(&gateway_wise_success_rate_inputs, &txn_detail).await;

//     if M::is_feature_enabled(
//         C::GW_RESET_SCORE_ENABLED,
//         &Utils::get_m_id(&txn_detail.merchant_id),
//         Config::KV_REDIS,
//     ).await {
//         trigger_reset_gateway_score(
//             &gateway_wise_success_rate_inputs,
//             &txn_detail,
//             reset_gateway_list,
//             true,
//         ).await;
//     }
// }

pub async fn update_gateway_score_based_on_success_rate(
    decider_flow: &mut DeciderFlow<'_>,
    is_sr_metric_enabled: bool,
    initial_gw_scores: GatewayScoreMap,
    gateway_scoring_data: GatewayScoringData,
    elimination_enabled: Option<bool>,
) -> GatewayScoreMap {
    let merchant_acc = decider_flow.get().dpMerchantAccount.clone();
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let txn_card_info = decider_flow.get().dpTxnCardInfo.clone();
    let enable_success_rate_based_gateway_elimination = isPaymentFlowEnabledWithHierarchyCheck(merchant_acc.id.clone(), merchant_acc.tenantAccountId.clone(), ModuleName::MERCHANT_CONFIG, PaymentFlow::ELIMINATION_BASED_ROUTING, 
        crate::types::country::country_iso::text_db_to_country_iso(merchant_acc.country.as_deref().unwrap_or_default()).ok()).await || elimination_enabled == Some(true);

    // log_debug_t(
    //     "updateGatewayScoreBasedOnSuccessRate",
    //     format!(
    //         "enableSuccessRateBasedGatewayElimination = {:?} for merchant {}",
    //         enable_success_rate_based_gateway_elimination,
    //         ETM::to_text(&merchant_acc.merchant_id)
    //     ),
    // );

    if enable_success_rate_based_gateway_elimination {
        let (default_success_rate_based_routing_input, gateway_success_rate_merchant_input) =
            get_success_rate_routing_inputs(merchant_acc.clone()).await;

        let is_reset_score_enabled_for_merchant = isFeatureEnabled(
            C::GATEWAY_RESET_SCORE_ENABLED.get_key(), 
            Utils::get_m_id(txn_detail.merchantId.clone()),
            "kv_redis".to_string(),
        ).await;

        let payment_method_type = if Utils::is_card_transaction(&txn_card_info) {
            ETP::payment_method::PaymentMethodType::Card
        } else {
            txn_card_info.paymentMethodType.clone()
        };

        let enabled_payment_method_types = gateway_success_rate_merchant_input.clone()
            .map(|input| input.enabledPaymentMethodTypes.clone())
            .unwrap_or_default();

        if !enabled_payment_method_types.is_empty()
            && !enabled_payment_method_types.contains(&payment_method_type)
        {
            // log_info_v(
            //     "scoringFlow",
            //     format!(
            //         "Transaction {} with payment method types {:?} not enabled by {} for SR based routing",
            //         review(ETTD::transaction_id_text(), &txn_detail.txn_id),
            //         payment_method_type,
            //         ETM::to_text(&merchant_acc.merchant_id)
            //     ),
            // );
        } else {
            let (
                gateway_score_global_sr,
                global_elimination_gateway_score_map,
                global_elimination_occurred,
            ) = update_gateway_score_based_on_global_success_rate(
                decider_flow,
                gateway_success_rate_merchant_input.clone(),
                default_success_rate_based_routing_input.clone(),
                gateway_scoring_data.clone(),
            ).await;

            // log_info_v(
            //     "scoringFlow",
            //     format!(
            //         "Gateway scores input for merchant wise SR based evaluation for {} : {:?}",
            //         review(ETTD::transaction_id_text(), &txn_detail.txn_id),
            //         to_list_of_gateway_score(&gateway_score_global_sr),
            //     ),
            // );

            let sr_based_elimination_approach_info = if global_elimination_occurred {
                vec!["GLOBAL".to_string()]
            } else {
                vec![]
            };

            let mut gateway_success_rate_inputs = vec![];
            for (gw, _) in gateway_score_global_sr.clone() {
                gateway_success_rate_inputs.push(get_gateway_wise_routing_inputs_for_merchant_sr(
                    merchant_acc.clone(),
                    txn_detail.clone(),
                    txn_card_info.clone(),
                    gw,
                    gateway_success_rate_merchant_input.clone(),
                    default_success_rate_based_routing_input.clone(),
                ).await);
            }

            if !gateway_success_rate_inputs.is_empty() {
                let gateway_list = Utils::get_gateway_list(gateway_score_global_sr.clone());
                let gateway_redis_key_map = Utils::get_consumer_key(
                    decider_flow,
                    gateway_scoring_data.clone(),
                    ScoreKeyType::ELIMINATION_MERCHANT_KEY,
                    false,
                    gateway_list.clone(),
                ).await;

                let mut gateway_success_rate_inputs_with_updated_score = Vec::new();
                for input in gateway_success_rate_inputs.clone() {
                    gateway_success_rate_inputs_with_updated_score.push(update_current_score(
                        decider_flow,
                        gateway_redis_key_map.clone(),
                        input.clone(),
                    ).await);
                }

                let filtered_gateway_success_rate_inputs: Vec<_> = gateway_success_rate_inputs_with_updated_score.clone()
                    .into_iter()
                    .filter(|input| {
                        input
                            .currentScore
                            .zip(input.eliminationThreshold)
                            .map(|(cs, et)| cs < et)
                            .unwrap_or(false)
                    })
                    .collect();

                reset_metric_log_data(decider_flow);
                let init_metric_log_data = decider_flow.writer.srMetricLogData.clone();
                let before_gwsm = get_gwsm(decider_flow);
                set_metric_log_data(decider_flow, SRMetricLogData {
                    gatewayBeforeEvaluation: Utils::get_max_score_gateway(&before_gwsm).map(|(gw, _)| gw),
                    downtimeStatus: filtered_gateway_success_rate_inputs.iter().map(|input| input.gateway.clone()).collect(),
                    ..init_metric_log_data.clone()
                });

                if !filtered_gateway_success_rate_inputs.is_empty() {
                    let new_sm = filtered_gateway_success_rate_inputs.iter().fold(
                        gateway_score_global_sr.clone(),
                        |acc, input| update_score_with_log(txn_detail.txnId.clone(), acc, input.clone()),
                    );

                    set_gwsm(decider_flow, new_sm.clone());
                    set_metric_log_data(decider_flow, SRMetricLogData {
                        gatewayAfterEvaluation: Utils::get_max_score_gateway(&new_sm).map(|(gw, _)| gw),
                        ..init_metric_log_data.clone()
                    });

                    if is_reset_score_enabled_for_merchant {
                        // let reset_enabled_gateway_list =
                        //     evaluate_reset_gateway_score(&filtered_gateway_success_rate_inputs, &txn_detail);
                        let reset_enabled_gateway_list: Vec<ETG::Gateway> = vec![];

                        if !reset_enabled_gateway_list.is_empty() {
                            decider_flow.writer.resetGatewayList = reset_enabled_gateway_list.clone();
                            decider_flow.writer.resetGatewayList.dedup();
                        }
                    }
                } else {
                    set_metric_log_data(decider_flow, SRMetricLogData {
                        gatewayAfterEvaluation: Utils::get_max_score_gateway(&before_gwsm).map(|(gw, _)| gw),
                        ..init_metric_log_data.clone()
                    });

                    // log_info_v(
                    //     "scoringFlow",
                    //     format!(
                    //         "No gateways are eligible for penalties & fallback : {}",
                    //         txn_detail.txn_id
                    //     ),
                    // );
                }

                let old_sr_metric_log_data = decider_flow.writer.srMetricLogData.clone();
                let sr_based_elimination_approach_info = if old_sr_metric_log_data.clone()
                    .gatewayBeforeEvaluation
                    .zip(old_sr_metric_log_data.gatewayAfterEvaluation.clone())
                    .map(|(before, after)| before != after)
                    .unwrap_or(false)
                {
                    vec!["MERCHANT".to_string()]
                        .into_iter()
                        .chain(sr_based_elimination_approach_info.into_iter())
                        .collect()
                } else {
                    sr_based_elimination_approach_info
                };

                set_metric_log_data(
                    decider_flow,
                    SRMetricLogData {
                        merchantGatewayScore: Some(serde_json::json!(gateway_success_rate_inputs_with_updated_score.clone()
                            .iter()
                            .map(transform_gateway_wise_success_rate_based_routing)
                            .collect::<Vec<DeciderGatewayWiseSuccessRateBasedRoutingInput>>()
                        )),
                        ..old_sr_metric_log_data.clone()
                    },
                );

                Utils::metric_tracker_log(
                    "SR_EVALUATION",
                    "GW_SCORING",
                    Utils::get_metric_log_format(decider_flow, "SR_EVALUATION"),
                ).await;

                // log_debug_v(
                //     "MetricData-MERCHANT_PMT_PM",
                //     format!("{:?}", old_sr_metric_log_data),
                // );

                let new_gateway_score = get_gwsm(decider_flow);
                // let merchant_enabled_for_unification = Redis::is_feature_enabled(
                //     C::merchants_enabled_for_score_keys_unification(),
                //     Utils::get_m_id(&txn_detail.merchant_id),
                //     Config::kv_redis(),
                // );

                // let (new_gateway_score, reset_gateway_level_list, gateway_level_sr_elimination) =
                //     if merchant_enabled_for_unification {
                //         (new_gateway_score.clone(), vec![], false);
                    // } else {
                    //     update_gateway_score_based_on_gateway_level_scores(
                    //         &gateway_success_rate_inputs,
                    //         new_gateway_score.clone(),
                    //         is_reset_score_enabled_for_merchant,
                    //     )
                    // };

                // if !reset_gateway_level_list.is_empty() {
                //     modify(|ctx| {
                //         ctx.reset_gateway_list = DL::nub(
                //             ctx.reset_gateway_list
                //                 .clone()
                //                 .into_iter()
                //                 .chain(reset_gateway_level_list.into_iter())
                //                 .collect(),
                //         );
                //     });
                // }

                // let sr_based_elimination_approach_info = if gateway_level_sr_elimination {
                //     vec!["GATEWAY".to_string()]
                //         .into_iter()
                //         .chain(sr_based_elimination_approach_info.into_iter())
                //         .collect()
                // } else {
                //     sr_based_elim ination_approach_info
                // };

                let reset_gw_list = decider_flow.writer.resetGatewayList.clone();
                trigger_reset_gateway_score(
                    decider_flow,
                    gateway_success_rate_inputs,
                    txn_detail.clone(),
                    reset_gw_list,
                    is_reset_score_enabled_for_merchant,
                );

                let gateway_decider_approach = get_decider_approach(decider_flow);
                let (gw_score, downtime, sr_based_elimination_approach_info_res) =
                    if filtered_gateway_success_rate_inputs.len() > 1
                        && new_gateway_score.len() == filtered_gateway_success_rate_inputs.len()
                    {
                        let optimization_during_downtime_enabled = isFeatureEnabled(
                            C::ENABLE_OPTIMIZATION_DURING_DOWNTIME.get_key(),
                            Utils::get_m_id(txn_detail.merchantId.clone()),
                            "kv_redis".to_string(),
                        ).await;

                        if optimization_during_downtime_enabled {
                            if is_sr_metric_enabled {
                                // log_info_v(
                                //     "scoringFlow",
                                //     format!(
                                //         "Overriding priority with SR Scores during downtime for {} : {:?}",
                                //         review(ETTD::transaction_id_text(), &txn_detail.txn_id),
                                //         new_gateway_score,
                                //     ),
                                // );

                                (new_gateway_score.clone(), DownTime::ALL_DOWNTIME, vec![])
                            } else {
                                // log_info_v(
                                //     "scoringFlow",
                                //     format!(
                                //         "Overriding priority with PL during downtime for {} : {:?}",
                                //         review(ETTD::transaction_id_text(), &txn_detail.txn_id),
                                //         initial_gw_scores,
                                //     ),
                                // );

                                (initial_gw_scores.clone(), DownTime::ALL_DOWNTIME, vec![])
                            }
                        } else {
                            // log_info_t(
                            //     "scoringFlow",
                            //     format!(
                            //         "Overriding priority with SR Scores during downtime is not enabled for {}",
                            //         Utils::get_m_id(&txn_detail.merchant_id),
                            //     ),
                            // );

                            (new_gateway_score.clone(), DownTime::ALL_DOWNTIME, sr_based_elimination_approach_info)
                        }
                    } else if !global_elimination_gateway_score_map.unwrap_or_default().is_empty() {
                        (
                            new_gateway_score.clone(),
                            DownTime::GLOBAL_DOWNTIME,
                            sr_based_elimination_approach_info,
                        )
                    } else if !filtered_gateway_success_rate_inputs.is_empty() {
                        (
                            new_gateway_score.clone(),
                            DownTime::DOWNTIME,
                            sr_based_elimination_approach_info,
                        )
                    } else {
                        (
                            new_gateway_score.clone(),
                            DownTime::NO_DOWNTIME,
                            sr_based_elimination_approach_info,
                        )
                    };

                let gateway_decider_approach =
                    Utils::modify_gateway_decider_approach(gateway_decider_approach, downtime);

                set_gwsm(decider_flow, gw_score.clone());
                Utils::set_elimination_scores(decider_flow, toListOfGatewayScore(gw_score));
                set_decider_approach(decider_flow, gateway_decider_approach);
                set_sr_elimination_approach_info(decider_flow, sr_based_elimination_approach_info_res);

                // log_info_v(
                //     "routing_approach",
                //     format!("{:?}", gateway_decider_approach),
                // );
            }
        }
    }

    let gateway_score_sr_based = get_gwsm(decider_flow);
    // log_info_v(
    //     "GW_Scoring",
    //     format!(
    //         "Gateway scores after considering SR based elimination for {} : {:?}",
    //         review(ETTD::transaction_id_text(), &txn_detail.txn_id),
    //         to_list_of_gateway_score(&gateway_score_sr_based),
    //     ),
    // );

    return_sm_with_log(decider_flow, DeciderScoringName::UpdateGatewayScoreBasedOnSuccessRate, enable_success_rate_based_gateway_elimination)
}

pub fn update_score_with_log(
    txn_id: TransactionId,
    m: GatewayScoreMap,
    v: ETGRI::GatewayWiseSuccessRateBasedRoutingInput,
) -> GatewayScoreMap {
    let new_m = m
        .iter()
        .filter_map(|(gw, score)| {
            if *gw == v.gateway {
                let new_score = *score / 5_f64;
                // log_info_v::<String>(
                //     "scoringFlow",
                //     format!(
                //         "Penalizing gateway {} for {} with penalty {} : {} -> {}",
                //         gw, txn_id, v.penalty, score, new_score
                //     ),
                // );
                Some((gw.clone(), new_score))
            } else {
                Some((gw.clone(), *score))
            }
        })
        .collect();
    new_m
}

pub async fn get_merchant_elimination_gateway_score(i: RedisKey) -> Option<ETGRI::GatewayScore> {
    let app_state = get_tenant_app_state().await;
    app_state
        .redis_conn
        .get_key::<ETGRI::GatewayScore>(&i, "elimination_score_key")
        .await.ok()
}

pub async fn update_current_score(
    decider_flow: &DeciderFlow<'_>,
    gateway_redis_key_map: GatewayRedisKeyMap,
    i: ETGRI::GatewayWiseSuccessRateBasedRoutingInput,
) -> ETGRI::GatewayWiseSuccessRateBasedRoutingInput {
    let redis_key = gateway_redis_key_map
        .get(&format!("{:?}", i.gateway))
        .unwrap_or(&String::new())
        .to_string();
    let txn_detail = decider_flow.get().dpTxnDetail.clone();
    let m_score = get_merchant_elimination_gateway_score(redis_key).await;
    // log_info_t(
    //     "scoringFlow",
    //     format!(
    //         "Current score for {} {} : {:?} with elimination level {} threshold {}",
    //         review::<String>(ETTD::transaction_id_text(), txn_detail.txn_id),
    //         i.gateway,
    //         m_score.as_ref().map(|score| score.score),
    //         i.elimination_level,
    //         i.elimination_threshold
    //     ),
    // );
    let updated_input = ETGRI::GatewayWiseSuccessRateBasedRoutingInput {
        currentScore: m_score.as_ref().map(|score| score.score),
        lastResetTimeStamp: m_score.as_ref().map(|score| score.lastResetTimestamp),
        ..i
    };
    updated_input
}

pub fn log_final_gateways_scoring(decider_flow: &mut DeciderFlow<'_>) -> GatewayScoreMap {
    return_sm_with_log(decider_flow, DeciderScoringName::FinalScoring, false)
}

pub fn merchantGatewayScoreDimension(
    routingInput: GatewayWiseSuccessRateBasedRoutingInput,
) -> Dimension {
    match routingInput.eliminationLevel {
        Some(EliminationLevel::PAYMENT_METHOD_TYPE) => Dimension::SECOND,
        Some(EliminationLevel::PAYMENT_METHOD) => Dimension::THIRD,
        _ => Dimension::FIRST,
    }
}

pub async fn getKeyTTLFromMerchantDimension(dimension: Dimension) -> f64 {
    let mTtl: Option<f64> = match dimension {
        Dimension::FIRST => {
            RService::findByNameFromRedis(C::gwScoreFirstDimensionTtl.get_key()).await
        }
        Dimension::SECOND => {
            RService::findByNameFromRedis(C::gwScoreSecondDimensionTtl.get_key()).await
        }
        Dimension::THIRD => {
            RService::findByNameFromRedis(C::gwScoreThirdDimensionTtl.get_key()).await
        }
        Dimension::FOURTH => {
            RService::findByNameFromRedis(C::gwScoreFourthDimensionTtl.get_key()).await
        }
    };

    mTtl.unwrap_or(C::defScoreKeysTtl)
}

pub async fn evaluate_reset_gateway_score(
    filteredGatewaySuccessRateInputs: Vec<GatewayWiseSuccessRateBasedRoutingInput>,
    txnDetail: ETTD::TxnDetail,
) -> Vec<ETG::Gateway> {
    // log_debug!(
    //     "evaluateResetGatewayScore",
    //     format!(
    //         "Evaluating Reset Logic for Gateways for {}",
    //         txnDetail.txnId
    //     )
    // );

    let current_time: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    let mut acc: Vec<ETG::Gateway> = Vec::new();

    for it in filteredGatewaySuccessRateInputs {
        let key_ttl = getKeyTTLFromMerchantDimension(merchantGatewayScoreDimension(it.clone())).await;
        if let Some(last_reset_time) = it.lastResetTimeStamp {
            if (current_time * 1000 - last_reset_time as i64) > key_ttl.round() as i64 {
                // log_debug!(
                //     "evaluateResetGatewayScore",
                //     format!(
                //         "Adding gateway {} to resetAPI Request for {} for level {:?}",
                //         it.gateway, txnDetail.txnId, it.eliminationLevel
                //     )
                // );
                acc.push(it.gateway.clone());
            }
        }
    }

    acc
}

pub fn trigger_reset_gateway_score(
    decider_flow: &mut DeciderFlow<'_>,
    gateway_success_rate_inputs: Vec<GatewayWiseSuccessRateBasedRoutingInput>,
    txn_detail: ETTD::TxnDetail,
    reset_gateway_list: Vec<ETG::Gateway>,
    is_reset_score_enabled_for_merchant: bool,
) {
    // log_info_t("scoringFlow", format!("Triggering Reset for Gateways for {:?}", reset_gateway_list));
    if is_reset_score_enabled_for_merchant {
        // log_info_v::<String>(
        //     "scoringFlow",
        //     format!(
        //         "Reset Gateway Scores is enabled for {:?} and merchantId {:?}",
        //         txn_detail.txn_id,
        //         Utils::get_m_id(txn_detail.merchant_id)
        //     ),
        // );
        let reset_gateway_sr_list = reset_gateway_list.iter().fold(Vec::new(), |mut acc, it| {
            // log_info_v::<String>(
            //     "scoringFlow",
            //     format!(
            //         "Adding gateway {:?} to resetAPI Request for {:?}",
            //         it, txn_detail.txn_id
            //     ),
            // );
            let m_sr_input = get_gateway_success_rate_input(it, &gateway_success_rate_inputs);
            let oref = decider_flow.get().dpOrder.clone();
            let macc = decider_flow.get().dpMerchantAccount.clone();
            let (meta, pl_ref_id_map) =
                Utils::get_order_metadata_and_pl_ref_id_map(decider_flow, macc.enableGatewayReferenceIdBasedRouting, &oref);
            match m_sr_input {
                Some(sr_input) => {
                    let gw_ref_id = Utils::get_gateway_reference_id(meta, it, oref, pl_ref_id_map);
                    let reset_gateway_input = ResetGatewayInput {
                        gateway: it.clone(),
                        eliminationThreshold: sr_input.eliminationThreshold,
                        eliminationMaxCount: sr_input.softTxnResetCount.map(|v| v as i64),
                        gatewayEliminationThreshold: sr_input.gatewayLevelEliminationThreshold,
                        gatewayReferenceId: gw_ref_id.map(|id| id.mga_reference_id),
                    };
                    acc.push(reset_gateway_input);
                }
                None => {
                    // log_info_v::<String>(
                    //     "scoringFlow",
                    //     format!("No SR Input for {:?} and {:?}", it, txn_detail.txn_id),
                    // );
                }
            }
            acc
        });

        let reset_approach = Utils::get_reset_approach(decider_flow);
        match reset_approach {
            ResetApproach::SRV2_RESET => Utils::set_reset_approach(decider_flow, ResetApproach::SRV2_ELIMINATION_RESET),
            ResetApproach::SRV3_RESET => Utils::set_reset_approach(decider_flow, ResetApproach::SRV3_ELIMINATION_RESET),
            _ => Utils::set_reset_approach(decider_flow, ResetApproach::ELIMINATION_RESET),
        }
        // log_info_v::<String>("RESET_APPROACH", format!("{:?}", reset_approach));
        // log_info_v::<String>(
        //     "scoringFlow",
        //     format!(
        //         "Reset Gateway List for {:?} is {:?}",
        //         txn_detail.txn_id, reset_gateway_sr_list
        //     ),
        // );
        // reset_gateway_score(txn_detail, reset_gateway_sr_list);
    } else {
        // log_info_v::<String>(
        //     "scoringFlow",
        //     format!(
        //         "Reset Gateway Scores is not enabled for {:?} and merchantId {:?}",
        //         txn_detail.txn_id,
        //         Utils::get_m_id(txn_detail.merchant_id)
        //     ),
        // );
    }
}

fn get_gateway_success_rate_input(
    gw: &ETG::Gateway,
    gateway_success_rate_inputs: &[GatewayWiseSuccessRateBasedRoutingInput],
) -> Option<GatewayWiseSuccessRateBasedRoutingInput> {
    gateway_success_rate_inputs.iter().find(|it| it.gateway == *gw).cloned()
}

// pub fn reset_gateway_score(
//     txn_detail: ETTD::TxnDetail,
//     reset_gateway_sr_list: Vec<ResetGatewayInput>,
// ) -> DeciderFlow<()> {
//     if !reset_gateway_sr_list.is_empty() {
//         let endpoint = ENV::euler_endpoint();
//         let params = ResetCallParams {
//             txn_detail_id: txn_detail.id.to_string(),
//             txn_id: review(ETTD::transaction_id_text(), txn_detail.txn_id.clone()),
//             merchant_id: Utils::get_m_id(txn_detail.merchant_id.clone()),
//             order_id: Ord::un_order_id(txn_detail.order_id.clone()),
//             reset_gateway_score_req_arr: reset_gateway_sr_list,
//         };
//         log_debug_v::<String>(
//             "resetGatewayScore",
//             format!(
//                 "Reset score call to Euler with Endpoint: {:?}, params: {:?}, for {:?}",
//                 endpoint, params, txn_detail.txn_id
//             ),
//         );
//         let url = Client::parse_base_url(endpoint.as_str()).unwrap();
//         let m_cell_selector = language::get_option_local::<Options::XCellSelectorHeader>();
//         language::call_api(
//             Some(T::ManagerSelector::TlsManager),
//             url,
//             EC_RESET_GATEWAY_SCORE,
//             |_| None,
//             reset_gw_score_call(Some("HS".to_string()), m_cell_selector, params),
//         );
//     } else {
//         log_debug_v::<String>(
//             "resetGatewayScore",
//             format!("Not eligible to send reset gateway score callback {:?}", txn_detail.txn_id),
//         );
//     }
// }

// fn reset_gw_score_call(param1: Option<Text>, param2: Option<Text>, params: ResetCallParams) -> T::EulerClient<A::Value> {
//     T::client::<ResetGWScoreAPI>(param1, param2, params)
// }

pub fn route_random_traffic(
    decider_flow: &mut DeciderFlow<'_>,
    gws: GatewayScoreMap,
    hedging_percent: f64,
    is_sr_v3_metric_enabled: bool,
    tag: String,
) -> GatewayScoreMap {
    let num = generate_random_number(
        format!("GatewayDecider::routeRandomTraffic::{}", tag),
        (0.0, 100.0),
    );
    // language::log_debug_t("RandomNumber", format!("{:?}", num));
    let mut sorted_gw_list: Vec<_> = gws.iter().collect();
    sorted_gw_list.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    let (head_gateway, remaining_gateways) = sorted_gw_list.split_at(1);
    if num < hedging_percent * (remaining_gateways.len() as f64) {
        let remaining_gateways: Vec<_> = remaining_gateways
            .iter()
            .map(|(gw, _)| (gw.clone(), 1.0))
            .collect();
        let head_gateways: Vec<_> = head_gateway
            .iter()
            .map(|(gw, _)| (gw.clone(), 0.5))
            .collect();
        // language::log_debug_t(
        //     "Gateway Scores After Route Random Traffic Feature",
        //     format!("{:?}", remaining_gateways.iter().chain(head_gateways.iter()).collect::<Vec<_>>()),
        // );
        if is_sr_v3_metric_enabled {
            set_decider_approach(decider_flow, GatewayDeciderApproach::SR_V3_HEDGING);
        } else {
            set_decider_approach(decider_flow, GatewayDeciderApproach::SR_V3_HEDGING);
        }
        remaining_gateways
            .into_iter()
            .map(|(gw, score)| (gw.clone(), score))
            .chain(
                head_gateways
                    .into_iter()
                    .map(|(gw, score)| (gw.clone(), score)),
            )
            .collect()
    } else {
        // language::log_debug_t("Selection Based Routing Gateways SR", format!("{:?}", sorted_gw_list));
        sorted_gw_list
            .into_iter()
            .map(|(gw, score)| (gw.clone(), *score))
            .collect()
    }
}
