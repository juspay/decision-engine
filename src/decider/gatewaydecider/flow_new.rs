use super::runner::get_gateway_priority;
use super::types::RankingAlgorithm;
use super::types::UnifiedError;
use crate::app::get_tenant_app_state;
use crate::decider::network_decider;
use serde_json::json;
use serde_json::Value as AValue;
use std::collections::HashMap;
use std::option::Option;
use std::string::String;
use std::time::Instant;
use std::vec::Vec;
// use eulerhs::prelude::*;
// use eulerhs::language as L;
// use eulerhs::framework as Framework;
// use gatewaydecider::flow::*;
use super::gw_scoring as GS;
use super::runner::handle_fallback_logic;
use super::types as T;
use super::types::PriorityLogicFailure;
use super::utils as Utils;
// use optics_core::{preview, review};
use crate::decider::gatewaydecider::constants as C;
use crate::logger;
use crate::redis::feature::RedisDataStruct;
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::merchant as ETM;
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;

pub async fn decider_full_payload_hs_function(
    dreq_: T::DomainDeciderRequestForApiCallV2,
    cpu_start: Instant,
) -> Result<T::DecidedGateway, T::ErrorResponse> {
    let merchant_account =
        ETM::merchant_account::load_merchant_by_merchant_id(dreq_.merchant_id.clone())
            .await
            .ok_or(T::ErrorResponse {
                status: "Invalid Request".to_string(),
                error_code: "invalid_request_error".to_string(),
                error_message: "Merchant not found".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "MERCHANT_NOT_FOUND".to_string(),
                    user_message: "Merchant not found".to_string(),
                    developer_message: "Merchant not found".to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            })?;
    let enforced_gateway_filter = handle_enforced_gateway(dreq_.clone().eligible_gateway_list);

    // check if type formation is correct
    let merchant_prefs = ETM::merchant_iframe_preferences::MerchantIframePreferences {
        id: ETM::merchant_iframe_preferences::to_merchant_iframe_prefs_pid(
            crate::types::merchant::id::merchant_pid_to_text(merchant_account.id.clone()),
        ),
        merchantId: merchant_account.merchantId.clone(),
        dynamicSwitchingEnabled: enforced_gateway_filter
            .as_ref()
            .map(|list| !(list.len() <= 1))
            .unwrap_or(false),
        isinRoutingEnabled: false,
        issuerRoutingEnabled: false,
        txnFailureGatewayPenality: false,
        cardBrandRoutingEnabled: false,
    };

    let dreq = dreq_.to_domain_decider_request().await;
    let resolve_bin = match Utils::fetch_extended_card_bin(&dreq.txnCardInfo.clone()) {
        Some(card_bin) => Some(card_bin),
        None => match dreq.txnCardInfo.card_isin {
            Some(c_isin) => {
                let res_bin = Utils::get_card_bin_from_token_bin(6, c_isin.as_str(), None).await;
                Some(res_bin)
            }
            None => dreq.txnCardInfo.card_isin.clone(),
        },
    };
    logger::debug!(
        action = "resolveBin of txnCardInfo",
        tag = "resolveBin of txnCardInfo",
        "{:?}",
        resolve_bin.clone()
    );
    let m_vault_provider = Utils::get_vault_provider(dreq.cardToken.as_deref());
    let update_txn_card_info = TxnCardInfo {
        card_isin: resolve_bin,
        ..dreq.txnCardInfo
    };

    let decider_params = T::DeciderParams {
        dpMerchantAccount: dreq.merchantAccount,
        dpOrder: dreq.orderReference,
        dpTxnDetail: dreq.txnDetail,
        dpTxnOfferDetails: dreq.txnOfferDetails,
        dpTxnCardInfo: update_txn_card_info,
        dpTxnOfferInfo: None,
        dpVaultProvider: m_vault_provider,
        dpTxnType: dreq.txnType,
        dpMerchantPrefs: merchant_prefs,
        dpOrderMetadata: dreq.orderMetadata,
        dpEnforceGatewayList: enforced_gateway_filter,
        dpPriorityLogicOutput: dreq.priorityLogicOutput,
        dpPriorityLogicScript: dreq.priorityLogicScript,
        dpEDCCApplied: dreq.isEdccApplied,
        dpIsOnUsTxn: dreq.isOnUsTxn,
        dpShouldConsumeResult: dreq.shouldConsumeResult,
        dpRedisCompressionConfig: None,
    };

    if dreq_.ranking_algorithm == Some(RankingAlgorithm::NtwBasedRouting) {
        logger::debug!("Performing debit routing");
        network_decider::debit_routing::perform_debit_routing(dreq_).await
    } else {
        logger::debug!("Performing gateway routing");
        run_decider_flow(
            decider_params,
            dreq_.clone().ranking_algorithm,
            dreq_.clone().elimination_enabled,
            false,
            cpu_start,
        )
        .await
    }
}

fn handle_enforced_gateway(gateway_list: Option<Vec<String>>) -> Option<Vec<String>> {
    match gateway_list {
        None => None,
        Some(list) if list.is_empty() => None,
        list => list,
    }
}

pub async fn run_decider_flow(
    deciderParams: T::DeciderParams,
    rankingAlgorithm: Option<RankingAlgorithm>,
    eliminationEnabled: Option<bool>,
    is_legacy_decider_flow: bool,
    cpu_start: Instant,
) -> Result<T::DecidedGateway, T::ErrorResponse> {
    let txnCreationTime = deciderParams
        .dpTxnDetail
        .dateCreated
        .clone()
        .to_string()
        .replace(" ", "T")
        .replace(" UTC", "Z");
    let mut deciderState = T::initial_decider_state(txnCreationTime.clone());
    let mut logger = HashMap::new();

    let mut decider_flow =
        T::initial_decider_flow(deciderParams.clone(), &mut logger, &mut deciderState).await; // TODO: Check if this is correct & changes decider state
    decider_flow.writer.gateway_scoring_data = Utils::get_gateway_scoring_data(
        &mut decider_flow,
        deciderParams.dpTxnDetail.clone(),
        deciderParams.dpTxnCardInfo.clone(),
        deciderParams.dpMerchantAccount.clone(),
        is_legacy_decider_flow.clone(),
    )
    .await;
    let functionalGateways = deciderParams
        .dpEnforceGatewayList
        .clone()
        .unwrap_or_default();

    let preferredGateway = deciderParams
        .dpTxnDetail
        .gateway
        .clone()
        .or(deciderParams.dpOrder.preferredGateway.clone());
    // let gatewayMgaIdMap = getGatewayToMGAIdMapF(&allMgas, &functionalGateways);

    logger::warn!(
        action = "PreferredGateway",
        tag = "PreferredGateway",
        "Preferred gateway provided by merchant for {:?} = {:?}",
        &deciderParams.dpTxnDetail.txnId,
        preferredGateway
            .clone()
            .map_or("None".to_string(), |pgw| pgw.to_string())
    );

    let dResult = match (
        preferredGateway.clone(),
        deciderParams
            .dpMerchantPrefs
            .dynamicSwitchingEnabled
            .clone(),
    ) {
        (Some(pgw), false) => {
            if functionalGateways.contains(&pgw) {
                Utils::log_gateway_decider_approach(
                    &mut decider_flow,
                    Some(pgw.clone()),
                    None,
                    Vec::new(),
                    T::GatewayDeciderApproach::MerchantPreference,
                    None,
                    functionalGateways,
                    None,
                )
                .await;
                let cpu_time = cpu_start.elapsed().as_millis() as u64;
                Ok(T::DecidedGateway {
                    decided_gateway: pgw.clone(),
                    gateway_priority_map: Some(json!(HashMap::from([(pgw.to_string(), 1.0)]))),
                    filter_wise_gateways: None,
                    priority_logic_tag: None,
                    routing_approach: T::GatewayDeciderApproach::MerchantPreference,
                    gateway_before_evaluation: Some(pgw.clone()),
                    priority_logic_output: None,
                    reset_approach: T::ResetApproach::NoReset,
                    routing_dimension: None,
                    routing_dimension_level: None,
                    is_scheduled_outage: false,
                    is_dynamic_mga_enabled: decider_flow.writer.is_dynamic_mga_enabled,
                    gateway_mga_id_map: None,
                    debit_routing_output: None,
                    is_rust_based_decider: true,
                    latency: Some(cpu_time),
                })
            } else {
                decider_flow
                    .writer
                    .debugFilterList
                    .push(T::DebugFilterEntry {
                        filterName: "preferredGateway".to_string(),
                        gateways: vec![],
                    });
                logger::info!(
                    action = "PreferredGateway",
                    tag = "PreferredGateway",
                    "Preferred gateway {:?} functional/valid for merchant {:?} in txn {:?}",
                    pgw,
                    &deciderParams.dpMerchantAccount.merchantId,
                    deciderParams.dpTxnDetail.txnId
                );
                Utils::log_gateway_decider_approach(
                    &mut decider_flow,
                    None,
                    None,
                    Vec::new(),
                    T::GatewayDeciderApproach::None,
                    None,
                    functionalGateways,
                    None,
                )
                .await;
                Err((
                    decider_flow.writer.debugFilterList.clone(),
                    decider_flow.writer.debugScoringList.clone(),
                    None,
                    T::GatewayDeciderApproach::None,
                    None,
                    decider_flow.writer.is_dynamic_mga_enabled,
                ))
            }
        }
        _ => {
            let gwPLogic = if rankingAlgorithm != Some(RankingAlgorithm::SrBasedRouting) {
                match deciderParams.dpPriorityLogicOutput {
                    Some(ref plOp) => plOp.clone(),
                    None => {
                        get_gateway_priority(
                            deciderParams.dpMerchantAccount.clone(),
                            deciderParams.dpOrder.clone(),
                            deciderParams.dpTxnDetail.clone(),
                            deciderParams.dpTxnCardInfo.clone(),
                            decider_flow.writer.internalMetaData.clone(),
                            deciderParams.dpOrderMetadata.metadata.clone(),
                            deciderParams.dpPriorityLogicScript.clone(),
                        )
                        .await
                    }
                }
            } else {
                T::GatewayPriorityLogicOutput {
                    gws: functionalGateways.clone(),
                    is_enforcement: false,
                    priority_logic_tag: None,
                    primary_logic: None,
                    gateway_reference_ids: HashMap::new(),
                    fallback_logic: None,
                }
            };

            let gatewayPriorityList = add_preferred_gateways_to_priority_list(
                gwPLogic.gws.clone(),
                preferredGateway.clone(),
            );
            logger::info!(
                tag = "gatewayPriorityList",
                action = "gatewayPriorityList",
                "Gateway priority for merchant for {:?} = {:?}",
                &deciderParams.dpTxnDetail.txnId,
                gatewayPriorityList
            );

            let (mut functionalGateways, updatedPriorityLogicOutput) = if gwPLogic.is_enforcement {
                logger::info!(
                    tag = "gatewayPriorityList",
                    action = "Enforcing Priority Logic",
                    "Enforcing Priority Logic for {:?}",
                    deciderParams.dpTxnDetail.txnId
                );
                let (res, priorityLogicOutput) = filter_functional_gateways_with_enforcement(
                    &mut decider_flow,
                    &functionalGateways,
                    &gatewayPriorityList,
                    &gwPLogic,
                    preferredGateway,
                )
                .await;
                logger::info!(
                    tag = "gatewayPriorityList",
                    action = "gatewayPriorityList",
                    "Functional gateways after filtering for Enforcement Logic for {:?} : {:?}",
                    &deciderParams.dpTxnDetail.txnId,
                    res
                );
                decider_flow
                    .writer
                    .debugFilterList
                    .push(T::DebugFilterEntry {
                        filterName: "filterEnforcement".to_string(),
                        gateways: res.clone(),
                    });
                (res, priorityLogicOutput)
            } else {
                (functionalGateways.clone(), gwPLogic)
            };

            // uniqueFunctionalGateways should have unique gateways
            functionalGateways.dedup();
            let uniqueFunctionalGateways = functionalGateways.clone();
            logger::info!(
                tag = "PriorityLogicOutput",
                action = "PriorityLogicOutput",
                "{:?}",
                updatedPriorityLogicOutput
            );
            logger::info!(
                tag = "GW_Filtering",
                action = "GW_Filtering",
                "Functional gateways after {:?} for {:?} : {:?}",
                "FilterByPriorityLogic",
                &deciderParams.dpTxnDetail.txnId,
                uniqueFunctionalGateways
            );

            // let currentGatewayScoreMap = GS::get_score_with_priority(
            //     uniqueFunctionalGateways.clone(),
            //     updatedPriorityLogicOutput.gws.clone(),
            // );

            let currentGatewayScoreMap = GS::scoring_flow(
                &mut decider_flow,
                uniqueFunctionalGateways.clone(),
                updatedPriorityLogicOutput.gws.clone(),
                rankingAlgorithm,
                eliminationEnabled,
            )
            .await;

            logger::info!(
                tag = "GW_Scoring",
                action = "GW_Scoring",
                "{:?}",
                &decider_flow
                    .writer
                    .debugScoringList
                    .iter()
                    .map(|scoreData| {
                        (
                            scoreData.scoringName.clone(),
                            scoreData.gatewayScores.clone(),
                        )
                    })
                    .collect::<HashMap<_, _>>()
            );

            let scoreList = currentGatewayScoreMap.iter().collect::<Vec<_>>();
            logger::debug!(action = "scoreList", tag = "scoreList", "{:?}", scoreList);

            let gatewayPriorityMap = Some(json!(scoreList
                .iter()
                .map(|(gw, score)| { (gw.to_string(), *score) })
                .collect::<HashMap<_, _>>()));

            match scoreList.as_slice() {
                [] => Err((
                    decider_flow.writer.debugFilterList.clone(),
                    decider_flow.writer.debugScoringList.clone(),
                    updatedPriorityLogicOutput.priority_logic_tag.clone(),
                    T::GatewayDeciderApproach::None,
                    Some(updatedPriorityLogicOutput),
                    decider_flow.writer.is_dynamic_mga_enabled,
                )),
                gs => {
                    let maxScore = Utils::get_max_score_gateway(&currentGatewayScoreMap)
                        .map(|(gw, score)| score);
                    let decidedGateway = Utils::random_gateway_selection_for_same_score(
                        &currentGatewayScoreMap,
                        maxScore,
                    );
                    logger::debug!(
                        action = "decidedGateway after randomGatewaySelectionForSameScore",
                        tag = "decidedGateway after randomGatewaySelectionForSameScore",
                        "{:?}",
                        decidedGateway
                    );

                    let stateBindings = (
                        decider_flow.writer.srElminiationApproachInfo.clone(),
                        decider_flow.writer.isOptimizedBasedOnSRMetricEnabled,
                        decider_flow.writer.isSrV3MetricEnabled,
                        decider_flow
                            .writer
                            .topGatewayBeforeSRDowntimeEvaluation
                            .clone(),
                        decider_flow.writer.isPrimaryGateway,
                        decider_flow.writer.experiment_tag.clone(),
                    );

                    let (
                        srEliminationInfo,
                        isOptimizedBasedOnSRMetricEnabled,
                        isSrV3MetricEnabled,
                        topGatewayBeforeSRDowntimeEvaluation,
                        isPrimaryGateway,
                        experimentTag,
                    ) = stateBindings;

                    let finalDeciderApproach = Utils::get_gateway_decider_approach(
                        &currentGatewayScoreMap,
                        decider_flow.writer.gwDeciderApproach.clone(),
                    );
                    Utils::log_gateway_decider_approach(
                        &mut decider_flow,
                        decidedGateway.clone(),
                        topGatewayBeforeSRDowntimeEvaluation.clone(),
                        srEliminationInfo,
                        finalDeciderApproach.clone(),
                        isPrimaryGateway,
                        uniqueFunctionalGateways,
                        experimentTag,
                    )
                    .await;

                    logger::info!(
                        action = "Decided Gateway",
                        tag = "Decided Gateway",
                        "Gateway decided for {:?} = {:?}",
                        &deciderParams.dpTxnDetail.txnId,
                        decidedGateway
                    );

                    // addMetricsToStream(
                    //     Some(decidedGateway.as_ref()),
                    //     finalDeciderApproach.clone(),
                    //     updatedPriorityLogicOutput.priorityLogicTag.clone(),
                    //     &st,
                    //     &deciderParams,
                    //     &currentGatewayScoreMap
                    // ).await?;

                    if let Some(ref priority_map) = gatewayPriorityMap {
                        logger::debug!(
                            action = "GATEWAY_PRIORITY_MAP",
                            tag = "GATEWAY_PRIORITY_MAP",
                            gateway_priority_map = %priority_map
                        );
                    }

                    match decidedGateway {
                        Some(decideGatewayOutput) => {
                            let cpu_time = cpu_start.elapsed().as_millis() as u64;
                            Ok(T::DecidedGateway {
                                decided_gateway: decideGatewayOutput,
                                gateway_priority_map: gatewayPriorityMap,
                                filter_wise_gateways: None,
                                priority_logic_tag: updatedPriorityLogicOutput
                                    .priority_logic_tag
                                    .clone(),
                                routing_approach: finalDeciderApproach.clone(),
                                gateway_before_evaluation: topGatewayBeforeSRDowntimeEvaluation
                                    .clone(),
                                priority_logic_output: Some(updatedPriorityLogicOutput),
                                reset_approach: decider_flow.writer.reset_approach.clone(),
                                routing_dimension: decider_flow.writer.routing_dimension.clone(),
                                routing_dimension_level: decider_flow
                                    .writer
                                    .routing_dimension_level
                                    .clone(),
                                is_scheduled_outage: decider_flow.writer.isScheduledOutage,
                                is_dynamic_mga_enabled: decider_flow.writer.is_dynamic_mga_enabled,
                                gateway_mga_id_map: None,
                                debit_routing_output: None,
                                is_rust_based_decider: true,
                                latency: Some(cpu_time),
                            })
                        }
                        None => Err((
                            decider_flow.writer.debugFilterList.clone(),
                            decider_flow.writer.debugScoringList.clone(),
                            updatedPriorityLogicOutput.priority_logic_tag.clone(),
                            finalDeciderApproach.clone(),
                            Some(updatedPriorityLogicOutput),
                            decider_flow.writer.is_dynamic_mga_enabled,
                        )),
                    }
                }
            }
        }
    };
    let key = [
        C::GATEWAY_SCORING_DATA,
        &deciderParams.dpTxnDetail.txnUuid.clone(),
    ]
    .concat();
    let updated_gateway_scoring_data = T::GatewayScoringData {
        routingApproach: Some(decider_flow.writer.gwDeciderApproach.clone().to_string()),
        eliminationEnabled: eliminationEnabled.unwrap_or_default(),
        is_legacy_decider_flow,
        udfs: Some(deciderParams.dpOrder.udfs.clone()),
        ..decider_flow.writer.gateway_scoring_data.clone()
    };
    let app_state = get_tenant_app_state().await;
    app_state
        .redis_conn
        .setx(
            &key,
            serde_json::to_string(&updated_gateway_scoring_data.clone())
                .unwrap_or_default()
                .as_str(),
            C::GATEWAY_SCORE_KEYS_TTL,
            None,
            RedisDataStruct::STRING,
        )
        .await
        .unwrap_or_default();
    updated_gateway_scoring_data;
    match dResult {
        Ok(result) => Ok(result),
        Err((
            debugFilterList,
            _,
            priorityLogicTag,
            finalDeciderApproach,
            priorityLogicOutput,
            isDynamicMGAEnabled,
        )) => Err(T::ErrorResponse {
            status: "Invalid Request".to_string(),
            error_code: "invalid_request_error".to_string(),
            error_message: "Can't find a suitable gateway to process the transaction".to_string(),
            priority_logic_tag: priorityLogicTag,
            routing_approach: Some(finalDeciderApproach),
            filter_wise_gateways: None,
            error_info: UnifiedError {
                code: "GATEWAY_NOT_FOUND".to_string(),
                user_message: "Gateway not found to process the transaction request.".to_string(),
                developer_message: "Gateway not found to process the transaction request."
                    .to_string(),
            },
            priority_logic_output: priorityLogicOutput,
            is_dynamic_mga_enabled: isDynamicMGAEnabled,
        }),
    }
}

#[allow(dead_code)]
fn get_gateway_to_mga_id_map_f(
    allMgas: &Vec<MerchantGatewayAccount>,
    gateways: &Vec<String>,
) -> AValue {
    json!(gateways
        .iter()
        .map(|x| {
            (
                x.to_string(),
                allMgas
                    .iter()
                    .find(|mga| mga.gateway == *x)
                    .map(|mga| mga.id.merchantGwAccId.clone()),
            )
        })
        .collect::<HashMap<_, _>>())
}

fn add_preferred_gateways_to_priority_list(
    gwPriority: Vec<String>,
    preferredGateway: Option<String>,
) -> Vec<String> {
    match preferredGateway {
        None => gwPriority,
        Some(pgw) => {
            let mut list = gwPriority;
            list.retain(|gw| *gw != pgw);
            list.insert(0, pgw);
            list
        }
    }
}

async fn filter_functional_gateways_with_enforcement(
    decider_flow: &mut T::DeciderFlow<'_>,
    fGws: &[String],
    priorityGws: &[String],
    plOp: &T::GatewayPriorityLogicOutput,
    preferredGw: Option<String>,
) -> (Vec<String>, T::GatewayPriorityLogicOutput) {
    let enforcedGateways = fGws
        .iter()
        .filter(|&gw| priorityGws.contains(gw))
        .cloned()
        .collect::<Vec<_>>();
    if enforcedGateways.is_empty() && decider_flow.get().dpPriorityLogicOutput.is_none() {
        let mCardInfo =
            Utils::get_card_info_by_bin(decider_flow.get().dpTxnCardInfo.card_isin.clone()).await;
        let updatedPlOp = handle_fallback_logic(
            decider_flow.get().dpMerchantAccount.clone(),
            decider_flow.get().dpOrder.clone(),
            decider_flow.get().dpTxnDetail.clone(),
            decider_flow.get().dpTxnCardInfo.clone(),
            mCardInfo.clone(),
            decider_flow.writer.internalMetaData.clone(),
            decider_flow.get().dpOrderMetadata.metadata.clone(),
            plOp.clone(),
            PriorityLogicFailure::NullAfterEnforce,
        )
        .await;
        let fallBackGwPriority =
            add_preferred_gateways_to_priority_list(updatedPlOp.gws.clone(), preferredGw);
        if updatedPlOp.is_enforcement {
            let updatedEnforcedGateways = fGws
                .iter()
                .filter(|&gw| fallBackGwPriority.contains(gw))
                .cloned()
                .collect::<Vec<_>>();
            if updatedEnforcedGateways.is_empty() {
                let updatedPlOp = handle_fallback_logic(
                    decider_flow.get().dpMerchantAccount.clone(),
                    decider_flow.get().dpOrder.clone(),
                    decider_flow.get().dpTxnDetail.clone(),
                    decider_flow.get().dpTxnCardInfo.clone(),
                    mCardInfo.clone(),
                    decider_flow.writer.internalMetaData.clone(),
                    decider_flow.get().dpOrderMetadata.metadata.clone(),
                    updatedPlOp,
                    PriorityLogicFailure::NullAfterEnforce,
                )
                .await;
                (updatedEnforcedGateways, updatedPlOp)
            } else {
                (updatedEnforcedGateways, updatedPlOp)
            }
        } else {
            (fGws.to_vec(), updatedPlOp)
        }
    } else {
        (enforcedGateways, plOp.clone())
    }
}

// fn makeFirstLetterSmall(s: &str) -> String {
//     let mut chars = s.chars();
//     match chars.next() {
//         None => String::new(),
//         Some(f) => f.to_lowercase().collect::<String>() + chars.as_str(),
//     }
// }

// async fn addMetricsToStream(
//     decidedGateway: Option<&Gateway>,
//     finalDeciderApproach: T::RoutingApproach,
//     mPriorityLogicTag: Option<String>,
//     st: &T::DeciderState,
//     deciderParams: &T::DeciderParams,
//     currentGatewayScoreMap: &HashMap<Gateway, f64>
// ) -> Result<(), Box<dyn std::error::Error>> {
//     Utils::pushToStream(
//         decidedGateway,
//         finalDeciderApproach,
//         mPriorityLogicTag,
//         currentGatewayScoreMap,
//         deciderParams,
//         st
//     ).await
// }
