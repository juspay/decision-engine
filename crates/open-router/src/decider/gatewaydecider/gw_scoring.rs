use eulerhs::prelude::*;
use optics::core::{review, Field1};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use gateway_decider::types::{GatewayScoreMap, GatewayDeciderApproach, SRMetricLogData, DeciderScoringName};
use types::gateway_routing_input::{GatewayWiseSuccessRateBasedRoutingInput, SelectionLevel};
use types::card_brand_routes as ETCBR;
use types::gateway_routing_input as ETGRI;
use types::gateway as ETG;
use types::gateway_health as ETGH;
use types::gateway_outage as ETGO;
use types::payment as ETP;
use types::card as ETCT;
use types::issuer_routes as ETIssuerR;
use types::merchant as ETM;
use types::order as Ord;
use types::juspay_bank_code as ETJ;
use types::txn_detail as ETTD;
use types::tenant_config as TenantConfig;
use configs::env_vars as ENV;
use utils::redis as Redis;
use utils::redis::cache as RService;
use utils::config::merchant_config as MerchantConfig;
use utils::api_tag::*;
use utils::wai::middleware::options as Options;
use eulerhs::art::v2::types::ArtRecordable;
use gateway_decider::constants as C;
use gateway_decider::utils as Utils;
use juspay::extra::secret::unsafe_extract_secret;
use juspay::extra::list as EList;
use juspay::extra::env as Env;
use servant::client as Client;
use eulerhs::types as T;
use eulerhs::api_helpers as T;
use eulerhs::language as L;
use eulerhs::tenant_redis_layer as RC;
use data_random::rvar::run_rvar;
use data_random::distribution::binomial::binomial;
use data_random::distribution::beta::beta;
use system_random::stateful::{init_std_gen, new_io_gen_m, IOGenM};
use system_random::internal::StdGen;
use std::collections::{HashMap as MP, HashSet as ST};
use std::string::String as T;
use std::vec::Vec;
use std::option::Option;
use std::char as Char;
use std::iter::Iterator;
use std::fmt::Debug;

#[derive(Debug, ArtRecordable)]
pub struct IOGenMStdGen;

pub fn get_gwsm() -> DeciderFlow<GatewayScoreMap> {
    DeciderFlow::gets(|st| st.gwScoreMap.clone())
}

pub fn set_gwsm(gwsm: GatewayScoreMap) -> DeciderFlow<()> {
    DeciderFlow::modify(|st| {
        st.gwScoreMap = gwsm;
    })
}

pub fn get_decider_approach() -> DeciderFlow<GatewayDeciderApproach> {
    DeciderFlow::gets(|st| st.gwDeciderApproach.clone())
}

pub fn set_decider_approach(approach: GatewayDeciderApproach) -> DeciderFlow<()> {
    DeciderFlow::modify(|st| {
        st.gwDeciderApproach = approach;
    })
}

pub fn set_is_scheduled_outage(is_scheduled_outage: bool) -> DeciderFlow<()> {
    DeciderFlow::modify(|st| {
        st.isScheduledOutage = is_scheduled_outage;
    })
}

pub fn get_sr_elimination_approach_info() -> DeciderFlow<Vec<T>> {
    DeciderFlow::gets(|st| st.srElminiationApproachInfo.clone())
}

pub fn set_sr_elimination_approach_info(approach: Vec<T>) -> DeciderFlow<()> {
    DeciderFlow::modify(|st| {
        st.srElminiationApproachInfo = approach;
    })
}

pub fn set_metric_log_data(log_data: SRMetricLogData) -> DeciderFlow<()> {
    DeciderFlow::modify(|st| {
        st.srMetricLogData = log_data;
    })
}

pub fn reset_metric_log_data() -> DeciderFlow<()> {
    DeciderFlow::do_with(|env, st| {
        let txn_detail = env.dpTxnDetail.clone();
        let gateway_before_downtime_evaluation = st.topGatewayBeforeSRDowntimeEvaluation.clone();
        let txn_creation_time = txn_detail.dateCreated.replace(" ", "T").replace(" UTC", "Z");
        st.srMetricLogData = SRMetricLogData {
            gatewayAfterEvaluation: None,
            gatewayBeforeEvaluation: None,
            merchantGatewayScore: None,
            downtimeStatus: vec![],
            dateCreated: txn_creation_time,
            gatewayBeforeDowntimeEvaluation: gateway_before_downtime_evaluation,
        };
    })
}

pub fn return_sm_with_log(
    s_name: DeciderScoringName,
    do_or_not: bool,
) -> DeciderFlow<GatewayScoreMap> {
    DeciderFlow::do_with(|env, st| {
        let sr = st.gwScoreMap.clone();
        let txn_id = env.dpTxnDetail.txnId.clone();
        log_debug!(
            "GW_Scoring",
            format!(
                "Gateway scores after {} for {} : {:?}",
                s_name,
                txn_id,
                sr.to_list_of_gateway_score()
            )
        );
        if do_or_not {
            st.debugScoringList.push(DebugScoringEntry {
                scoringName: s_name.to_string().make_first_letter_small(),
                gatewayScores: sr.to_list_of_gateway_score(),
            });
        }
        sr
    })
}

trait MakeFirstLetterSmall {
    fn make_first_letter_small(&self) -> String;
}

impl MakeFirstLetterSmall for String {
    fn make_first_letter_small(&self) -> String {
        if let Some((first, rest)) = self.split_first() {
            format!("{}{}", first.to_lowercase(), rest)
        } else {
            self.clone()
        }
    }
}



pub fn scoring_flow(
    functional_gateways: Vec<ETG::Gateway>,
    gateway_priority_list: Vec<ETG::Gateway>,
) -> DeciderFlow<GatewayScoreMap> {
    let merchant = asks(|ctx| ctx.dp_merchant_account);
    let txn_detail = asks(|ctx| ctx.dp_txn_detail);
    let txn_card_info = asks(|ctx| ctx.dp_txn_card_info);

    GF::set_gws(functional_gateways.clone());

    let gateway_scoring_data = Utils::get_gateway_scoring_data(txn_detail.clone(), txn_card_info.clone(), merchant.clone());

    if functional_gateways.len() == 1 {
        set_gwsm(
            functional_gateways
                .iter()
                .fold(GatewayScoreMap::new(), |acc, gateway| create_score_map(acc, gateway)),
        );
        set_decider_approach(DEFAULT);
        Utils::set_top_gateway_before_sr_downtime_evaluation(functional_gateways.first().cloned());
        let current_gateway_score_map = get_gwsm();
        update_gateway_score_based_on_success_rate(false, current_gateway_score_map, gateway_scoring_data);
        log_info_t(
            "scoringFlow",
            format!(
                "Intelligent routing not triggered due to 1 gateway eligible for merchant {} and for txn Id {}",
                Utils::get_m_id(&merchant.merchant_id),
                review(ETTD::transaction_id_text, txn_detail.txn_id.clone())
            ),
        );
    } else {
        let pmt = asks(|ctx| ctx.dp_txn_card_info.payment_method_type);
        let pm = asks(|ctx| ctx.dp_txn_card_info.payment_method);
        let maybe_source_object = asks(|ctx| ctx.dp_txn_detail.source_object);

        let pmt_str = pmt.to_string();
        let pm_str = Utils::get_payment_method(&pmt_str, &pm, maybe_source_object.unwrap_or_default());

        let is_merchant_enabled_for_sr_based_routing = MerchantConfig::is_merchant_enabled_for_payment_flows(
            merchant.id.clone(),
            Utils::get_m_id(&merchant.merchant_id),
            vec!["SR_BASED_ROUTING".to_string()],
            Redis::Enforce,
            C::GATEWAYDECIDER_SCORINGFLOW,
        );

        let is_merchant_enabled_globally = match is_merchant_enabled_for_sr_based_routing {
            Some(false) => false,
            _ => true,
        };

        let is_sr_v3_metric_enabled = if is_merchant_enabled_globally {
            let is_sr_v3_metric_enabled = M::is_feature_enabled(
                C::enable_gateway_selection_based_on_sr_v3_input(&pmt_str),
                Utils::get_m_id(&merchant.merchant_id),
                Config::kv_redis(),
            );

            if is_sr_v3_metric_enabled {
                log_info_t(
                    "scoringFlow",
                    format!(
                        "Deciding Gateway based on SR V3 Routing for merchant {} and for txn Id {}",
                        Utils::get_m_id(&merchant.merchant_id),
                        review(ETTD::transaction_id_text, txn_detail.txn_id.clone())
                    ),
                );

                let merchant_sr_v3_input_config = RService::find_by_name_from_redis(
                    C::sr_v3_input_config(Utils::get_m_id(&merchant.merchant_id)),
                );
                let default_sr_v3_input_config = RService::find_by_name_from_redis(C::sr_v3_default_input_config());

                log_info_v(
                    "scoringFlow_Sr_V3_Input_Config",
                    format!(
                        "Sr V3 Input Config {:?}",
                        merchant_sr_v3_input_config
                    ),
                );
                log_info_v(
                    "scoringFlow_Sr_V3_Default_Input_Config",
                    format!(
                        "Sr V3 Default Input Config {:?}",
                        default_sr_v3_input_config
                    ),
                );

                let hedging_percent = Utils::get_sr_v3_hedging_percent(
                    merchant_sr_v3_input_config.clone(),
                    &pmt_str,
                    &pm_str,
                )
                .or_else(|| Utils::get_sr_v3_hedging_percent(default_sr_v3_input_config.clone(), &pmt_str, &pm_str))
                .unwrap_or(C::default_sr_v3_based_hedging_percent());

                Utils::set_sr_v3_hedging_percent(hedging_percent);

                let is_explore_and_exploit_enabled = M::is_feature_enabled(
                    C::enable_explore_and_exploit_on_sr_v3(&pmt_str),
                    Utils::get_m_id(&merchant.merchant_id),
                    Config::kv_redis(),
                );

                let should_explore = if is_explore_and_exploit_enabled {
                    Utils::route_random_traffic_to_explore(
                        hedging_percent,
                        functional_gateways.clone(),
                        "SR_BASED_V3_ROUTING",
                    )
                } else {
                    false
                };

                let initial_sr_gw_scores = if should_explore {
                    functional_gateways
                        .iter()
                        .fold(GatewayScoreMap::new(), |acc, gateway| create_score_map(acc, gateway))
                } else {
                    get_cached_scores_based_on_sr_v3(
                        merchant_sr_v3_input_config,
                        default_sr_v3_input_config,
                        &pm_str,
                        gateway_scoring_data.clone(),
                    )
                };

                let initial_sr_gw_scores_list = to_list_of_gateway_score(&initial_sr_gw_scores);

                log_info_v(
                    "scoringFlow",
                    format!(
                        "Gateway Scores based on SR V3 Routing for txn id : {} is {:?}",
                        review(ETTD::transaction_id_text, txn_detail.txn_id.clone()),
                        initial_sr_gw_scores_list
                    ),
                );

                if !initial_sr_gw_scores.is_empty() {
                    Utils::set_sr_gateway_scores(initial_sr_gw_scores_list);

                    log_info_t(
                        "scoringFlow",
                        format!(
                            "Considering Gateway Scores based on SR V3 for txn id : {}",
                            review(ETTD::transaction_id_text, txn_detail.txn_id.clone())
                        ),
                    );

                    if should_explore {
                        set_decider_approach(SR_V3_HEDGING);
                    } else {
                        set_decider_approach(SR_SELECTION_V3_ROUTING);
                    }

                    let is_route_random_traffic_enabled = M::is_feature_enabled(
                        C::route_random_traffic_sr_v3_enabled_merchant(),
                        Utils::get_m_id(&merchant.merchant_id),
                        Config::kv_redis(),
                    );

                    let sr_gw_score = if is_route_random_traffic_enabled && !is_explore_and_exploit_enabled {
                        route_random_traffic(
                            initial_sr_gw_scores.clone(),
                            hedging_percent,
                            true,
                            "SR_BASED_V3_ROUTING".to_string(),
                        )
                    } else {
                        initial_sr_gw_scores.clone()
                    };

                    set_gwsm(sr_gw_score.clone());
                    return_sm_with_log(GetCachedScoresBasedOnSrV3, true);

                    if sr_gw_score.len() > 1 && (!is_explore_and_exploit_enabled || should_explore) {
                        let is_debug_mode_enabled = M::is_feature_enabled(
                            C::enable_debug_mode_on_sr_v3(),
                            Utils::get_m_id(&merchant.merchant_id),
                            Config::kv_redis(),
                        );

                        Utils::add_txn_to_hash_map_if_debug_mode(
                            is_debug_mode_enabled,
                            Utils::get_m_id(&merchant.merchant_id),
                            txn_detail.clone(),
                        );
                    }

                    return true;
                } else {
                    log_info_t(
                        "scoringFlow",
                        format!(
                            "Gateway Scores based on SR V3 for txn id : {} and for merchant : {} is null, So falling back to priorityLogic",
                            review(ETTD::transaction_id_text, txn_detail.txn_id.clone()),
                            Utils::get_m_id(&merchant.merchant_id)
                        ),
                    );

                    return_sm_with_log(GetCachedScoresBasedOnSrV3, true);
                    return false;
                }
            } else {
                false
            }
        } else {
            false
        };

        Utils::set_is_sr_v3_metric_enabled(is_sr_v3_metric_enabled);

        // Additional logic for SR Metric and Priority Logic omitted for brevity
    }
}


pub fn get_cached_scores_based_on_success_rate(
    gateway_scoring_data: GatewayScoringData,
) -> DeciderFlow<GatewayScoreMap> {
    let merchant = asks(|ctx| ctx.dp_merchant_account);
    let order_ref = asks(|ctx| ctx.dp_order);
    let txn_detail = asks(|ctx| ctx.dp_txn_detail);
    let functional_gateways = GF::get_gws();
    let mut st = functional_gateways.iter().fold(
        MP::new(),
        |acc, gw| create_score_map(acc, gw.clone()),
    );

    log_debug_v::<Text>("scoringFlow my scoring flow functionalGateways", &functional_gateways);
    log_debug_v::<Text>("scoringFlow my scoring flow score", &to_list_of_gateway_score(&st));

    let gateway_success_rate_merchant_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput> =
        Utils::decode_and_log_error(
            "Gateway Decider Input Decode Error",
            &BSL::from_strict(&TE::encode_utf8(&merchant.gateway_success_rate_based_decider_input)),
        );

    let optimization_based_routing_input = gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| Some(input.default_selection_level.as_ref()))
        .unwrap_or(&SelectionLevel::SL_PAYMENT_METHOD);

    let gri_sr_v2_cutover = gateway_scoring_data.is_gri_enabled_for_sr_routing;

    if gri_sr_v2_cutover {
        let (meta, pl_ref_id_map) =
            Utils::get_order_metadata_and_pl_ref_id_map(
                merchant.enable_gateway_reference_id_based_routing,
                order_ref,
            );

        let gateway_redis_key_map =
            Utils::get_consumer_key(&gateway_scoring_data, SR_V2_KEY, false, &functional_gateways);

        let key_value_pair_list = gateway_redis_key_map
            .iter()
            .map(|(gw, redis_key)| {
                (
                    gw.clone(),
                    redis_key.clone(),
                    RC::r_get(Config::kv_redis(), redis_key),
                )
            })
            .collect::<Vec<_>>();

        log_info_v::<Text>("keyValuePairList", &key_value_pair_list);

        let gateway_score_maps = key_value_pair_list.iter().fold(
            MP::new(),
            |acc, (gw, redis_key, obj)| match obj {
                Some(orbd) => {
                    let gateway = Utils::text_to_gateway(gw);
                    let gateway_score_map =
                        evaluate_and_create_score_map(gateway, orbd, &st);
                    Utils::log_sr_stale(
                        orbd,
                        Utils::get_m_id(&merchant.merchant_id),
                        redis_key,
                        &gateway_score_map,
                    );
                    MP::union(&gateway_score_map, &acc)
                }
                None => acc,
            },
        );

        log_debug_v::<Text>(
            "Gateway Score Map After SR Evaluation",
            &to_list_of_gateway_score(&gateway_score_maps),
        );

        reset_and_log_metrics(&gateway_score_maps, "SR_SELECTION_V2_EVALUATION".to_string());
        return gateway_score_maps;
    } else {
        let gateway_redis_key =
            Utils::get_consumer_key(&gateway_scoring_data, SR_V2_KEY, false, &functional_gateways);

        let (_, sr_redis_key) = gateway_redis_key.iter().next().unwrap();

        log_info_t("Optimization Based Consumer Key", sr_redis_key);

        let obj: Option<OptimizationRedisBlockData> =
            RC::r_get(Config::kv_redis(), sr_redis_key);

        match obj {
            Some(orbd) => {
                let vol_check_data = if M::is_feature_enabled(
                    C::is_merchant_enabled_for_volume_check,
                    Utils::get_m_id(&merchant.merchant_id),
                    Config::kv_redis(),
                ) {
                    Some(
                        RService::find_by_name_from_redis(
                            C::selection_bucket_txn_volume_threshold,
                        )
                        .unwrap_or(C::default_selection_bucket_txn_volume_threshold),
                    )
                } else {
                    None
                };

                let num_blocks = orbd.aggregate.len();

                let weights = if M::is_feature_enabled(
                    C::is_weighted_sr_evaluation_enabled_merchant,
                    Utils::get_m_id(&merchant.merchant_id),
                    Config::kv_redis(),
                ) {
                    let fetch_weightage_from_config: Option<Vec<Weights>> =
                        RService::find_by_name_from_redis(
                            C::selection_weights_factor_for_weighted_sr_evaluation,
                        );

                    let weight_arr = fetch_weightage_from_config
                        .unwrap_or_else(|| C::default_weights_factor_for_weighted_sr_evaluation)
                        .iter()
                        .map(|Weights(i, t)| (t.clone(), i.clone()))
                        .collect::<Vec<_>>();

                    if M::is_feature_enabled(
                        C::is_performing_experiment,
                        Utils::get_m_id(&merchant.merchant_id),
                        Config::kv_redis(),
                    ) {
                        Utils::set_is_experiment_tag(
                            &Utils::get_experiment_tag(
                                txn_detail.date_created,
                                "WEIGHTED_SR",
                            ),
                        );
                    }

                    let (_, final_weights) = (0..num_blocks).fold(
                        (1.0, Vec::new()),
                        |(prev_weight, acc), i| {
                            let curr_weight =
                                Utils::round_off_to_3(Utils::compute_block_weights(
                                    &weight_arr,
                                    i,
                                    prev_weight,
                                ));
                            (curr_weight, vec![curr_weight, acc])
                        },
                    );

                    final_weights
                } else {
                    vec![1.0; num_blocks]
                };

                log_info_t("SR_Weighted_Array", &weights);

                let weighted_blocks = weights
                    .iter()
                    .zip(orbd.aggregate.iter())
                    .map(|(w, b)| (w.clone(), b.clone()))
                    .collect::<Vec<_>>();

                let gateway_success_rates = weighted_blocks.iter().fold(
                    MP::new(),
                    |acc, (weight, gw_score_details)| {
                        gw_score_details.transactions_detail.iter().fold(
                            acc,
                            |acc, gw_details| {
                                process_data(&functional_gateways, Some(weight.clone()), &mut acc, gw_details)
                            },
                        )
                    },
                );

                let (update_functional_gateways, is_reset_done) = gateway_success_rates.iter().fold(
                    (MP::new(), false),
                    |(acc, is_reset_done), (gw_name, sr_data)| {
                        process_gw_sr(vol_check_data, orbd, (acc, is_reset_done), gw_name, sr_data)
                    },
                );

                let update_functional_gateways = update_functional_gateways.iter().fold(
                    update_functional_gateways.clone(),
                    |acc, (gw, _)| process_upd_functional_gws(acc, gw.clone(), 1.0),
                );

                reset_and_log_metrics(&update_functional_gateways, "SR_SELECTION_V2_EVALUATION".to_string());
                Utils::log_sr_stale(
                    orbd,
                    Utils::get_m_id(&merchant.merchant_id),
                    sr_redis_key,
                    &update_functional_gateways,
                );

                if is_reset_done {
                    Utils::set_reset_approach(SRV2_RESET);
                }

                return update_functional_gateways;
            }
            None => {
                reset_and_log_metrics(&st, "SR_SELECTION_V2_EVALUATION".to_string());
                log_debug_t("Optimization Redis Block not found", sr_redis_key);
                return st;
            }
        }
    }
}


pub fn get_cached_scores_based_on_srv3(
    merchant_srv3_input_config: Option<SrV3InputConfig>,
    default_srv3_input_config: Option<SrV3InputConfig>,
    pm: String,
    gateway_scoring_data: GatewayScoringData,
) -> DeciderFlow<GatewayScoreMap> {
    let merchant = asks(|ctx| ctx.dp_merchant_account.clone());
    let pmt = asks(|ctx| ctx.dp_txn_card_info.payment_method_type.clone());
    let order_ref = asks(|ctx| ctx.dp_order.clone());
    let pmt_str = pmt.to_string();
    let functional_gateways = GF::get_gws();
    log_debug_v("get_cached_scores_based_on_srv3", format!("my scoring flow functionalGateways {:?}", functional_gateways));

    let sr_gateway_redis_key_map = Utils::get_consumer_key(gateway_scoring_data, SR_V3_KEY, false, functional_gateways);

    let merchant_bucket_size = merchant_srv3_input_config
        .and_then(|config| Utils::get_srv3_bucket_size(&config, &pmt_str, &pm))
        .or_else(|| default_srv3_input_config.and_then(|config| Utils::get_srv3_bucket_size(&config, &pmt_str, &pm)))
        .unwrap_or(C::default_srv3_based_bucket_size);
    log_debug_t("Sr_V3_Bucket_Size", format!("{}", merchant_bucket_size));
    Utils::delete_score_key_if_bucket_size_changes(merchant_bucket_size, sr_gateway_redis_key_map.clone());
    Utils::set_srv3_bucket_size(merchant_bucket_size);

    let sr_gateway_redis_key_map_filtered: Vec<(ETG::Gateway, String)> = sr_gateway_redis_key_map
        .into_iter()
        .filter_map(|(gw_str, key)| {
            functional_gateways.iter().find(|gw| gw.to_string() == gw_str).map(|gw| (gw.clone(), key))
        })
        .collect();

    let mut score_map = GatewayScoreMap::new();
    for gateway_redis_key in sr_gateway_redis_key_map_filtered {
        score_map = get_gateway_score_based_on_srv3(merchant_bucket_size, score_map, gateway_redis_key);
    }
    log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation {:?}", score_map));
    reset_and_log_metrics(score_map.clone(), "SR_SELECTION_V3_EVALUATION".to_string());

    let is_srv3_reset_enabled = M::is_feature_enabled(C::enable_reset_on_sr_v3, Utils::get_mid(&merchant.merchant_id), Config::kv_redis);
    let updated_score_map_after_reset = if is_srv3_reset_enabled {
        let upper_reset_factor = merchant_srv3_input_config
            .and_then(|config| Utils::get_srv3_upper_reset_factor(&config, &pmt_str, &pm))
            .or_else(|| default_srv3_input_config.and_then(|config| Utils::get_srv3_upper_reset_factor(&config, &pmt_str, &pm)))
            .unwrap_or(C::default_srv3_based_upper_reset_factor);
        let lower_reset_factor = merchant_srv3_input_config
            .and_then(|config| Utils::get_srv3_lower_reset_factor(&config, &pmt_str, &pm))
            .or_else(|| default_srv3_input_config.and_then(|config| Utils::get_srv3_lower_reset_factor(&config, &pmt_str, &pm)))
            .unwrap_or(C::default_srv3_based_lower_reset_factor);
        log_debug_t("Sr_V3_Upper_Reset_Factor", format!("{}", upper_reset_factor));
        log_debug_t("Sr_V3_Lower_Reset_Factor", format!("{}", lower_reset_factor));
        let (updated_score_map_after_reset, is_reset_done) = reset_srv3_score(score_map.clone(), merchant_bucket_size, sr_gateway_redis_key_map.clone(), upper_reset_factor, lower_reset_factor);
        if is_reset_done {
            log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Reset {:?}", updated_score_map_after_reset));
            reset_and_log_metrics(updated_score_map_after_reset.clone(), "SR_SELECTION_V3_EVALUATION_AFTER_RESET".to_string());
            Utils::set_reset_approach(SRV3_RESET);
        }
        updated_score_map_after_reset
    } else {
        score_map
    };

    let is_srv3_extra_score_enabled = M::is_feature_enabled(C::enable_extra_score_on_sr_v3, Utils::get_mid(&merchant.merchant_id), Config::kv_redis);
    let final_score_map = if is_srv3_extra_score_enabled {
        let mut final_score_map = GatewayScoreMap::new();
        for gw in functional_gateways {
            final_score_map = add_extra_score(updated_score_map_after_reset.clone(), merchant_bucket_size, merchant_srv3_input_config.clone(), default_srv3_input_config.clone(), &pmt_str, &pm, final_score_map, gw);
        }
        log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Extra Score {:?}", final_score_map));
        reset_and_log_metrics(final_score_map.clone(), "SR_SELECTION_V3_EVALUATION_AFTER_EXTRA_SCORE".to_string());
        final_score_map
    } else {
        updated_score_map_after_reset
    };

    let is_srv3_binomial_distribution_enabled = M::is_feature_enabled(C::enable_binomial_distribution_on_sr_v3, Utils::get_mid(&merchant.merchant_id), Config::kv_redis);
    let is_srv3_beta_distribution_enabled = M::is_feature_enabled(C::enable_beta_distribution_on_sr_v3, Utils::get_mid(&merchant.merchant_id), Config::kv_redis);
    let final_score_map_after_distribution = match (is_srv3_binomial_distribution_enabled, is_srv3_beta_distribution_enabled) {
        (true, _) => {
            let mut final_score_map_after_distribution = GatewayScoreMap::new();
            for gw in functional_gateways {
                final_score_map_after_distribution = sample_from_binomial_distribution(final_score_map.clone(), merchant_bucket_size, final_score_map_after_distribution, gw);
            }
            log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Binomial Distribution {:?}", final_score_map_after_distribution));
            reset_and_log_metrics(final_score_map_after_distribution.clone(), "SR_SELECTION_V3_EVALUATION_AFTER_BINOMIAL_DISTRIBUTION".to_string());
            final_score_map_after_distribution
        }
        (_, true) => {
            let mut final_score_map_after_distribution = GatewayScoreMap::new();
            for gw in functional_gateways {
                final_score_map_after_distribution = sample_from_beta_distribution(final_score_map.clone(), merchant_bucket_size, final_score_map_after_distribution, gw);
            }
            log_debug_v("get_cached_scores_based_on_srv3", format!("Gateway Score Map After Sr V3 Evaluation And Beta Distribution {:?}", final_score_map_after_distribution));
            reset_and_log_metrics(final_score_map_after_distribution.clone(), "SR_SELECTION_V3_EVALUATION_AFTER_BETA_DISTRIBUTION".to_string());
            final_score_map_after_distribution
        }
        (_, _) => final_score_map,
    };
    final_score_map_after_distribution
}

pub fn sample_from_binomial_distribution(
    final_score_map: GatewayScoreMap,
    merchant_bucket_size: i32,
    acc: GatewayScoreMap,
    gw: ETG::Gateway,
) -> impl Future<Output = GatewayScoreMap> {
    let gw_score = final_score_map.get(&gw).unwrap_or(&1.0);
    let gen = run_io_with_art(
        "GatewayDecider.GWScoring::sampleFromBinomialDistribution::newIOGenM",
        new_io_gen_m::<IO>(init_std_gen()),
    );
    let sample_value = run_io_with_art(
        "GatewayDecider.GWScoring::sampleFromBinomialDistribution::runRVar",
        run_rvar(binomial(merchant_bucket_size, *gw_score), gen),
    );
    let updated_gw_score = sample_value as f64 / merchant_bucket_size as f64;
    acc.insert(gw, updated_gw_score);
    acc
}

pub fn sample_from_beta_distribution(
    final_score_map: GatewayScoreMap,
    merchant_bucket_size: i32,
    acc: GatewayScoreMap,
    gw: ETG::Gateway,
) -> impl Future<Output = GatewayScoreMap> {
    let gw_score = final_score_map.get(&gw).unwrap_or(&1.0);
    let gw_success = merchant_bucket_size as f64 * gw_score;
    let gw_failure = merchant_bucket_size as f64 - gw_success;
    let gen = run_io_with_art(
        "GatewayDecider.GWScoring::sampleFromBetaDistribution::newIOGenM",
        new_io_gen_m::<IO>(init_std_gen()),
    );
    let updated_gw_score = run_io_with_art(
        "GatewayDecider.GWScoring::sampleFromBetaDistribution::runRVar",
        run_rvar(beta(gw_success, gw_failure), gen),
    );
    acc.insert(gw, updated_gw_score);
    acc
}

pub fn add_extra_score(
    updated_score_map_after_reset: GatewayScoreMap,
    merchant_bucket_size: i32,
    merchant_sr_v3_input_config: Option<SrV3InputConfig>,
    default_sr_v3_input_config: Option<SrV3InputConfig>,
    pmt: String,
    pm: String,
    final_score_map: GatewayScoreMap,
    gw: ETG::Gateway,
) -> impl Future<Output = GatewayScoreMap> {
    let gateway_sigma_factor = merchant_sr_v3_input_config
        .and_then(|config| Utils::get_sr_v3_gateway_sigma_factor(&config, &pmt, &pm, &gw))
        .or_else(|| {
            default_sr_v3_input_config
                .and_then(|config| Utils::get_sr_v3_gateway_sigma_factor(&config, &pmt, &pm, &gw))
        })
        .unwrap_or(C::default_sr_v3_based_gateway_sigma_factor);
    log_debug_t(
        "Sr_V3_Gateway_Sigma_Factor",
        format!(
            "Gateway: {:?}, Sigma Factor: {:?}",
            gw, gateway_sigma_factor
        ),
    );
    let score = updated_score_map_after_reset.get(&gw).unwrap_or(&1.0);
    let float_bucket_size = merchant_bucket_size as f64;
    let var = (score * (1.0 - score)) / float_bucket_size;
    let sigma = var.sqrt();
    let extra_score = sigma * gateway_sigma_factor;
    let final_score = score + extra_score;
    final_score_map.insert(gw, final_score.clamp(0.0, 1.0));
    final_score_map
}

pub fn reset_sr_v3_score(
    score_map: GatewayScoreMap,
    bucket_size: i32,
    sr_gateway_redis_key_map: GatewayRedisKeyMap,
    upper_reset_factor: f64,
    lower_reset_factor: f64,
) -> impl Future<Output = (GatewayScoreMap, bool)> {
    let max_score = Utils::get_max_score_gateway(&score_map)
        .map(|(_, score)| score)
        .unwrap_or(1.0);
    let float_bucket_size = bucket_size as f64;
    let var = (max_score * (1.0 - max_score)).max(0.09) / float_bucket_size;
    let sigma = var.sqrt();
    let score_reset_threshold = max_score - (upper_reset_factor * sigma);
    let number_of_zeros = bucket_size
        .min(2)
        .max(((1.0 - max_score + lower_reset_factor * sigma) * float_bucket_size) as i32);
    let interval_between_zeros = (float_bucket_size - 1.0) / (number_of_zeros as f64 - 1.0);
    let score_list = (1..=bucket_size)
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
    let score_reset_value = bucket_size - number_of_zeros;
    let key_score_map: Vec<_> = sr_gateway_redis_key_map
        .iter()
        .filter_map(|(gw, key)| {
            score_map.get(gw).map(|score| (key.clone(), *score))
        })
        .collect();
    let keys_for_reset: Vec<_> = key_score_map
        .iter()
        .filter(|(_, score)| *score < score_reset_threshold)
        .map(|(key, _)| key.clone())
        .collect();
    let updated_score_map = score_map.iter().fold(
        GatewayScoreMap::new(),
        |mut acc, (gw, score)| {
            acc.insert(
                gw.clone(),
                if *score < score_reset_threshold {
                    score_reset_value as f64 / float_bucket_size
                } else {
                    *score
                },
            );
            acc
        },
    );
    keys_for_reset.iter().for_each(|key| {
        reset_gateway_for_sr_v3(score_reset_value, score_list.clone(), key.clone());
    });
    (updated_score_map, !keys_for_reset.is_empty())
}

pub fn reset_gateway_for_sr_v3(
    score_reset_value: i32,
    score_list: Vec<String>,
    redis_key: String,
) -> impl Future<Output = ()> {
    let score_key = format!("{}_}score", redis_key);
    let queue_key = format!("{}_}queue", redis_key);
    Utils::create_moving_window_and_score(
        Config::kv_redis(),
        &queue_key,
        &score_key,
        score_reset_value,
        score_list,
    )
}

pub fn get_gateway_score_based_on_sr_v3(
    bucket_size: i32,
    score_map: GatewayScoreMap,
    gateway_redis_key: (ETG::Gateway, String),
) -> impl Future<Output = GatewayScoreMap> {
    let (gw, key) = gateway_redis_key;
    let score_key = format!("{}_}score", key);
    let maybe_success_count = RC::r_get_t(Config::kv_redis(), &score_key);
    let success_count = maybe_success_count
        .and_then(|count| count.parse::<i32>().ok())
        .unwrap_or(bucket_size);
    let score = (success_count as f64 / bucket_size as f64).clamp(0.0, 1.0);
    score_map.insert(gw, score);
    score_map
}

pub fn text_to_int(val: Option<String>) -> Option<i32> {
    val.and_then(|x| x.parse::<i32>().ok())
}

pub fn evaluate_and_create_score_map(
    mb_gateway: Option<ETG::Gateway>,
    orbd: OptimizationRedisBlockData,
    st: GatewayScoreMap,
) -> impl Future<Output = GatewayScoreMap> {
    if let Some(gateway) = mb_gateway {
        let gateway_success_rates = orbd.aggregate.iter().fold(
            GatewayScoreMap::new(),
            |mut acc, gw_score_details| {
                gw_score_details.transactions_detail.iter().for_each(|gw_details| {
                    process_data(&[gateway.clone()], None, &mut acc, gw_details);
                });
                acc
            },
        );
        let (update_functional_gateways, is_reset_done) = gateway_success_rates.iter().fold(
            (GatewayScoreMap::new(), false),
            |(mut acc, is_reset_done), (gw_name, sr_data)| {
                process_gw_sr(None, &orbd, (acc, is_reset_done), gw_name, sr_data)
            },
        );
        let update_functional_gateways = st.iter().fold(
            update_functional_gateways,
            |mut acc, (gw, _)| {
                process_upd_functional_gws(&mut acc, gw.clone());
                acc
            },
        );
        log_debug_v("gatewaySuccessRates", &gateway_success_rates);
        log_debug_v("updateFunctionalGateways", &update_functional_gateways);
        if is_reset_done {
            Utils::set_reset_approach(SRV2_RESET);
        }
        update_functional_gateways
    } else {
        GatewayScoreMap::new()
    }
}

pub fn create_score_map(score_map: GatewayScoreMap, gw: ETG::Gateway) -> GatewayScoreMap {
    score_map.insert(gw, 1.0);
    score_map
}

pub fn process_data(
    st: &[ETG::Gateway],
    weight: Option<f64>,
    acc: &mut GatewayScoreMap,
    gw_details: &GatewayDetails,
) {
    let gw_name = gw_details.gateway_name.clone();
    if st.iter().any(|gw| gw.to_string() == gw_name) {
        let curr_sr = acc.get(&gw_name).unwrap_or(&SuccessRateData::default());
        let new_sr = curr_sr.success_txn_count
            + (gw_details.success_txn_count as f64 * weight.unwrap_or(1.0)) as i32;
        let new_total_txn = curr_sr.total_txn
            + (gw_details.total_txn_count as f64 * weight.unwrap_or(1.0)) as i32;
        acc.insert(
            gw_name.clone(),
            SuccessRateData {
                success_txn_count: new_sr,
                total_txn: new_total_txn,
            },
        );
    }
}

pub fn process_gw_sr(
    vol_data: Option<i64>,
    orbd: &OptimizationRedisBlockData,
    (mut acc, is_reset_done): (GatewayScoreMap, bool),
    gw_name: &String,
    sr_data: &SuccessRateData,
) -> (GatewayScoreMap, bool) {
    let (sr, is_reset_done) = if sr_data.total_txn < 4
        || calculate_threshold(vol_data, orbd, sr_data.total_txn)
    {
        (1.0, true)
    } else {
        (
            (sr_data.success_txn_count as f64 / sr_data.total_txn as f64).min(1.0),
            false,
        )
    };
    if let Some(gw) = Utils::text_to_gateway(gw_name) {
        acc.insert(gw, sr);
        (acc, is_reset_done || is_reset_done)
    } else {
        (acc, is_reset_done || is_reset_done)
    }
}

pub fn calculate_threshold(
    vol_data: Option<i64>,
    orbd: &OptimizationRedisBlockData,
    gateway_total_txn: i64,
) -> bool {
    if let Some(vol_data) = vol_data {
        let total_bucket_txn_count = orbd.aggregate.iter().fold(0, |acc, block| {
            acc + block.block_total_txn
        });
        let vol_percent = vol_data as f64 / 100.0;
        gateway_total_txn < (total_bucket_txn_count as f64 * vol_percent) as i64
    } else {
        false
    }
}

pub fn process_upd_functional_gws(
    update_functional_gateways: &mut GatewayScoreMap,
    gw: ETG::Gateway,
) {
    if !update_functional_gateways.contains_key(&gw) {
        update_functional_gateways.insert(gw, 1.0);
    }
}

pub fn prepare_log_curr_score(acc: Vec<LogCurrScore>, gw: ETG::Gateway, score: f64) -> Vec<LogCurrScore> {
    let mut acc = acc;
    acc.push(LogCurrScore {
        gateway: gw.to_string(),
        score,
    });
    acc
}

pub async fn reset_and_log_metrics(
    final_updated_gateway_score_maps: GatewayScoreMap,
    metric_title: String,
) -> DeciderFlow<()> {
    reset_metric_log_data().await;
    modify(|s| {
        s.sr_metric_log_data.merchant_gateway_score = Some(
            serde_json::to_value(
                final_updated_gateway_score_maps.iter().fold(Vec::new(), |acc, (gw, score)| {
                    prepare_log_curr_score(acc, gw.clone(), *score)
                }),
            )
            .unwrap(),
        );
    })
    .await;
    Utils::metric_tracker_log(
        metric_title.clone(),
        "GW_SCORING",
        Utils::get_metric_log_format(metric_title).await,
    )
    .await;
}

pub fn get_score_with_priority(
    functional_gateways: Vec<ETG::Gateway>,
    gateway_priority_list: Vec<ETG::Gateway>,
) -> GatewayScoreMap {
    let (p1, im1) = gateway_priority_list.iter().fold((1.0, MP::new()), |(p, m), gw| {
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

    functional_gateways.iter().fold((p1, im1), |(p, m), gw| {
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

pub async fn update_score_for_issuer() -> DeciderFlow<GatewayScoreMap> {
    let old_sm = get_gwsm().await;
    let merchant = asks(|ctx| ctx.dp_merchant_account.clone()).await;
    let m_prefs = asks(|ctx| ctx.dp_merchant_prefs.clone()).await;
    let issuer_routing_enabled = m_prefs.issuer_routing_enabled;

    log_debug_t(
        "update_score_for_issuer",
        format!(
            "issuerRouting for merchant {} : {}",
            Utils::get_m_id(merchant.merchant_id.clone()),
            issuer_routing_enabled
        ),
    )
    .await;

    if issuer_routing_enabled {
        if let Some(issuer) = asks(|ctx| ctx.dp_txn_card_info.card_issuer_bank_name.clone()).await {
            if let Some(route) = ETIssuerR::find_by_issuer_and_merchant_id(
                issuer.clone(),
                merchant.merchant_id.clone(),
            )
            .await
            {
                set_gwsm({
                    let mut old_sm = old_sm;
                    old_sm.entry(route.preferred_gateway.clone()).and_modify(|score| {
                        *score *= route.preference_score;
                    });
                    old_sm
                })
                .await;
            }
        }
    }

    return_sm_with_log(UpdateScoreForIssuer, issuer_routing_enabled).await
}

pub fn update_score_for_isin() -> DeciderFlow<GatewayScoreMap> {
    let old_sm = get_gwsm();
    let merchant = asks(|ctx| ctx.dp_merchant_account.clone());
    let m_prefs = asks(|ctx| ctx.dp_merchant_prefs.clone());
    let isin_routing_enabled = m_prefs.isin_routing_enabled;

    log_debug_t(
        "updateScoreForIsin",
        format!(
            "isinRouting for merchant {} : {}",
            Utils::get_m_id(merchant.merchant_id),
            isin_routing_enabled
        ),
    );

    if isin_routing_enabled {
        let m_isin = asks(|ctx| ctx.dp_txn_card_info.card_isin.clone());
        let m_route = Utils::get_isin_routes_with_extended_bins(&m_isin, merchant.merchant_id.clone());

        if let Some(route) = m_route {
            set_gwsm(MP::adjust(
                |score| score * route.preference_score,
                route.preferred_gateway.clone(),
                old_sm,
            ));
        }
    }

    return_sm_with_log("UpdateScoreForIsin", isin_routing_enabled)
}

pub fn update_score_for_card_brand() -> DeciderFlow<GatewayScoreMap> {
    let old_sm = get_gwsm();
    let merchant = asks(|ctx| ctx.dp_merchant_account.clone());
    let m_prefs = asks(|ctx| ctx.dp_merchant_prefs.clone());
    let card_brand_routing_enabled = m_prefs.card_brand_routing_enabled;

    log_debug_t(
        "updateScoreForCardBrand",
        format!(
            "cardBrandRouting for merchant {} : {}",
            Utils::get_m_id(merchant.merchant_id),
            card_brand_routing_enabled
        ),
    );

    if card_brand_routing_enabled {
        let m_card_brand = Utils::get_card_brand();

        if let Some(cb) = m_card_brand {
            let m_route = ETCBR::find_by_card_brand_and_merchant_p_id(
                &cb,
                ETM::MerchantPId {
                    merchant_p_id: merchant.id.merchant_p_id.clone(),
                },
            );

            if let Some(route) = m_route {
                set_gwsm(MP::adjust(
                    |score| score * route.preference_score,
                    route.preferred_gateway.clone(),
                    old_sm,
                ));
            }
        }
    }

    return_sm_with_log("UpdateScoreForCardBrand", card_brand_routing_enabled)
}

pub fn update_score_for_outage() -> DeciderFlow<GatewayScoreMap> {
    let old_sm = get_gwsm();
    let txn_detail = asks(|ctx| ctx.dp_txn_detail.clone());
    let txn_card_info = asks(|ctx| ctx.dp_txn_card_info.clone());
    let merchant = asks(|ctx| ctx.dp_merchant_account.clone());
    let scheduled_outage_validation_duration = RService::find_by_name_from_redis(C::SCHEDULED_OUTAGE_VALIDATION_DURATION)
        .and_then(|val| Utils::decode_from_text(&val).unwrap_or(Some(86400)))
        .unwrap_or(86400);

    let potential_outages = get_scheduled_outage(scheduled_outage_validation_duration);
    log_debug_v("updated score for outage", &potential_outages);

    let juspay_bank_code = Utils::fetch_juspay_bank_code(&txn_card_info)
        .and_then(|code| ETJ::find_juspay_bank_code(&code));

    let out_gws: Vec<_> = potential_outages
        .into_iter()
        .filter(|outage| check_scheduled_outage(&txn_detail, &txn_card_info, &merchant.merchant_id, &juspay_bank_code, outage))
        .collect();

    log_debug_v("updated score for outage filtered", &out_gws);
    log_debug_v(
        "updated score for outage info",
        format!(
            "{:?}, {:?}, {:?}",
            txn_detail.txn_object_type,
            txn_detail.source_object,
            Utils::fetch_juspay_bank_code(&txn_card_info)
        ),
    );

    let new_sm = out_gws.iter().fold(old_sm, |m, ogw| {
        MP::adjust(|score| score / 10.0, ogw.gateway.clone(), m)
    });

    if !out_gws.is_empty() {
        set_is_scheduled_outage(true);
    }

    set_gwsm(new_sm);
    return_sm_with_log("UpdateScoreForOutage", true)
}

pub fn get_global_gateway_score(
    redis_key: String,
    max_count: Option<i64>,
    score_threshold: Option<f64>,
) -> DeciderFlow<Option<(Vec<GlobalScoreLog>, f64)>> {
    if let (Some(max_count), Some(score_threshold)) = (max_count, score_threshold) {
        let m_value: Option<GlobalGatewayScore> = RC::r_get(Config::ec_redis(), &redis_key);
        match m_value {
            None => Ok(None),
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
                Ok(Some((
                    filtered_merchants,
                    if should_penalize {
                        score_threshold - 0.1
                    } else {
                        score_threshold
                    },
                )))
            }
        }
    } else {
        log_warning_t(
            "get_global_gateway_score",
            format!(
                "max_count is {:?}, score_threshold is {:?}",
                max_count, score_threshold
            ),
        );
        Ok(None)
    }
}

fn mk_gsl(gs: GlobalScore, score_threshold: f64, max_count: i64) -> GlobalScoreLog {
    GlobalScoreLog {
        current_score: Utils::round_off_to_3(gs.score),
        transaction_count: gs.transaction_count,
        merchant_id: gs.merchant_id,
        elimination_threshold: Utils::round_off_to_3(score_threshold),
        elimination_max_count_threshold: max_count,
    }
}

pub fn get_gateway_wise_routing_inputs_for_global_sr(
    gateway: ETG::Gateway,
    merchant_wise_global_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    global_success_rate_based_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    global_routing_defaults: SRGlobalRoutingDefaults,
) -> GatewayWiseSuccessRateBasedRoutingInput {
    let global_gateway_wise_inputs = global_success_rate_based_routing_input
        .and_then(|input| input.global_gateway_wise_inputs)
        .unwrap_or_default();
    let merchant_gateway_wise_inputs = merchant_wise_global_routing_input
        .and_then(|input| input.global_gateway_wise_inputs)
        .unwrap_or_default();

    let get_gateway_threshold_input_given_by_global_config =
        |gw: &ETG::Gateway| global_gateway_wise_inputs.iter().find(|ri| ri.gateway == *gw);

    let get_merchant_gateway_threshold_input_given_by_global_config =
        |gw: &ETG::Gateway| merchant_gateway_wise_inputs.iter().find(|ri| ri.gateway == *gw);

    let mk_new_entry = |gw: ETG::Gateway| GatewayWiseSuccessRateBasedRoutingInput {
        gateway: gw,
        elimination_threshold: global_routing_defaults.default_global_elimination_threshold,
        elimination_max_count_threshold: global_routing_defaults.default_global_elimination_max_count_threshold,
        elimination_level: global_routing_defaults.default_global_elimination_level,
        current_score: None,
        selection_max_count_threshold: None,
        soft_txn_reset_count: None,
        gateway_level_elimination_threshold: None,
        last_reset_time_stamp: None,
    };

    let adjust_defs = |mut gri: GatewayWiseSuccessRateBasedRoutingInput| {
        gri.elimination_level = gri.elimination_level.or_else(|| {
            global_success_rate_based_routing_input
                .as_ref()
                .and_then(|input| input.default_global_elimination_level)
        }).or(Some(ETGRI::SelectionLevel::PAYMENT_METHOD));
        gri.elimination_max_count_threshold = gri
            .elimination_max_count_threshold
            .or(global_routing_defaults.default_global_elimination_max_count_threshold);
        gri.elimination_threshold = gri
            .elimination_threshold
            .or(global_routing_defaults.default_global_elimination_threshold);
        gri
    };

    get_merchant_gateway_threshold_input_given_by_global_config(&gateway)
        .or_else(|| get_gateway_threshold_input_given_by_global_config(&gateway))
        .map(adjust_defs)
        .unwrap_or_else(|| adjust_defs(mk_new_entry(gateway)))
}

pub fn get_global_elimination_gateway_score(
    gateway_key_map: HashMap<String, String>,
    gsri: GatewayWiseSuccessRateBasedRoutingInput,
) -> DeciderFlow<Option<(Vec<GlobalScoreLog>, f64)>> {
    if gsri.elimination_level != Some(ETGRI::SelectionLevel::NONE) {
        let redis_key = gateway_key_map
            .get(&gsri.gateway.to_string())
            .cloned()
            .unwrap_or_default();
        get_global_gateway_score(
            redis_key,
            gsri.elimination_max_count_threshold,
            gsri.elimination_threshold,
        )
    } else {
        Ok(None)
    }
}

pub fn update_gateway_score_based_on_global_success_rate(
    merchant_wise_global_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    global_success_rate_based_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    gateway_scoring_data: GatewayScoringData,
) -> DeciderFlow<(GatewayScoreMap, Option<Vec<GatewayWiseSuccessRateBasedRoutingInput>, bool>)> {
    let gateway_score = get_gwsm();
    let txn_detail = get_dp_txn_detail();
    let merchant_id = txn_detail.merchant_id.clone();

    let (global_elimination_occurred, global_elimination_gateway_score_map) = match check_sr_global_routing_defaults(
        global_success_rate_based_routing_input.clone(),
        merchant_wise_global_routing_input.clone(),
    ) {
        Ok(global_routing_defaults) => {
            let gateway_success_rate_inputs = gateway_score
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

            let gateway_list = Utils::get_gateway_list(&gateway_score);
            let gateway_redis_key_map = Utils::get_consumer_key(
                &gateway_scoring_data,
                ELIMINATION_GLOBAL_KEY,
                false,
                &gateway_list,
            );

            let (upd_gateway_success_rate_inputs, global_gateway_scores) = gateway_success_rate_inputs
                .iter()
                .fold((Vec::new(), Vec::new()), |(mut ugsri, mut ggs), gsri| {
                    log_info_t(
                        "scoringFlow",
                        format!(
                            "Current global score evaluation {} {} : {} with elimination level {} threshold {} max count {}",
                            txn_detail.txn_id,
                            gsri.gateway,
                            gsri.current_score.unwrap_or_default(),
                            gsri.elimination_level.unwrap_or_default(),
                            gsri.elimination_threshold.unwrap_or_default(),
                            gsri.elimination_max_count_threshold.unwrap_or_default()
                        ),
                    );

                    if let Some((global_gateway_score, s)) =
                        get_global_elimination_gateway_score(&gateway_redis_key_map, gsri)
                    {
                        let new_gsri = GatewayWiseSuccessRateBasedRoutingInput {
                            current_score: Some(s),
                            ..gsri.clone()
                        };
                        ugsri.push(new_gsri);
                        ggs.push(update_global_score_log(gsri.gateway.clone(), global_gateway_score));
                    }

                    (ugsri, ggs)
                });

            let filtered_gateway_success_rate_inputs = upd_gateway_success_rate_inputs
                .into_iter()
                .filter(|x| {
                    if let (Some(cs), Some(et)) = (x.current_score, x.elimination_threshold) {
                        cs < et
                    } else {
                        false
                    }
                })
                .collect::<Vec<_>>();

            reset_metric_log_data();
            let init_metric_log_data = get_sr_metric_log_data();
            let before_gwsm = get_gwsm();
            set_metric_log_data(SRMetricLogData {
                gateway_before_evaluation: Utils::get_max_score_gateway(&before_gwsm).map(|x| x.0),
                downtime_status: filtered_gateway_success_rate_inputs
                    .iter()
                    .map(|x| x.gateway.clone())
                    .collect(),
                ..init_metric_log_data.clone()
            });

            if !filtered_gateway_success_rate_inputs.is_empty() {
                let new_gateway_score = filtered_gateway_success_rate_inputs.iter().fold(
                    gateway_score.clone(),
                    |acc, x| penalize_gsr(&txn_detail.txn_id, acc, x),
                );
                set_gwsm(new_gateway_score.clone());
                let old_sr_metric_log_data = get_sr_metric_log_data();
                set_metric_log_data(SRMetricLogData {
                    gateway_after_evaluation: Utils::get_max_score_gateway(&new_gateway_score)
                        .map(|x| x.0),
                    ..old_sr_metric_log_data.clone()
                });
            } else {
                log_info_t(
                    "scoringFlow",
                    format!(
                        "No gateways are eligible for penalties & fallback {} based on global score",
                        txn_detail.txn_id
                    ),
                );
                let old_sr_metric_log_data = get_sr_metric_log_data();
                set_metric_log_data(SRMetricLogData {
                    gateway_after_evaluation: Utils::get_max_score_gateway(&gateway_score)
                        .map(|x| x.0),
                    ..old_sr_metric_log_data.clone()
                });
            }

            let old_sr_metric_log_data = get_sr_metric_log_data();
            log_debug_v("MetricData-GLOBAL-ELIMINATION", old_sr_metric_log_data.clone());

            let global_elimination_occurred = old_sr_metric_log_data
                .gateway_before_evaluation
                .is_some()
                && old_sr_metric_log_data.gateway_before_evaluation
                    != old_sr_metric_log_data.gateway_after_evaluation;

            if !global_gateway_scores.is_empty() {
                set_metric_log_data(SRMetricLogData {
                    merchant_gateway_score: Some(A::to_json(global_gateway_scores)),
                    ..old_sr_metric_log_data.clone()
                });
                Utils::metric_tracker_log(
                    "GLOBAL_SR_EVALUATION",
                    "GW_SCORING",
                    Utils::get_metric_log_format("GLOBAL_SR_EVALUATION"),
                );
            } else {
                log_info_v(
                    "scoringFlow",
                    format!(
                        "Global scores not available for {} {}",
                        merchant_id, txn_detail.txn_id
                    ),
                );
            }

            log_info_v(
                "scoringFlow",
                format!(
                    "Gateway scores after considering global SR based elimination for {} : {}",
                    txn_detail.txn_id, global_gateway_scores
                ),
            );

            (global_elimination_occurred, Some(filtered_gateway_success_rate_inputs))
        }
        Err(reason) => {
            log_debug_t("Global SR routing", reason.clone());
            log_info_t(
                "scoringFlow",
                format!(
                    "Global SR routing not enabled for merchant {} txn {}",
                    ETM::to_text(&merchant_id),
                    txn_detail.txn_id
                ),
            );
            (false, None)
        }
    };

    let sm = return_sm_with_log(ScoringByGatewayScoreBasedOnGlobalSuccessRate, false);
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

pub async fn penalize_gsr(
    txn_id: ETTD::TransactionId,
    gs: GatewayScoreMap,
    sri: GatewayWiseSuccessRateBasedRoutingInput,
) -> DeciderFlow<GatewayScoreMap> {
    let mut new_gs = gs.clone();
    new_gs.entry(sri.gateway.clone()).and_modify(|v| *v /= 5.0);
    log_info_t(
        "scoringFlow",
        format!(
            "Penalizing gateway {:?} for {:?} based on global score",
            sri.gateway,
            review(ETTD::transaction_id_text, txn_id)
        ),
    )
    .await;
    Ok(new_gs)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
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

pub async fn get_gateway_wise_routing_inputs_for_merchat_sr(
    merchant_acc: ETM::MerchantAccount,
    txn_detail: ETTD::TxnDetail,
    txn_card_info: ETCT::TxnCardInfo,
    gateway: ETG::Gateway,
    gateway_success_rate_merchant_input: Option<GatewayWiseSuccessRateBasedRoutingInput>,
    default_success_rate_based_routing_input: Option<GatewayWiseSuccessRateBasedRoutingInput>,
) -> GatewayWiseSuccessRateBasedRoutingInput {
    let m_option = RService::find_by_name_from_redis(C::SR_BASED_GATEWAY_ELIMINATION_THRESHOLD).await;
    let default_soft_txn_reset_count = RService::find_by_name_from_redis(C::SR_BASED_TXN_RESET_COUNT)
        .await
        .unwrap_or(C::GW_DEFAULT_TXN_SOFT_RESET_COUNT);
    let is_elimination_v2_enabled = Redis::is_redis_feature_enabled(
        C::ENABLE_ELIMINATION_V2,
        ETM::to_text(&merchant_acc.merchant_id),
    )
    .await;

    let default_elimination_threshold = m_option.or(Some(C::DEFAULT_SR_BASED_GATEWAY_ELIMINATION_THRESHOLD));
    let merchant_given_default_threshold = gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| input.default_elimination_threshold.clone());
    let merchant_given_default_gateway_sr_threshold = gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| input.default_gateway_level_elimination_threshold.clone());
    let merchant_given_default_elimination_level = gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| input.default_elimination_level.clone());
    let merchant_given_default_soft_txn_reset_count = gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| input.default_global_soft_txn_reset_count.clone());

    let default_merchant_elimination_threshold = default_success_rate_based_routing_input
        .as_ref()
        .and_then(|input| input.default_elimination_threshold.clone());
    let default_gateway_level_sr_elimination_threshold = default_success_rate_based_routing_input
        .as_ref()
        .and_then(|input| input.default_gateway_level_elimination_threshold.clone());
    let default_merchant_elimination_level = default_success_rate_based_routing_input
        .as_ref()
        .and_then(|input| input.default_elimination_level.clone());
    let default_merchant_soft_txn_reset_count = default_success_rate_based_routing_input
        .as_ref()
        .and_then(|input| input.default_global_soft_txn_reset_count.clone());

    let gateway_wise_inputs_list = gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| input.gateway_wise_inputs.clone())
        .unwrap_or_else(Vec::new);

    let elimination_threshold = merchant_given_default_threshold
        .or(default_merchant_elimination_threshold)
        .or(default_elimination_threshold);

    let elimination_threshold = if is_elimination_v2_enabled {
        get_elimination_v2_threshold(&merchant_acc, &txn_card_info, &txn_detail).await
    } else {
        elimination_threshold
    };

    let mk_new_entry = |gw: ETG::Gateway| GatewayWiseSuccessRateBasedRoutingInput {
        gateway: gw,
        elimination_threshold: elimination_threshold.clone(),
        elimination_level: merchant_given_default_elimination_level
            .or(default_merchant_elimination_level)
            .or(Some(SelectionLevel::PAYMENT_METHOD)),
        soft_txn_reset_count: merchant_given_default_soft_txn_reset_count
            .or(default_merchant_soft_txn_reset_count)
            .or(Some(default_soft_txn_reset_count)),
        gateway_level_sr_threshold: merchant_given_default_gateway_sr_threshold
            .or(default_gateway_level_sr_elimination_threshold)
            .or(Some(C::DEF_SR_BASED_GW_LEVEL_ELIMINATION_THRESHOLD)),
        ..Default::default()
    };

    gateway_wise_inputs_list
        .iter()
        .find(|ri| ri.gateway == gateway)
        .map(|e| GatewayWiseSuccessRateBasedRoutingInput {
            elimination_level: e.elimination_level.clone().or(merchant_given_default_elimination_level).or(Some(SelectionLevel::GATEWAY)),
            ..e.clone()
        })
        .unwrap_or_else(|| mk_new_entry(gateway))
}

async fn get_elimination_v2_threshold(
    merchant_acc: &ETM::MerchantAccount,
    txn_card_info: &ETCT::TxnCardInfo,
    txn_detail: &ETTD::TxnDetail,
) -> Option<f64> {
    let m_gateway_success_rate_merchant_input = Utils::decode_and_log_error(
        "Gateway Decider Input Decode Error",
        merchant_acc.gateway_success_rate_based_decider_input.clone(),
    )
    .await;

    let sr1_th_weight_env = Env::JuspayEnv {
        key: C::THRESHOLD_WEIGHT_SR1,
        action_left: Env::mk_default_env_action(0.29),
        decrypt_func: Box::new(|x| async { x }),
        log_when_throw_exception: None,
    };
    let sr1_th_weight = Env::lookup_env(sr1_th_weight_env).await;

    let sr2_th_weight_env = Env::JuspayEnv {
        key: C::THRESHOLD_WEIGHT_SR2,
        action_left: Env::mk_default_env_action(0.71),
        decrypt_func: Box::new(|x| async { x }),
        log_when_throw_exception: None,
    };
    let sr2_th_weight = Env::lookup_env(sr2_th_weight_env).await;

    if let Some((sr1, sr2, n, m_pmt, m_pm, m_txn_object_type, source)) =
        get_sr1_and_sr2_and_n(m_gateway_success_rate_merchant_input, merchant_acc.merchant_id.clone(), txn_card_info, txn_detail).await
    {
        log_info_t(
            "CALCULATING_THRESHOLD:SR1_SR2_N_PMT_PM_TXNOBJECTTYPE_CONFIGSOURCE",
            format!(
                "{} {} {} {} {} {} {}",
                sr1,
                sr2,
                n,
                m_pmt.unwrap_or_else(|| "Nothing".to_string()),
                m_pm.unwrap_or_else(|| "Nothing".to_string()),
                m_txn_object_type.unwrap_or_else(|| "Nothing".to_string()),
                source
            ),
        )
        .await;

        log_info_t(
            "THRESHOLD_VALUE",
            format!("{}", ((sr1_th_weight * sr1) + (sr2_th_weight * sr2)) / 100.0),
        )
        .await;

        Some(((sr1_th_weight * sr1) + (sr2_th_weight * sr2)) / 100.0)
    } else {
        log_info_t(
            "ELIMINATION_V2_VALUES_NOT_FOUND:THRESHOLD:PMT_PM_TXNOBJECTTYPE_SOUCREOBJECT",
            format!(
                "{} {} {} {}",
                txn_card_info.payment_method_type,
                txn_card_info.payment_method,
                txn_detail.txn_object_type,
                txn_detail.source_object.unwrap_or_else(|| "Nothing".to_string())
            ),
        )
        .await;

        None
    }
}

async fn get_sr1_and_sr2_and_n(
    m_gateway_success_rate_merchant_input: Option<GatewayWiseSuccessRateBasedRoutingInput>,
    merchant_id: String,
    txn_card_info: ETCT::TxnCardInfo,
    txn_detail: ETTD::TxnDetail,
) -> Option<(f64, f64, f64, Option<String>, Option<String>, Option<String>, ConfigSource)> {
    if let Some(gateway_success_rate_merchant_input) = m_gateway_success_rate_merchant_input {
        if let Some(inputs) = gateway_success_rate_merchant_input.elimination_v2_success_rate_inputs {
            let pmt = txn_card_info.payment_method_type.to_string();
            let source_obj = if txn_card_info.payment_method == "UPI" {
                txn_detail.source_object.clone()
            } else {
                Some(txn_card_info.payment_method.clone())
            };
            let pm = if txn_card_info.payment_method_type == ETP::UPI {
                source_obj.clone()
            } else {
                Some(txn_card_info.payment_method.clone())
            };
            let txn_obj_type = txn_detail.txn_object_type.to_string();

            filter_using_service_config(merchant_id, pmt, pm, txn_obj_type, inputs).await
        } else {
            fetch_default_sr1_and_sr2_and_n(&gateway_success_rate_merchant_input).await
        }
    } else {
        None
    }
}

async fn fetch_default_sr1_and_sr2_and_n(
    gateway_success_rate_merchant_input: &GatewayWiseSuccessRateBasedRoutingInput,
) -> Option<(f64, f64, f64, Option<String>, Option<String>, Option<String>, ConfigSource)> {
    if let Some(sr2) = gateway_success_rate_merchant_input.default_elimination_v2_success_rate {
        fetch_default_sr1_and_n_and_mk_result(sr2).await
    } else {
        None
    }
}

async fn fetch_default_sr1_and_n_and_mk_result(sr2: f64) -> Option<(f64, f64, f64, Option<String>, Option<String>, Option<String>, ConfigSource)> {
    let m_default_sr1 = RC::r_hget(Config::EC_REDIS, construct_sr1_key(merchant_id), C::DEFAULT_FIELD_NAME_FOR_SR1_AND_N).await;
    let m_default_n = RC::r_hget(Config::EC_REDIS, construct_n_key(merchant_id), C::DEFAULT_FIELD_NAME_FOR_SR1_AND_N).await;

    if let (Some(sr1), Some(n)) = (m_default_sr1, m_default_n) {
        Some((sr1, sr2, n, None, None, None, ConfigSource::MERCHANT_DEFAULT))
    } else {
        let m_s_config_sr1 = RService::find_by_name_from_redis(C::DEFAULT_SR1_S_CONFIG_PREFIX(merchant_id)).await;
        let m_s_config_n = RService::find_by_name_from_redis(C::DEFAULT_N_S_CONFIG_PREFIX(merchant_id)).await;

        if let (Some(sr1), Some(n)) = (m_s_config_sr1, m_s_config_n) {
            Some((sr1, sr2, n, None, None, None, ConfigSource::GLOBAL_DEFAULT))
        } else {
            None
        }
    }
}

async fn filter_using_service_config(
    merchant_id: String,
    pmt: String,
    pm: Option<String>,
    txn_obj_type: String,
    inputs: Vec<EliminationSuccessRateInput>,
) -> Option<(f64, f64, f64, Option<String>, Option<String>, Option<String>, ConfigSource)> {
    let m_configs = RService::find_by_name_from_redis(C::INTERNAL_DEFAULT_ELIMINATION_V2_SUCCESS_RATE1_AND_N_PREFIX(merchant_id)).await;
    let configs = m_configs.unwrap_or_else(Vec::new);

    filter_using_service_config_upto(ConfigSource::TXN_OBJECT_TYPE, merchant_id, pmt, pm, txn_obj_type, inputs, configs)
        .await
        .or_else(|| {
            filter_using_service_config_upto(ConfigSource::PAYMENT_METHOD, merchant_id, pmt, pm, txn_obj_type, inputs, configs).await
        })
        .or_else(|| {
            filter_using_service_config_upto(ConfigSource::PAYMENT_METHOD_TYPE, merchant_id, pmt, pm, txn_obj_type, inputs, configs).await
        })
}

pub fn filter_inputs_upto(
    level: FilterLevel,
    pmt: T,
    pm: Option<T>,
    txn_obj_type: T,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    match level {
        FilterLevel::TxnObjectType => filter_inputs_upto_txn_object_type(pmt, pm, txn_obj_type, inputs),
        FilterLevel::PaymentMethod => filter_inputs_upto_payment_method(pmt, pm, inputs),
        FilterLevel::PaymentMethodType => filter_inputs_upto_payment_method_type(pmt, inputs),
    }
}

pub async fn filter_using_redis_upto(
    level: FilterLevel,
    merchant_id: T,
    pmt: T,
    pm: Option<T>,
    txn_obj_type: T,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<(f64, f64, f64, Option<T>, Option<T>, Option<T>, ConfigSource)> {
    let m_input = filter_inputs_upto(level, pmt.clone(), pm.clone(), txn_obj_type.clone(), inputs);
    let m_sr1_and_n = get_sr1_and_n_from_redis_upto(level, merchant_id.clone(), pmt.clone(), pm.clone(), txn_obj_type.clone()).await;
    match (m_input, m_sr1_and_n) {
        (Some(input), Some((sr1, n))) => Some((
            sr1,
            input.success_rate,
            n,
            Some(input.payment_method_type),
            input.payment_method.clone(),
            input.txn_object_type.clone(),
            ConfigSource::Redis,
        )),
        _ => None,
    }
}

pub async fn get_sr1_and_n_from_redis_upto(
    level: FilterLevel,
    merchant_id: T,
    pmt: T,
    m_pm: Option<T>,
    txn_obj_type: T,
) -> Option<(f64, f64)> {
    let sr1_key = construct_sr1_key(&merchant_id);
    let n_key = construct_n_key(&merchant_id);
    let dim_key = construct_dimension_key(level, &pmt, m_pm.as_ref(), &txn_obj_type);

    let redis_sr1 = fetch_from_redis(&sr1_key, &dim_key).await;
    let redis_n = fetch_from_redis(&n_key, &dim_key).await;

    match (redis_sr1, redis_n) {
        (Some(sr1), Some(n)) => Some((sr1, n)),
        _ => None,
    }
}

fn construct_sr1_key(merchant_id: &T) -> T {
    format!("{}{}", C::SR1_KEY_PREFIX, merchant_id)
}

fn construct_n_key(merchant_id: &T) -> T {
    format!("{}{}", C::N_KEY_PREFIX, merchant_id)
}

fn construct_dimension_key(
    level: FilterLevel,
    pmt: &T,
    pm: Option<&T>,
    txn_obj_type: &T,
) -> Option<T> {
    match level {
        FilterLevel::TxnObjectType => pm.map(|pm| format!("{}|{}|{}", pmt, pm, txn_obj_type)),
        FilterLevel::PaymentMethod => pm.map(|pm| format!("{}|{}", pmt, pm)),
        FilterLevel::PaymentMethodType => Some(pmt.clone()),
    }
}

async fn fetch_from_redis(key: &T, dim_key: &Option<T>) -> Option<f64> {
    match dim_key {
        None => None,
        Some(dkey) => RC::r_hget(Config::EC_REDIS, key, dkey).await,
    }
}

pub async fn fetch_sr1_and_n_from_service_config_upto(
    level: FilterLevel,
    merchant_id: T,
    pmt: T,
    pm: Option<T>,
    txn_object_type: T,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
    configs: Vec<SuccessRate1AndNConfig>,
) -> Option<(f64, f64, f64, Option<T>, Option<T>, Option<T>, ConfigSource)> {
    let m_input = filter_inputs_upto(level, pmt.clone(), pm.clone(), txn_object_type.clone(), inputs);
    let m_config = match level {
        FilterLevel::TxnObjectType => filter_configs_upto_txn_object_type(&pmt, pm.as_ref(), &txn_object_type, &configs),
        FilterLevel::PaymentMethod => filter_configs_upto_payment_method(&pmt, pm.as_ref(), &configs),
        FilterLevel::PaymentMethodType => filter_configs_upto_payment_method_type(&pmt, &configs),
    };

    match (m_input, m_config) {
        (Some(input), Some(config)) => Some((
            config.success_rate,
            input.success_rate,
            config.n_value,
            Some(input.payment_method_type),
            input.payment_method.clone(),
            input.txn_object_type.clone(),
            ConfigSource::ServiceConfig,
        )),
        _ => None,
    }
}

fn filter_configs_upto_txn_object_type(
    pmt: &T,
    pm: Option<&T>,
    txn_object_type: &T,
    configs: &[SuccessRate1AndNConfig],
) -> Option<SuccessRate1AndNConfig> {
    pm.and_then(|pm| {
        configs.iter().find(|x| {
            x.payment_method_type == *pmt
                && x.payment_method.as_ref() == Some(pm)
                && x.txn_object_type.as_ref() == Some(txn_object_type)
        }).cloned()
    })
}

fn filter_configs_upto_payment_method(
    pmt: &T,
    pm: Option<&T>,
    configs: &[SuccessRate1AndNConfig],
) -> Option<SuccessRate1AndNConfig> {
    pm.and_then(|pm| {
        configs.iter().find(|x| {
            x.payment_method_type == *pmt
                && x.payment_method.as_ref() == Some(pm)
                && x.txn_object_type.is_none()
        }).cloned()
    })
}

fn filter_configs_upto_payment_method_type(
    pmt: &T,
    configs: &[SuccessRate1AndNConfig],
) -> Option<SuccessRate1AndNConfig> {
    configs.iter().find(|x| {
        x.payment_method_type == *pmt
            && x.payment_method.is_none()
            && x.txn_object_type.is_none()
    }).cloned()
}

fn filter_inputs_upto_txn_object_type(
    pmt: T,
    pm: Option<T>,
    txn_obj_type: T,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    pm.and_then(|pm| {
        inputs.into_iter().find(|x| {
            x.payment_method_type == pmt
                && x.payment_method.as_ref() == Some(&pm)
                && x.txn_object_type.as_ref() == Some(&txn_obj_type)
        })
    })
}

fn filter_inputs_upto_payment_method(
    pmt: T,
    pm: Option<T>,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    pm.and_then(|pm| {
        inputs.into_iter().find(|x| {
            x.payment_method_type == pmt
                && x.payment_method.as_ref() == Some(&pm)
                && x.txn_object_type.is_none()
        })
    })
}

fn filter_inputs_upto_payment_method_type(
    pmt: T,
    inputs: Vec<ETGRI::EliminationSuccessRateInput>,
) -> Option<ETGRI::EliminationSuccessRateInput> {
    inputs.into_iter().find(|x| {
        x.payment_method_type == pmt
            && x.payment_method.is_none()
            && x.txn_object_type.is_none()
    })
}

pub async fn get_success_rate_routing_inputs(
    merchant_acc: ETM::MerchantAccount,
) -> (Option<ETGRI::GatewaySuccessRateBasedRoutingInput>, Option<ETGRI::GatewaySuccessRateBasedRoutingInput>) {
    let redis_input = RService::find_by_name_from_redis(C::DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT).await;
    let decoded_input = Utils::decode_and_log_error(
        "Gateway Decider Input Decode Error",
        &TE::encode_utf8(&merchant_acc.gateway_success_rate_based_decider_input),
    ).await;
    (redis_input, decoded_input)
}

pub async fn evaluate_and_trigger_reset(
    gateway_wise_success_rate_inputs: Vec<GatewayWiseSuccessRateBasedRoutingInput>,
) -> DeciderFlow<()> {
    let txn_detail = DeciderFlow::get_txn_detail().await;
    let reset_gateway_list = evaluate_reset_gateway_score(&gateway_wise_success_rate_inputs, &txn_detail).await;

    if M::is_feature_enabled(
        C::GW_RESET_SCORE_ENABLED,
        &Utils::get_m_id(&txn_detail.merchant_id),
        Config::KV_REDIS,
    ).await {
        trigger_reset_gateway_score(
            &gateway_wise_success_rate_inputs,
            &txn_detail,
            reset_gateway_list,
            true,
        ).await;
    }
}

pub fn update_gateway_score_based_on_success_rate(
    is_sr_metric_enabled: bool,
    initial_gw_scores: GatewayScoreMap,
    gateway_scoring_data: GatewayScoringData,
) -> DeciderFlow<GatewayScoreMap> {
    let merchant_acc = asks(|ctx| ctx.dp_merchant_account);
    let txn_detail = asks(|ctx| ctx.dp_txn_detail);
    let txn_card_info = asks(|ctx| ctx.dp_txn_card_info);
    let enable_success_rate_based_gateway_elimination = None; // Placeholder for MerchantConfig.isPaymentFlowEnabledWithHierarchyCheck logic

    log_debug_t(
        "updateGatewayScoreBasedOnSuccessRate",
        format!(
            "enableSuccessRateBasedGatewayElimination = {:?} for merchant {}",
            enable_success_rate_based_gateway_elimination,
            ETM::to_text(&merchant_acc.merchant_id)
        ),
    );

    if let Some(true) = enable_success_rate_based_gateway_elimination {
        let (default_success_rate_based_routing_input, gateway_success_rate_merchant_input) =
            get_success_rate_routing_inputs(&merchant_acc);

        let is_reset_score_enabled_for_merchant = Redis::is_feature_enabled(
            C::gw_reset_score_enabled(),
            Utils::get_m_id(&txn_detail.merchant_id),
            Config::kv_redis(),
        );

        let payment_method_type = if Utils::is_card_transaction(&txn_card_info) {
            ETP::PaymentMethodType::Card
        } else {
            txn_card_info.payment_method_type.clone()
        };

        let enabled_payment_method_types = gateway_success_rate_merchant_input
            .as_ref()
            .and_then(|input| input.enabled_payment_method_types.clone())
            .unwrap_or_default();

        if !enabled_payment_method_types.is_empty()
            && !enabled_payment_method_types.contains(&payment_method_type)
        {
            log_info_v(
                "scoringFlow",
                format!(
                    "Transaction {} with payment method types {:?} not enabled by {} for SR based routing",
                    review(ETTD::transaction_id_text(), &txn_detail.txn_id),
                    payment_method_type,
                    ETM::to_text(&merchant_acc.merchant_id)
                ),
            );
        } else {
            let (
                gateway_score_global_sr,
                global_elimination_gateway_score_map,
                global_elimination_occurred,
            ) = update_gateway_score_based_on_global_success_rate(
                gateway_success_rate_merchant_input.clone(),
                default_success_rate_based_routing_input.clone(),
                gateway_scoring_data.clone(),
            );

            log_info_v(
                "scoringFlow",
                format!(
                    "Gateway scores input for merchant wise SR based evaluation for {} : {:?}",
                    review(ETTD::transaction_id_text(), &txn_detail.txn_id),
                    to_list_of_gateway_score(&gateway_score_global_sr),
                ),
            );

            let sr_based_elimination_approach_info = if global_elimination_occurred {
                vec!["GLOBAL".to_string()]
            } else {
                vec![]
            };

            let gateway_success_rate_inputs = MP::fold(
                |acc, k, _| {
                    acc.push(get_gateway_wise_routing_inputs_for_merchant_sr(
                        &merchant_acc,
                        &txn_detail,
                        &txn_card_info,
                        k,
                        gateway_success_rate_merchant_input.clone(),
                        default_success_rate_based_routing_input.clone(),
                    ));
                    acc
                },
                vec![],
                &gateway_score_global_sr,
            );

            if !gateway_success_rate_inputs.is_empty() {
                let gateway_list = Utils::get_gateway_list(&gateway_score_global_sr);
                let gateway_redis_key_map = Utils::get_consumer_key(
                    &gateway_scoring_data,
                    ELIMINATION_MERCHANT_KEY,
                    false,
                    &gateway_list,
                );

                let gateway_success_rate_inputs_with_updated_score: Vec<_> =
                    gateway_success_rate_inputs
                        .into_iter()
                        .map(|input| update_current_score(&gateway_redis_key_map, input))
                        .collect();

                let filtered_gateway_success_rate_inputs: Vec<_> = gateway_success_rate_inputs_with_updated_score
                    .into_iter()
                    .filter(|input| {
                        input
                            .current_score
                            .zip(input.elimination_threshold)
                            .map(|(cs, et)| cs < et)
                            .unwrap_or(false)
                    })
                    .collect();

                reset_metric_log_data();
                let init_metric_log_data = gets(|ctx| ctx.sr_metric_log_data.clone());
                let before_gwsm = get_gwsm();
                set_metric_log_data(init_metric_log_data.clone().update_gateway_before_evaluation(
                    Utils::get_max_score_gateway(&before_gwsm).map(|(gw, _)| gw),
                ));

                if !filtered_gateway_success_rate_inputs.is_empty() {
                    let new_sm = filtered_gateway_success_rate_inputs.iter().fold(
                        gateway_score_global_sr.clone(),
                        |acc, input| update_score_with_log(&txn_detail.txn_id, acc, input.clone()),
                    );

                    set_gwsm(new_sm.clone());
                    set_metric_log_data(init_metric_log_data.clone().update_gateway_after_evaluation(
                        Utils::get_max_score_gateway(&new_sm).map(|(gw, _)| gw),
                    ));

                    if is_reset_score_enabled_for_merchant {
                        let reset_enabled_gateway_list =
                            evaluate_reset_gateway_score(&filtered_gateway_success_rate_inputs, &txn_detail);

                        if !reset_enabled_gateway_list.is_empty() {
                            modify(|ctx| {
                                ctx.reset_gateway_list = DL::nub(
                                    ctx.reset_gateway_list
                                        .clone()
                                        .into_iter()
                                        .chain(reset_enabled_gateway_list.into_iter())
                                        .collect(),
                                );
                            });
                        }
                    }
                } else {
                    set_metric_log_data(init_metric_log_data.clone().update_gateway_after_evaluation(
                        Utils::get_max_score_gateway(&before_gwsm).map(|(gw, _)| gw),
                    ));

                    log_info_v(
                        "scoringFlow",
                        format!(
                            "No gateways are eligible for penalties & fallback : {}",
                            txn_detail.txn_id
                        ),
                    );
                }

                let old_sr_metric_log_data = gets(|ctx| ctx.sr_metric_log_data.clone());
                let sr_based_elimination_approach_info = if old_sr_metric_log_data
                    .gateway_before_evaluation
                    .zip(old_sr_metric_log_data.gateway_after_evaluation)
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
                    old_sr_metric_log_data.clone().update_merchant_gateway_score(Some(
                        A::to_json(
                            gateway_success_rate_inputs_with_updated_score
                                .into_iter()
                                .map(transform_gateway_wise_success_rate_based_routing)
                                .collect(),
                        ),
                    )),
                );

                Utils::metric_tracker_log(
                    "SR_EVALUATION",
                    "GW_SCORING",
                    Utils::get_metric_log_format("SR_EVALUATION"),
                );

                log_debug_v(
                    "MetricData-MERCHANT_PMT_PM",
                    format!("{:?}", old_sr_metric_log_data),
                );

                let new_gateway_score = get_gwsm();
                let merchant_enabled_for_unification = Redis::is_feature_enabled(
                    C::merchants_enabled_for_score_keys_unification(),
                    Utils::get_m_id(&txn_detail.merchant_id),
                    Config::kv_redis(),
                );

                let (new_gateway_score, reset_gateway_level_list, gateway_level_sr_elimination) =
                    if merchant_enabled_for_unification {
                        (new_gateway_score.clone(), vec![], false)
                    } else {
                        update_gateway_score_based_on_gateway_level_scores(
                            &gateway_success_rate_inputs,
                            new_gateway_score.clone(),
                            is_reset_score_enabled_for_merchant,
                        )
                    };

                if !reset_gateway_level_list.is_empty() {
                    modify(|ctx| {
                        ctx.reset_gateway_list = DL::nub(
                            ctx.reset_gateway_list
                                .clone()
                                .into_iter()
                                .chain(reset_gateway_level_list.into_iter())
                                .collect(),
                        );
                    });
                }

                let sr_based_elimination_approach_info = if gateway_level_sr_elimination {
                    vec!["GATEWAY".to_string()]
                        .into_iter()
                        .chain(sr_based_elimination_approach_info.into_iter())
                        .collect()
                } else {
                    sr_based_elimination_approach_info
                };

                let reset_gw_list = gets(|ctx| ctx.reset_gateway_list.clone());
                trigger_reset_gateway_score(
                    &gateway_success_rate_inputs,
                    &txn_detail,
                    reset_gw_list,
                    is_reset_score_enabled_for_merchant,
                );

                let gateway_decider_approach = get_decider_approach();
                let (gw_score, downtime, sr_based_elimination_approach_info_res) =
                    if filtered_gateway_success_rate_inputs.len() > 1
                        && new_gateway_score.len() == filtered_gateway_success_rate_inputs.len()
                    {
                        let optimization_during_downtime_enabled = Redis::is_feature_enabled(
                            C::enable_optimization_during_downtime(),
                            Utils::get_m_id(&txn_detail.merchant_id),
                            Config::kv_redis(),
                        );

                        if optimization_during_downtime_enabled {
                            if is_sr_metric_enabled {
                                log_info_v(
                                    "scoringFlow",
                                    format!(
                                        "Overriding priority with SR Scores during downtime for {} : {:?}",
                                        review(ETTD::transaction_id_text(), &txn_detail.txn_id),
                                        new_gateway_score,
                                    ),
                                );

                                (new_gateway_score.clone(), ALL_DOWNTIME, vec![])
                            } else {
                                log_info_v(
                                    "scoringFlow",
                                    format!(
                                        "Overriding priority with PL during downtime for {} : {:?}",
                                        review(ETTD::transaction_id_text(), &txn_detail.txn_id),
                                        initial_gw_scores,
                                    ),
                                );

                                (initial_gw_scores.clone(), ALL_DOWNTIME, vec![])
                            }
                        } else {
                            log_info_t(
                                "scoringFlow",
                                format!(
                                    "Overriding priority with SR Scores during downtime is not enabled for {}",
                                    Utils::get_m_id(&txn_detail.merchant_id),
                                ),
                            );

                            (new_gateway_score.clone(), ALL_DOWNTIME, sr_based_elimination_approach_info)
                        }
                    } else if !global_elimination_gateway_score_map.is_empty() {
                        (
                            new_gateway_score.clone(),
                            GLOBAL_DOWNTIME,
                            sr_based_elimination_approach_info,
                        )
                    } else if !filtered_gateway_success_rate_inputs.is_empty() {
                        (
                            new_gateway_score.clone(),
                            DOWNTIME,
                            sr_based_elimination_approach_info,
                        )
                    } else {
                        (
                            new_gateway_score.clone(),
                            NO_DOWNTIME,
                            sr_based_elimination_approach_info,
                        )
                    };

                let gateway_decider_approach =
                    Utils::modify_gateway_decider_approach(gateway_decider_approach, downtime);

                set_gwsm(gw_score.clone());
                Utils::set_elimination_scores(to_list_of_gateway_score(&gw_score));
                set_decider_approach(gateway_decider_approach);
                set_sr_elimination_approach_info(sr_based_elimination_approach_info_res);

                log_info_v(
                    "routing_approach",
                    format!("{:?}", gateway_decider_approach),
                );
            }
        }
    }

    let gateway_score_sr_based = get_gwsm();
    log_info_v(
        "GW_Scoring",
        format!(
            "Gateway scores after considering SR based elimination for {} : {:?}",
            review(ETTD::transaction_id_text(), &txn_detail.txn_id),
            to_list_of_gateway_score(&gateway_score_sr_based),
        ),
    );

    return_sm_with_log(UpdateGatewayScoreBasedOnSuccessRate, enable_success_rate_based_gateway_elimination.unwrap_or(false))
}


pub fn update_score_with_log(
    txn_id: ETTD::TransactionId,
    mut m: GatewayScoreMap,
    v: ETGRI::GatewayWiseSuccessRateBasedRoutingInput,
) -> GatewayScoreMap {
    let new_m = m.entry(v.gateway.clone()).and_modify(|score| *score /= 5);
    log_info_v::<String>(
        "scoringFlow",
        format!(
            "Penalizing gateway {} for {}",
            v.gateway, txn_id
        ),
    );
    m
}

pub fn get_merchant_elimination_gateway_score(
    i: RedisKey,
) -> DeciderFlow<Option<ETGRI::GatewayScore>> {
    RC::r_get(Config::ec_redis(), i)
}

pub fn update_current_score(
    gateway_redis_key_map: GatewayRedisKeyMap,
    i: ETGRI::GatewayWiseSuccessRateBasedRoutingInput,
) -> DeciderFlow<ETGRI::GatewayWiseSuccessRateBasedRoutingInput> {
    let redis_key = gateway_redis_key_map
        .get(&i.gateway.to_string())
        .unwrap_or(&String::new())
        .to_string();
    let txn_detail = asks(|ctx| ctx.dp_txn_detail.clone());
    let m_score = get_merchant_elimination_gateway_score(redis_key);
    log_info_t(
        "scoringFlow",
        format!(
            "Current score for {} {} : {:?} with elimination level {} threshold {}",
            review::<String>(ETTD::transaction_id_text(), txn_detail.txn_id),
            i.gateway,
            m_score.as_ref().map(|score| score.score),
            i.elimination_level,
            i.elimination_threshold
        ),
    );
    let updated_input = ETGRI::GatewayWiseSuccessRateBasedRoutingInput {
        current_score: m_score.as_ref().map(|score| score.score),
        last_reset_time_stamp: m_score
            .as_ref()
            .and_then(|score| score.last_reset_timestamp.map(|ts| ts as i64)),
        ..i
    };
    updated_input
}

pub fn log_final_gateways_scoring() -> DeciderFlow<GatewayScoreMap> {
    return_sm_with_log(FinalScoring, false)
}

pub fn get_gateway_success_based_routing_input(
    gw: ETG::Gateway,
    gateway_success_rate_merchant_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    default_success_rate_based_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    default_elimination_threshold: f64,
    default_soft_txn_reset_count: i64,
) -> DeciderFlow<ETGRI::GatewayWiseSuccessRateBasedRoutingInput> {
    let (merchant_given_default_elimination_level, merchant_given_default_threshold, merchant_given_default_gateway_level_sr_threshold, default_merchant_soft_txn_reset_count) =
        match gateway_success_rate_merchant_input {
            Some(val) => (
                Some(val.default_elimination_level),
                Some(val.default_elimination_threshold),
                val.default_gateway_level_elimination_threshold,
                val.default_global_soft_txn_reset_count,
            ),
            None => (None, None, None, None),
        };

    let gateway_success_based_routing_input = get_gateway_threshold_input_given_by_merchant(
        gateway_success_rate_merchant_input.clone(),
        gw.clone(),
    );

    let gateway_success_based_routing_input = gateway_success_based_routing_input.unwrap_or_else(|| {
        ETGRI::GatewayWiseSuccessRateBasedRoutingInput {
            gateway: gw.clone(),
            elimination_level: merchant_given_default_elimination_level
                .or_else(|| default_success_rate_based_routing_input.as_ref().map(|input| input.default_elimination_level))
                .unwrap_or(ETGRISelectionLevel::PAYMENT_METHOD),
            elimination_threshold: merchant_given_default_threshold
                .or_else(|| default_success_rate_based_routing_input.as_ref().map(|input| input.default_elimination_threshold))
                .unwrap_or(default_elimination_threshold),
            gateway_level_elimination_threshold: merchant_given_default_gateway_level_sr_threshold
                .or_else(|| extract_maybe(default_success_rate_based_routing_input.as_ref(), |input| input.default_gateway_level_elimination_threshold))
                .unwrap_or(C::def_sr_based_gw_level_elimination_threshold),
            soft_txn_reset_count: default_merchant_soft_txn_reset_count
                .or_else(|| extract_maybe(default_success_rate_based_routing_input.as_ref(), |input| input.default_global_soft_txn_reset_count))
                .unwrap_or(default_soft_txn_reset_count),
            elimination_max_count_threshold: None,
            current_score: None,
            last_reset_time_stamp: None,
            selection_max_count_threshold: None,
        }
    });

    gateway_success_based_routing_input
}

pub fn get_global_gateway_success_based_routing_input(
    gw: ETG::Gateway,
    default_success_rate_based_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    gateway_success_rate_merchant_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
) -> DeciderFlow<ETGRI::GatewayWiseSuccessRateBasedRoutingInput> {
    let global_gateway_success_based_routing_input_by_global_config =
        get_global_gateway_sr_input_given_by_global_config(default_success_rate_based_routing_input.clone(), gw.clone());
    let global_gateway_success_based_routing_input_by_merchant_config =
        get_global_gateway_sr_input_given_by_merchant_config(gateway_success_rate_merchant_input.clone(), gw.clone());

    let gateway_success_based_routing_input = ETGRI::GatewayWiseSuccessRateBasedRoutingInput {
        gateway: global_gateway_success_based_routing_input_by_merchant_config
            .as_ref()
            .and_then(|input| input.gateway.clone())
            .or_else(|| global_gateway_success_based_routing_input_by_global_config.as_ref().and_then(|input| input.gateway.clone()))
            .or_else(|| Some(gw.clone()))
            .unwrap_or(ETG::DEFAULT),
        elimination_level: extract_maybe(global_gateway_success_based_routing_input_by_merchant_config.as_ref(), |input| input.elimination_level)
            .or_else(|| extract_maybe(global_gateway_success_based_routing_input_by_global_config.as_ref(), |input| input.elimination_level))
            .or_else(|| extract_maybe(gateway_success_rate_merchant_input.as_ref(), |input| input.default_global_elimination_level))
            .or_else(|| extract_maybe(default_success_rate_based_routing_input.as_ref(), |input| input.default_global_elimination_level))
            .unwrap_or(ETGRISelectionLevel::PAYMENT_METHOD),
        selection_max_count_threshold: extract_maybe(global_gateway_success_based_routing_input_by_merchant_config.as_ref(), |input| input.selection_max_count_threshold)
            .or_else(|| extract_maybe(global_gateway_success_based_routing_input_by_global_config.as_ref(), |input| input.selection_max_count_threshold))
            .or_else(|| extract_maybe(gateway_success_rate_merchant_input.as_ref(), |input| input.default_global_selection_max_count_threshold))
            .or_else(|| extract_maybe(default_success_rate_based_routing_input.as_ref(), |input| input.default_global_selection_max_count_threshold))
            .unwrap_or(C::default_global_selection_max_count_threshold),
        elimination_threshold: None,
        elimination_max_count_threshold: None,
        soft_txn_reset_count: None,
        gateway_level_elimination_threshold: None,
        current_score: None,
        last_reset_time_stamp: None,
    };

    gateway_success_based_routing_input
}

fn extract_maybe<T, U>(src: Option<T>, func: impl Fn(&T) -> Option<U>) -> Option<U> {
    src.and_then(func)
}

fn get_gateway_threshold_input_given_by_merchant(
    gateway_success_rate_merchant_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    gateway: ETG::Gateway,
) -> Option<ETGRI::GatewayWiseSuccessRateBasedRoutingInput> {
    gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| input.gateway_wise_inputs.iter().find(|x| x.gateway == gateway))
        .cloned()
}

fn get_global_gateway_sr_input_given_by_global_config(
    default_success_rate_based_routing_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    gateway: ETG::Gateway,
) -> Option<ETGRI::GatewayWiseSuccessRateBasedRoutingInput> {
    default_success_rate_based_routing_input
        .as_ref()
        .and_then(|input| input.global_gateway_wise_inputs.iter().find(|x| x.gateway == gateway))
        .cloned()
}

fn get_global_gateway_sr_input_given_by_merchant_config(
    gateway_success_rate_merchant_input: Option<ETGRI::GatewaySuccessRateBasedRoutingInput>,
    gateway: ETG::Gateway,
) -> Option<ETGRI::GatewayWiseSuccessRateBasedRoutingInput> {
    gateway_success_rate_merchant_input
        .as_ref()
        .and_then(|input| input.global_gateway_wise_inputs.iter().find(|x| x.gateway == gateway))
        .cloned()
}


pub type GatewayScoreMap = HashMap<ETG::Gateway, f64>;

pub async fn update_gateway_score_based_on_gateway_level_scores(
    gateway_success_rate_inputs: Vec<GatewayWiseSuccessRateBasedRoutingInput>,
    gateway_score: GatewayScoreMap,
    is_reset_score_enabled_for_merchant: bool,
) -> DeciderFlow<(GatewayScoreMap, Vec<ETG::Gateway>, bool)> {
    let txn_detail = L::asks(|ctx| ctx.dp_txn_detail.clone()).await;
    let merchant = L::asks(|ctx| ctx.dp_merchant_account.clone()).await;

    let is_gateway_level_elimination_enabled = M::is_feature_enabled(
        C::ENABLE_GW_LEVEL_SR_ELIMINATION,
        Utils::get_m_id(&merchant.merchant_id),
        Config::KV_REDIS,
    )
    .await;

    let mut gateway_level_success_rate_inputs = Vec::new();
    for input in gateway_success_rate_inputs {
        let mut gateway_level_sr_input = GatewayWiseSuccessRateBasedRoutingInput {
            gateway: input.gateway.clone(),
            elimination_threshold: input.gateway_level_elimination_threshold.clone(),
            elimination_level: Some(SelectionLevel::Gateway),
            soft_txn_reset_count: input.soft_txn_reset_count.clone(),
            elimination_max_count_threshold: None,
            selection_max_count_threshold: None,
            gateway_level_elimination_threshold: None,
            current_score: None,
            last_reset_time_stamp: None,
        };

        let g_score: Option<GatewayScore> = RC::r_get(
            Config::EC_REDIS,
            get_merchant_gateway_level_score_key(&gateway_level_sr_input, &txn_detail, &merchant).await,
        )
        .await;

        gateway_level_sr_input.current_score = g_score.as_ref().map(|score| score.score);
        gateway_level_sr_input.last_reset_time_stamp = g_score
            .as_ref()
            .map(|score| score.last_reset_timestamp.map(|ts| ts as u64));

        log_info!(
            "scoringFlow",
            "Current score for {}, gateway: {}, score: {:?}, elimination level: {:?}, threshold: {:?}",
            txn_detail.txn_id,
            gateway_level_sr_input.gateway,
            gateway_level_sr_input.current_score,
            gateway_level_sr_input.elimination_level,
            gateway_level_sr_input.elimination_threshold
        );

        gateway_level_success_rate_inputs.push(gateway_level_sr_input);
    }

    let mut new_gateway_score = gateway_score.clone();

    reset_metric_log_data().await;
    let init_metric_log_data = L::gets(|ctx| ctx.sr_metric_log_data.clone()).await;
    L::set_metric_log_data(init_metric_log_data.clone()).await;

    let (new_gateway_score, reset_gateway_list) = if is_gateway_level_elimination_enabled {
        log_info!(
            "scoringFlow",
            "Gateway level elimination enabled for merchant {}",
            Utils::get_m_id(&txn_detail.merchant_id)
        );

        let filtered_gateway_success_rate_inputs: Vec<_> = gateway_level_success_rate_inputs
            .into_iter()
            .filter(|input| {
                if let (Some(curr_score), Some(elim_threshold)) = (&input.current_score, &input.elimination_threshold) {
                    curr_score < elim_threshold
                } else {
                    false
                }
            })
            .collect();

        if !filtered_gateway_success_rate_inputs.is_empty() {
            let mut updated_gateway_score = new_gateway_score.clone();
            for input in &filtered_gateway_success_rate_inputs {
                if let Some(val) = updated_gateway_score.get_mut(&input.gateway) {
                    *val /= 5.0;
                }
                log_info!(
                    "scoringFlow",
                    "Penalizing gateway {} for transaction {} based on Gateway SR Scores",
                    input.gateway,
                    txn_detail.txn_id
                );
            }

            let old_sr_metric_log_data = L::gets(|ctx| ctx.sr_metric_log_data.clone()).await;
            let sr_metric_log_data = old_sr_metric_log_data.clone();
            L::modify(|ctx| ctx.sr_metric_log_data = sr_metric_log_data.clone()).await;

            log_info!(
                "scoringFlow",
                "Gateway scores input after merchant wise Gateway level SR based evaluation for transaction {}: {:?}",
                txn_detail.txn_id,
                updated_gateway_score
            );

            if is_reset_score_enabled_for_merchant {
                let reset_enabled_gateway_list =
                    evaluate_reset_gateway_score(&filtered_gateway_success_rate_inputs, &txn_detail).await;
                if !reset_enabled_gateway_list.is_empty() {
                    (updated_gateway_score, reset_enabled_gateway_list)
                } else {
                    (updated_gateway_score, Vec::new())
                }
            } else {
                (updated_gateway_score, Vec::new())
            }
        } else {
            log_info!(
                "scoringFlow",
                "No gateways are eligible for penalties & fallback for transaction {}",
                txn_detail.txn_id
            );

            let old_sr_metric_log_data = L::gets(|ctx| ctx.sr_metric_log_data.clone()).await;
            let sr_metric_log_data = old_sr_metric_log_data.clone();
            L::modify(|ctx| ctx.sr_metric_log_data = sr_metric_log_data.clone()).await;

            (new_gateway_score.clone(), Vec::new())
        }
    } else {
        log_info!(
            "scoringFlow",
            "Gateway Level SR Elimination is not enabled for merchant {} and transaction {}",
            Utils::get_m_id(&txn_detail.merchant_id),
            txn_detail.txn_id
        );

        let old_sr_metric_log_data = L::gets(|ctx| ctx.sr_metric_log_data.clone()).await;
        let sr_metric_log_data = old_sr_metric_log_data.clone();
        L::modify(|ctx| ctx.sr_metric_log_data = sr_metric_log_data.clone()).await;

        (new_gateway_score.clone(), Vec::new())
    };

    let old_sr_metric_log_data = L::gets(|ctx| ctx.sr_metric_log_data.clone()).await;
    let gateway_level_sr_elimination = old_sr_metric_log_data.gateway_before_evaluation.is_some()
        && old_sr_metric_log_data.gateway_before_evaluation != old_sr_metric_log_data.gateway_after_evaluation;

    let sr_metric_log_data = old_sr_metric_log_data.clone();
    L::modify(|ctx| ctx.sr_metric_log_data = sr_metric_log_data.clone()).await;

    Utils::metric_tracker_log("SR_GATEWAY_EVALUATION", "GW_SCORING", Utils::get_metric_log_format("SR_GATEWAY_EVALUATION").await).await;

    Ok((new_gateway_score, reset_gateway_list, gateway_level_sr_elimination))
}

pub fn merchantGatewayScoreDimension(
    routingInput: GatewayWiseSuccessRateBasedRoutingInput,
) -> Dimension {
    match routingInput.eliminationLevel {
        Some(SelectionLevel::PAYMENT_METHOD_TYPE) => Dimension::SECOND,
        Some(SelectionLevel::PAYMENT_METHOD) => Dimension::THIRD,
        _ => Dimension::FIRST,
    }
}

pub fn sortGwScoreMap(oldGwSm: GatewayScoreMap) -> GatewayScoreMap {
    let mut sorted_list: Vec<_> = oldGwSm.into_iter().collect();
    sorted_list.sort_by(|(af, as_), (bf, bs)| {
        if as_ == bs {
            af.cmp(bf)
        } else {
            as_.cmp(bs)
        }
    });
    sorted_list.into_iter().collect()
}

pub async fn getKeyTTLFromMerchantDimension(
    dimension: Dimension,
) -> DeciderFlow<f64> {
    let mTtl: Option<f64> = match dimension {
        Dimension::FIRST => RService::findByNameFromRedis(C::gwScoreFirstDimensionTtl).await,
        Dimension::SECOND => RService::findByNameFromRedis(C::gwScoreSecondDimensionTtl).await,
        Dimension::THIRD => RService::findByNameFromRedis(C::gwScoreThirdDimensionTtl).await,
        Dimension::FOURTH => RService::findByNameFromRedis(C::gwScoreFourthDimensionTtl).await,
    };

    Ok(mTtl.unwrap_or(C::defScoreKeysTtl))
}

pub async fn evaluateResetGatewayScore(
    filteredGatewaySuccessRateInputs: Vec<GatewayWiseSuccessRateBasedRoutingInput>,
    txnDetail: ETTD::TxnDetail,
) -> DeciderFlow<Vec<ETG::Gateway>> {
    log_debug!(
        "evaluateResetGatewayScore",
        format!(
            "Evaluating Reset Logic for Gateways for {}",
            txnDetail.txnId
        )
    );

    let current_time: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    let mut acc: Vec<ETG::Gateway> = Vec::new();

    for it in filteredGatewaySuccessRateInputs {
        let key_ttl = getKeyTTLFromMerchantDimension(merchantGatewayScoreDimension(it.clone())).await?;
        if let Some(last_reset_time) = it.lastResetTimeStamp {
            if (current_time * 1000 - last_reset_time as i64) > key_ttl.round() as i64 {
                log_debug!(
                    "evaluateResetGatewayScore",
                    format!(
                        "Adding gateway {} to resetAPI Request for {} for level {:?}",
                        it.gateway, txnDetail.txnId, it.eliminationLevel
                    )
                );
                acc.push(it.gateway.clone());
            }
        }
    }

    Ok(acc)
}


pub fn trigger_reset_gateway_score(
    gateway_success_rate_inputs: Vec<GatewayWiseSuccessRateBasedRoutingInput>,
    txn_detail: ETTD::TxnDetail,
    reset_gateway_list: Vec<ETG::Gateway>,
    is_reset_score_enabled_for_merchant: bool,
) -> DeciderFlow<()> {
    log_info_t("scoringFlow", format!("Triggering Reset for Gateways for {:?}", reset_gateway_list));
    if is_reset_score_enabled_for_merchant {
        log_info_v::<String>(
            "scoringFlow",
            format!(
                "Reset Gateway Scores is enabled for {:?} and merchantId {:?}",
                txn_detail.txn_id,
                Utils::get_m_id(txn_detail.merchant_id)
            ),
        );
        let reset_gateway_sr_list = reset_gateway_list.iter().fold(Vec::new(), |mut acc, it| {
            log_info_v::<String>(
                "scoringFlow",
                format!(
                    "Adding gateway {:?} to resetAPI Request for {:?}",
                    it, txn_detail.txn_id
                ),
            );
            let m_sr_input = get_gateway_success_rate_input(it, &gateway_success_rate_inputs);
            let oref = asks(|ctx| ctx.dp_order.clone());
            let macc = asks(|ctx| ctx.dp_merchant_account.clone());
            let (meta, pl_ref_id_map) =
                Utils::get_order_metadata_and_pl_ref_id_map(macc.enable_gateway_reference_id_based_routing, oref);
            match m_sr_input {
                Some(sr_input) => {
                    let gw_ref_id = Utils::get_gateway_reference_id(meta, it, oref, pl_ref_id_map);
                    let reset_gateway_input = ResetGatewayInput {
                        gateway: it.clone(),
                        elimination_threshold: sr_input.elimination_threshold,
                        elimination_max_count: sr_input.soft_txn_reset_count.map(|v| v as i64),
                        gateway_elimination_threshold: sr_input.gateway_level_elimination_threshold,
                        gateway_reference_id: gw_ref_id.map(|id| ETM::un_mga_reference_id(id)),
                    };
                    acc.push(reset_gateway_input);
                }
                None => {
                    log_info_v::<String>(
                        "scoringFlow",
                        format!("No SR Input for {:?} and {:?}", it, txn_detail.txn_id),
                    );
                }
            }
            acc
        });

        let reset_approach = Utils::get_reset_approach();
        match reset_approach {
            SRV2_RESET => Utils::set_reset_approach(SRV2_ELIMINATION_RESET),
            SRV3_RESET => Utils::set_reset_approach(SRV3_ELIMINATION_RESET),
            _ => Utils::set_reset_approach(ELIMINATION_RESET),
        }
        log_info_v::<String>("RESET_APPROACH", format!("{:?}", reset_approach));
        log_info_v::<String>(
            "scoringFlow",
            format!(
                "Reset Gateway List for {:?} is {:?}",
                txn_detail.txn_id, reset_gateway_sr_list
            ),
        );
        reset_gateway_score(txn_detail, reset_gateway_sr_list);
    } else {
        log_info_v::<String>(
            "scoringFlow",
            format!(
                "Reset Gateway Scores is not enabled for {:?} and merchantId {:?}",
                txn_detail.txn_id,
                Utils::get_m_id(txn_detail.merchant_id)
            ),
        );
    }
}

fn get_gateway_success_rate_input<'a>(
    gw: &'a ETG::Gateway,
    gateway_success_rate_inputs: &'a [GatewayWiseSuccessRateBasedRoutingInput],
) -> Option<&'a GatewayWiseSuccessRateBasedRoutingInput> {
    gateway_success_rate_inputs.iter().find(|it| it.gateway == *gw)
}

pub fn reset_gateway_score(
    txn_detail: ETTD::TxnDetail,
    reset_gateway_sr_list: Vec<ResetGatewayInput>,
) -> DeciderFlow<()> {
    if !reset_gateway_sr_list.is_empty() {
        let endpoint = ENV::euler_endpoint();
        let params = ResetCallParams {
            txn_detail_id: txn_detail.id.to_string(),
            txn_id: review(ETTD::transaction_id_text(), txn_detail.txn_id.clone()),
            merchant_id: Utils::get_m_id(txn_detail.merchant_id.clone()),
            order_id: Ord::un_order_id(txn_detail.order_id.clone()),
            reset_gateway_score_req_arr: reset_gateway_sr_list,
        };
        log_debug_v::<String>(
            "resetGatewayScore",
            format!(
                "Reset score call to Euler with Endpoint: {:?}, params: {:?}, for {:?}",
                endpoint, params, txn_detail.txn_id
            ),
        );
        let url = Client::parse_base_url(endpoint.as_str()).unwrap();
        let m_cell_selector = language::get_option_local::<Options::XCellSelectorHeader>();
        language::call_api(
            Some(T::ManagerSelector::TlsManager),
            url,
            EC_RESET_GATEWAY_SCORE,
            |_| None,
            reset_gw_score_call(Some("HS".to_string()), m_cell_selector, params),
        );
    } else {
        log_debug_v::<String>(
            "resetGatewayScore",
            format!("Not eligible to send reset gateway score callback {:?}", txn_detail.txn_id),
        );
    }
}


fn reset_gw_score_call(param1: Option<Text>, param2: Option<Text>, params: ResetCallParams) -> T::EulerClient<A::Value> {
    T::client::<ResetGWScoreAPI>(param1, param2, params)
}

pub fn route_random_traffic(
    gws: GatewayScoreMap,
    hedging_percent: f64,
    is_sr_v3_metric_enabled: bool,
    tag: String,
) -> DeciderFlow<GatewayScoreMap> {
    let num = language::random_rio(
        format!("GatewayDecider::routeRandomTraffic::{}", tag),
        (0.0, 100.0),
    );
    language::log_debug_t("RandomNumber", format!("{:?}", num));
    let sorted_gw_list: Vec<_> = gws.iter().map(|(k, v)| (k.clone(), *v)).collect();
    let sorted_gw_list = sorted_gw_list.into_iter().sorted_by(|a, b| Ord::cmp(&b.1, &a.1));
    let (head_gateway, remaining_gateways) = sorted_gw_list.split_at(1);
    if num < hedging_percent * (remaining_gateways.len() as f64) {
        let remaining_gateways: Vec<_> = remaining_gateways
            .iter()
            .map(|(gw, _)| (gw.clone(), 1.0))
            .collect();
        let head_gateways: Vec<_> = head_gateway.iter().map(|(gw, _)| (gw.clone(), 0.5)).collect();
        language::log_debug_t(
            "Gateway Scores After Route Random Traffic Feature",
            format!("{:?}", remaining_gateways.iter().chain(head_gateways.iter()).collect::<Vec<_>>()),
        );
        let is_primary_gateway = Some(false);
        if is_sr_v3_metric_enabled {
            set_decider_approach(SR_V3_HEDGING);
        } else {
            set_decider_approach(SR_V2_HEDGING);
        }
        Ok(remaining_gateways.into_iter().chain(head_gateways.into_iter()).collect())
    } else {
        language::log_debug_t("Selection Based Routing Gateways SR", format!("{:?}", sorted_gw_list));
        Ok(sorted_gw_list.into_iter().collect())
    }
}