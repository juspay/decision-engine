use super::runner::get_gateway_priority;
use super::types::RankingAlgorithm;
use super::types::UnifiedError;
use axum::response::IntoResponse;
use diesel::expression::is_aggregate::No;
use crate::app::get_tenant_app_state;
use serde_json::json;
use serde_json::Value as AValue;
use std::collections::HashMap;
use std::option::Option;
use std::string::String;
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
use super::utils::is_card_transaction;
use super::utils::is_emandate_transaction;
use super::utils::is_mandate_transaction;
use super::utils::is_tpv_mandate_transaction;
use super::utils::is_tpv_transaction;
use crate::decider::storage::utils::txn_card_info::is_google_pay_txn;
use crate::types::card::card_type::card_type_to_text;
use crate::types::card::card_type::CardType;
use crate::types::card::txn_card_info::AuthType;
// use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::card::vault_provider::VaultProvider;
// use optics_core::{preview, review};
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::gateway::Gateway;
use crate::types::gateway_card_info::ValidationType;
use crate::types::merchant as ETM;
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;
use crate::types::payment::payment_method::PaymentMethodType;
use crate::types::txn_details::types as ETTD;
use crate::decider::gatewaydecider::constants as C;

pub async fn deciderFullPayloadHSFunction(
    dreq_: T::DomainDeciderRequestForApiCallV2,
) -> Result<(T::DecidedGateway), T::ErrorResponse> {
    let merchant_prefs = match ETM::merchant_iframe_preferences::getMerchantIPrefsByMId(
        dreq_.merchantId.clone(),
    ).await
    {
        Some(prefs) => prefs,
        None => {
            Err(T::ErrorResponse {
                status: "400".to_string(),
                error_code: "DATA_NOT_FOUND".to_string(),
                error_message: "merchant iframe preferences not found".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "MERCHANT_IFRAME_PREFERENCES_NOT_FOUND".to_string(),
                    user_message:
                        "merchant iframe preferences not found with the given merchant id"
                            .to_string(),
                    developer_message: "merchant iframe preferences not found".to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            })?
            // L::logErrorV("getMerchantPrefsByMId", format!("Merchant iframe preferences not found for id: {}", dreq.txnDetail.merchantId));
        }
    };
    let enforced_gateway_filter = handleEnforcedGateway(dreq_.clone().eligibleGatewayList);
    let dreq = dreq_.to_domain_decider_request().await;  
    let resolve_bin = match Utils::fetch_extended_card_bin(&dreq.txnCardInfo.clone()) {
        Some(card_bin) => Some(card_bin),
        None => match dreq.txnCardInfo.card_isin {
            Some(c_isin) => {
                let res_bin = Utils::get_card_bin_from_token_bin(6, c_isin.as_str()).await;
                Some(res_bin)
            }
            None => dreq.txnCardInfo.card_isin.clone(),
        },
    };
    // L::logDebugV("resolveBin of txnCardInfo", resolve_bin.clone());
    let m_vault_provider = Utils::get_vault_provider(dreq.cardToken.as_deref());
    let update_txn_card_info = TxnCardInfo {
        card_isin: resolve_bin,
        ..dreq.txnCardInfo
    };
    //
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
    };
    runDeciderFlow(decider_params, dreq_.clone().rankingAlgorithm, dreq_.clone().eliminationEnabled).await
}

fn handleEnforcedGateway(gateway_list: Option<Vec<Gateway>>) -> Option<Vec<Gateway>> {
    match gateway_list {
        None => None,
        Some(list) if list.is_empty() => None,
        list => list,
    }
}

pub async fn runDeciderFlow(
    deciderParams: T::DeciderParams,
    rankingAlgorithm: Option<RankingAlgorithm>,
    eliminationEnabled: Option<bool>,
) -> Result<(T::DecidedGateway), T::ErrorResponse> {
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
    decider_flow.writer.gateway_scoring_data = Utils::get_gateway_scoring_data(&mut decider_flow, deciderParams.dpTxnDetail.clone(), deciderParams.dpTxnCardInfo.clone(), deciderParams.dpMerchantAccount.clone()).await;
    let (functionalGateways) = deciderParams.dpEnforceGatewayList.clone().unwrap_or_default();
    println!(
        "Gateway filtered list: {:?}",
        decider_flow.writer.debugFilterList
    );

    // L::logInfoV("GW_Filtering", &sortedFilterList(&deciderState.debugFilterList)).await;

    let preferredGateway = deciderParams
        .dpTxnDetail
        .gateway
        .clone()
        .or(deciderParams.dpOrder.preferredGateway.clone());
   // let gatewayMgaIdMap = getGatewayToMGAIdMapF(&allMgas, &functionalGateways);

    // L::logInfoT("PreferredGateway", &format!(
    //     "Preferred gateway provided by merchant for {} = {}",
    //     transactionIdText(&deciderParams.dpTxnDetail.txnId),
    //     preferredGateway.map_or("None".to_string(), |pgw| pgw.to_string())
    // )).await;

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
                    T::GatewayDeciderApproach::MERCHANT_PREFERENCE,
                    None,
                    functionalGateways,
                    None,
                )
                .await;
                Ok(T::DecidedGateway {
                    decided_gateway: pgw.clone(),
                    gateway_priority_map: Some(json!(HashMap::from([(pgw.to_string(), 1.0)]))),
                    filter_wise_gateways: None,
                    priority_logic_tag: None,
                    routing_approach: T::GatewayDeciderApproach::MERCHANT_PREFERENCE,
                    gateway_before_evaluation: Some(pgw.clone()),
                    priority_logic_output: None,
                    reset_approach: T::ResetApproach::NO_RESET,
                    routing_dimension: None,
                    routing_dimension_level: None,
                    is_scheduled_outage: false,
                    is_dynamic_mga_enabled: decider_flow.writer.is_dynamic_mga_enabled,
                    gateway_mga_id_map: None,
                })
            } else {
                decider_flow
                    .writer
                    .debugFilterList
                    .push(T::DebugFilterEntry {
                        filterName: "preferredGateway".to_string(),
                        gateways: vec![],
                    });
                // L::logWarningV("PreferredGateway", &format!(
                //     "Preferred gateway {} functional/valid for merchant {} in txn {}",
                //     pgw,
                //     deciderParams.dpMerchantAccount.merchantId,
                //     deciderParams.dpTxnDetail.txnId
                // )).await;
                Utils::log_gateway_decider_approach(
                    &mut decider_flow,
                    None,
                    None,
                    Vec::new(),
                    T::GatewayDeciderApproach::NONE,
                    None,
                    functionalGateways,
                    None,
                )
                .await;
                Err((
                    decider_flow.writer.debugFilterList.clone(),
                    decider_flow.writer.debugScoringList.clone(),
                    None,
                    T::GatewayDeciderApproach::NONE,
                    None,
                    decider_flow.writer.is_dynamic_mga_enabled,
                ))
            }
        }
        _ => {
            if rankingAlgorithm == Some(RankingAlgorithm::SR_BASED_ROUTING) {
                
            }
            let gwPLogic = if rankingAlgorithm != Some(RankingAlgorithm::SR_BASED_ROUTING) {
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
                            ).await
                        }
                    }
                } else {
                    T::GatewayPriorityLogicOutput {
                        gws: functionalGateways.clone(),
                        isEnforcement: false,
                        priorityLogicTag: None,
                        primaryLogic : None,
                        gatewayReferenceIds: HashMap::new(),
                        fallbackLogic: None,
                    }
                };

            let gatewayPriorityList =
                addPreferredGatewaysToPriorityList(gwPLogic.gws.clone(), preferredGateway.clone());
            // L::logInfoV("gatewayPriorityList", &format!(
            //     "Gateway priority for merchant for {} = {:?}",
            //     transactionIdText(&deciderParams.dpTxnDetail.txnId),
            //     gatewayPriorityList
            // )).await;

            let (mut functionalGateways, updatedPriorityLogicOutput) = if gwPLogic.isEnforcement {
                // L::logInfoT("gatewayPriorityList", &format!(
                //     "Enforcing Priority Logic for {}",
                //     transactionIdText(&deciderParams.dpTxnDetail.txnId)
                // )).await;
                let (res, priorityLogicOutput) = filterFunctionalGatewaysWithEnforcment(
                    &mut decider_flow,
                    &functionalGateways,
                    &gatewayPriorityList,
                    &gwPLogic,
                    preferredGateway,
                )
                .await;
                // L::logInfoT("gatewayPriorityList", &format!(
                //     "Functional gateways after filtering for Enforcement Logic for {} : {:?}",
                //     transactionIdText(&deciderParams.dpTxnDetail.txnId),
                //     res
                // )).await;
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
            // L::logInfoV("PriorityLogicOutput", &updatedPriorityLogicOutput).await;
            // L::logInfoT("GW_Filtering", &format!(
            //     "Functional gateways after {} for {} : {:?}",
            //     T::FilterByPriorityLogic,
            //     transactionIdText(&deciderParams.dpTxnDetail.txnId),
            //     uniqueFunctionalGateways
            // )).await;

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

            // L::logInfoV("GW_Scoring", &st.debugScoringList.iter().map(|scoreData| {
            //     (scoreData.scoringName.clone(), scoreData.gatewayScores.clone())
            // }).collect::<HashMap<_, _>>()).await;

            let scoreList = currentGatewayScoreMap.iter().collect::<Vec<_>>();
            // L::logDebugT("scoreList", &format!("{:?}", scoreList)).await;

            let gatewayPriorityMap = Some(json!(scoreList
                .iter()
                .map(|(gw, score)| { (gw.to_string(), *score) })
                .collect::<HashMap<_, _>>()));

            match scoreList.as_slice() {
                [] => Err((
                    decider_flow.writer.debugFilterList.clone(),
                    decider_flow.writer.debugScoringList.clone(),
                    updatedPriorityLogicOutput.priorityLogicTag.clone(),
                    T::GatewayDeciderApproach::NONE,
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
                    // L::logDebugT("decidedGateway after randomGatewaySelectionForSameScore", &format!("{:?}", decidedGateway)).await;

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

                    // L::logInfoT("Decided Gateway", &format!(
                    //     "Gateway decided for {} = {:?}",
                    //     transactionIdText(&deciderParams.dpTxnDetail.txnId),
                    //     decidedGateway
                    // )).await;

                    // addMetricsToStream(
                    //     Some(decidedGateway.as_ref()),
                    //     finalDeciderApproach.clone(),
                    //     updatedPriorityLogicOutput.priorityLogicTag.clone(),
                    //     &st,
                    //     &deciderParams,
                    //     &currentGatewayScoreMap
                    // ).await?;

                    // L::logInfoV("GATEWAY_PRIORITY_MAP", &gatewayPriorityMap).await;

                    match decidedGateway {
                        Some(decideGatewayOutput) => Ok(T::DecidedGateway {
                            decided_gateway: decideGatewayOutput,
                            gateway_priority_map: gatewayPriorityMap,
                            filter_wise_gateways: None,
                            priority_logic_tag: updatedPriorityLogicOutput.priorityLogicTag.clone(),
                            routing_approach: finalDeciderApproach.clone(),
                            gateway_before_evaluation: topGatewayBeforeSRDowntimeEvaluation.clone(),
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
                        }),
                        None => Err((
                            decider_flow.writer.debugFilterList.clone(),
                            decider_flow.writer.debugScoringList.clone(),
                            updatedPriorityLogicOutput.priorityLogicTag.clone(),
                            finalDeciderApproach.clone(),
                            Some(updatedPriorityLogicOutput),
                            decider_flow.writer.is_dynamic_mga_enabled,
                        )),
                    }
                }
            }
        }
    };
    let key = [C::gatewayScoringData, &deciderParams.dpTxnDetail.txnUuid.clone()].concat();
    let updated_gateway_scoring_data = T::GatewayScoringData {routingApproach: Some(decider_flow.writer.gwDeciderApproach.clone().to_string()), ..decider_flow.writer.gateway_scoring_data.clone() };
    let app_state = get_tenant_app_state().await;
        app_state.redis_conn.setx(&key, serde_json::json!(updated_gateway_scoring_data.clone()).as_str().unwrap_or_default(), C::gatewayScoreKeysTTL).await.unwrap_or_default();
        updated_gateway_scoring_data;
    match dResult {
        Ok(result) => Ok((
            result
        )),
        Err((
            debugFilterList,
            _,
            priorityLogicTag,
            finalDeciderApproach,
            priorityLogicOutput,
            isDynamicMGAEnabled,
        )) => {

            Err(T::ErrorResponse {
                status: "Invalid Request".to_string(),
                error_code: "invalid_request_error".to_string(),
                error_message: "Can't find a suitable gateway to process the transaction"
                    .to_string(),
                priority_logic_tag: priorityLogicTag,
                routing_approach: Some(finalDeciderApproach),
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "GATEWAY_NOT_FOUND".to_string(),
                    user_message: "Gateway not found to process the transaction request."
                        .to_string(),
                    developer_message: "Gateway not found to process the transaction request."
                        .to_string(),
                },
                priority_logic_output: priorityLogicOutput,
                is_dynamic_mga_enabled: isDynamicMGAEnabled,
            })
        }
    }
}

fn getGatewayToMGAIdMapF(allMgas: &Vec<MerchantGatewayAccount>, gateways: &Vec<Gateway>) -> AValue {
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

fn addPreferredGatewaysToPriorityList(
    gwPriority: Vec<Gateway>,
    preferredGatewayM: Option<Gateway>,
) -> Vec<Gateway> {
    match preferredGatewayM {
        None => gwPriority,
        Some(pgw) => {
            let mut list = gwPriority;
            list.retain(|gw| *gw != pgw);
            list.insert(0, pgw);
            list
        }
    }
}

async fn filterFunctionalGatewaysWithEnforcment(
    decider_flow: &mut T::DeciderFlow<'_>,
    fGws: &[Gateway],
    priorityGws: &[Gateway],
    plOp: &T::GatewayPriorityLogicOutput,
    preferredGw: Option<Gateway>,
) -> (Vec<Gateway>, T::GatewayPriorityLogicOutput) {
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
            PriorityLogicFailure::NULL_AFTER_ENFORCE,
        )
        .await;
        let fallBackGwPriority =
            addPreferredGatewaysToPriorityList(updatedPlOp.gws.clone(), preferredGw);
        if updatedPlOp.isEnforcement {
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
                    PriorityLogicFailure::NULL_AFTER_ENFORCE,
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

fn defaultDecidedGateway(
    gw: Gateway,
    gpm: Option<AValue>,
    priorityLogicTag: Option<String>,
    finalDeciderApproach: T::GatewayDeciderApproach,
    topGatewayBeforeSRDowntimeEvaluation: Option<Gateway>,
    priorityLogicOutput: Option<T::GatewayPriorityLogicOutput>,
    resetApproach: T::ResetApproach,
    routingDimension: Option<String>,
    routingDimensionLevel: Option<String>,
    isScheduledOutage: bool,
    isDynamicMGAEnabled: bool,
) -> T::DecidedGateway {
    T::DecidedGateway {
        decided_gateway: gw,
        gateway_priority_map: gpm,
        filter_wise_gateways: None,
        priority_logic_tag: priorityLogicTag,
        routing_approach: finalDeciderApproach,
        gateway_before_evaluation: topGatewayBeforeSRDowntimeEvaluation,
        priority_logic_output: priorityLogicOutput,
        reset_approach: resetApproach,
        routing_dimension: routingDimension,
        routing_dimension_level: routingDimensionLevel,
        is_scheduled_outage: isScheduledOutage,
        is_dynamic_mga_enabled: isDynamicMGAEnabled,
        gateway_mga_id_map: None,
    }
}

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

