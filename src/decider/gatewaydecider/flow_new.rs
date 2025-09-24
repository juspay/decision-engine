use super::runner::get_gateway_priority;
use super::types::RankingAlgorithm;
use super::types::UnifiedError;
use crate::app::get_tenant_app_state;
use crate::decider::network_decider;
use axum::response::IntoResponse;
use diesel::expression::is_aggregate::No;
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
use crate::decider::gatewaydecider::constants as C;
use crate::decider::network_decider::types as NetworkTypes;
use crate::logger;
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::gateway_card_info::ValidationType;
use crate::types::merchant as ETM;
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;
use crate::types::txn_details::types as ETTD;

pub async fn deciderFullPayloadHSFunction(
    dreq_: T::DomainDeciderRequestForApiCallV2,
) -> Result<(T::DecidedGateway), T::ErrorResponse> {
    let merchant_account =
        ETM::merchant_account::load_merchant_by_merchant_id(dreq_.merchantId.clone())
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
    let enforced_gateway_filter = handleEnforcedGateway(dreq_.clone().eligibleGatewayList);

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
                let res_bin = Utils::get_card_bin_from_token_bin(6, c_isin.as_str()).await;
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
        dpShouldConsumeResult: dreq.shouldConsumeResult,
    };

    if dreq_.rankingAlgorithm == Some(RankingAlgorithm::NTW_BASED_ROUTING) {
        logger::debug!("Performing debit routing");
        network_decider::debit_routing::perform_debit_routing(dreq_).await
    } else if dreq_.rankingAlgorithm == Some(RankingAlgorithm::SUPER_ROUTER) {
        logger::debug!("Performing SUPER_ROUTER routing");
        runSuperRouterFlow(decider_params, dreq_.clone()).await
    } else {
        logger::debug!("Performing gateway routing");
        runDeciderFlow(
            decider_params,
            dreq_.clone().rankingAlgorithm,
            dreq_.clone().eliminationEnabled,
            false,
        )
        .await
    }
}

fn handleEnforcedGateway(gateway_list: Option<Vec<String>>) -> Option<Vec<String>> {
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
    is_legacy_decider_flow: bool,
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
    decider_flow.writer.gateway_scoring_data = Utils::get_gateway_scoring_data(
        &mut decider_flow,
        deciderParams.dpTxnDetail.clone(),
        deciderParams.dpTxnCardInfo.clone(),
        deciderParams.dpMerchantAccount.clone(),
    )
    .await;
    let (functionalGateways) = deciderParams
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
                    debit_routing_output: None,
                    super_router: None,
                    reset_approach: T::ResetApproach::NO_RESET,
                    routing_dimension: None,
                    routing_dimension_level: None,
                    is_scheduled_outage: false,
                    is_dynamic_mga_enabled: decider_flow.writer.is_dynamic_mga_enabled,
                    gateway_mga_id_map: None,
                    is_rust_based_decider: true,
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
                        )
                        .await
                    }
                }
            } else {
                T::GatewayPriorityLogicOutput {
                    gws: functionalGateways.clone(),
                    isEnforcement: false,
                    priorityLogicTag: None,
                    primaryLogic: None,
                    gatewayReferenceIds: HashMap::new(),
                    fallbackLogic: None,
                }
            };

            let gatewayPriorityList =
                addPreferredGatewaysToPriorityList(gwPLogic.gws.clone(), preferredGateway.clone());
            logger::info!(
                tag = "gatewayPriorityList",
                action = "gatewayPriorityList",
                "Gateway priority for merchant for {:?} = {:?}",
                &deciderParams.dpTxnDetail.txnId,
                gatewayPriorityList
            );

            let (mut functionalGateways, updatedPriorityLogicOutput) = if gwPLogic.isEnforcement {
                logger::info!(
                    tag = "gatewayPriorityList",
                    action = "Enforcing Priority Logic",
                    "Enforcing Priority Logic for {:?}",
                    deciderParams.dpTxnDetail.txnId
                );
                let (res, priorityLogicOutput) = filterFunctionalGatewaysWithEnforcment(
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

                    logger::info!(
                        action = "GATEWAY_PRIORITY_MAP",
                        tag = "GATEWAY_PRIORITY_MAP",
                        "{:?}",
                        gatewayPriorityMap
                    );

                    match decidedGateway {
                        Some(decideGatewayOutput) => Ok(T::DecidedGateway {
                            decided_gateway: decideGatewayOutput,
                            gateway_priority_map: gatewayPriorityMap,
                            filter_wise_gateways: None,
                            priority_logic_tag: updatedPriorityLogicOutput.priorityLogicTag.clone(),
                            routing_approach: finalDeciderApproach.clone(),
                            gateway_before_evaluation: topGatewayBeforeSRDowntimeEvaluation.clone(),
                            priority_logic_output: Some(updatedPriorityLogicOutput),
                            debit_routing_output: None,
                            super_router: None,
                            reset_approach: decider_flow.writer.reset_approach.clone(),
                            routing_dimension: decider_flow.writer.routing_dimension.clone(),
                            routing_dimension_level: decider_flow
                                .writer
                                .routing_dimension_level
                                .clone(),
                            is_scheduled_outage: decider_flow.writer.isScheduledOutage,
                            is_dynamic_mga_enabled: decider_flow.writer.is_dynamic_mga_enabled,
                            gateway_mga_id_map: None,
                            is_rust_based_decider: true,
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
    let key = [
        C::gatewayScoringData,
        &deciderParams.dpTxnDetail.txnUuid.clone(),
    ]
    .concat();
    let updated_gateway_scoring_data = T::GatewayScoringData {
        routingApproach: Some(decider_flow.writer.gwDeciderApproach.clone().to_string()),
        eliminationEnabled: eliminationEnabled.unwrap_or_default(),
        is_legacy_decider_flow,
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
            C::gatewayScoreKeysTTL,
        )
        .await
        .unwrap_or_default();
    updated_gateway_scoring_data;
    match dResult {
        Ok(result) => Ok((result)),
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

fn getGatewayToMGAIdMapF(allMgas: &Vec<MerchantGatewayAccount>, gateways: &Vec<String>) -> AValue {
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

async fn filterFunctionalGatewaysWithEnforcment(
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
    gw: String,
    gpm: Option<AValue>,
    priorityLogicTag: Option<String>,
    finalDeciderApproach: T::GatewayDeciderApproach,
    topGatewayBeforeSRDowntimeEvaluation: Option<String>,
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
        debit_routing_output: None,
        super_router: None,
        reset_approach: resetApproach,
        routing_dimension: routingDimension,
        routing_dimension_level: routingDimensionLevel,
        is_scheduled_outage: isScheduledOutage,
        is_dynamic_mga_enabled: isDynamicMGAEnabled,
        gateway_mga_id_map: None,
        is_rust_based_decider: true,
    }
}

pub async fn runSuperRouterFlow(
    decider_params: T::DeciderParams,
    dreq: T::DomainDeciderRequestForApiCallV2,
) -> Result<T::DecidedGateway, T::ErrorResponse> {
    logger::debug!("Starting SUPER_ROUTER flow");

    let app_state = get_tenant_app_state().await;
    let card_isin = decider_params.dpTxnCardInfo.card_isin.clone();
    let amount = dreq.paymentInfo.amount;

    // Get networks to process
    let networks_to_process = if let Some(card_isin_value) = card_isin {
        logger::debug!("Card ISIN present, calling sorted_networks_by_absolute_fee");

        // Create CoBadgedCardRequest from the request metadata
        if let Some(metadata_value) = dreq
            .paymentInfo
            .metadata
            .map(|metadata_string| Utils::parse_json_from_string(&metadata_string))
            .flatten()
        {
            match TryInto::<NetworkTypes::CoBadgedCardRequest>::try_into(metadata_value) {
                Ok(co_badged_card_request) => {
                    if let Some(debit_routing_output) = co_badged_card_request
                        .sorted_networks_by_absolute_fee(&app_state, Some(card_isin_value), amount)
                        .await
                    {
                        let mut network_savings_info_for_super_router = Vec::new();
                        for network_savings_info in
                            debit_routing_output.co_badged_card_networks_info
                        {
                            network_savings_info_for_super_router.push(
                                NetworkTypes::NetworkSavingInfoForSuperRouter {
                                    network: network_savings_info.network.to_string(),
                                    saving_percentage: network_savings_info.saving_percentage,
                                },
                            );
                        }
                        network_savings_info_for_super_router
                    } else {
                        logger::warn!("Failed to get networks from sorted_networks_by_absolute_fee, using paymentMethod");
                        vec![NetworkTypes::NetworkSavingInfoForSuperRouter {
                            network: dreq.paymentInfo.paymentMethod,
                            saving_percentage: 0.0,
                        }]
                    }
                }
                Err(error) => {
                    logger::error!("Failed to parse metadata for SUPER_ROUTER: {:?}", error);
                    vec![NetworkTypes::NetworkSavingInfoForSuperRouter {
                        network: dreq.paymentInfo.paymentMethod,
                        saving_percentage: 0.0,
                    }]
                }
            }
        } else {
            logger::warn!("No metadata found, using paymentMethod");
            vec![NetworkTypes::NetworkSavingInfoForSuperRouter {
                network: dreq.paymentInfo.paymentMethod,
                saving_percentage: 0.0,
            }]
        }
    } else {
        logger::debug!("Card ISIN not present, using paymentMethod with 0 savings");
        vec![NetworkTypes::NetworkSavingInfoForSuperRouter {
            network: dreq.paymentInfo.paymentMethod,
            saving_percentage: 0.0,
        }]
    };

    logger::debug!(
        "Networks to process for SUPER_ROUTER before normalization: {:?}",
        networks_to_process
    );

    // normalize the cost savings within range [0, 1]
    let networks_to_process = network_decider::helpers::normalize_cost_savings(networks_to_process);

    logger::debug!(
        "Networks to process for SUPER_ROUTER: {:?}",
        networks_to_process
    );

    let mut super_router_priority_map = Vec::new();
    let mut first_gateway_result: Option<T::DecidedGateway> = None;

    // Process each network
    for network_info in networks_to_process {
        logger::debug!("Processing network: {:?}", network_info.network);

        // Create mutable copy of decider_params and update network
        let mut modified_decider_params = decider_params.clone();

        // Update the network in sourceObject field of txnDetail
        modified_decider_params.dpTxnDetail.sourceObject = Some(network_info.network.to_string());

        // Update the network in paymentMethod field of dpTxnCardInfo
        modified_decider_params.dpTxnCardInfo.paymentMethod = network_info.network.to_string();

        // Call runDeciderFlow for this network
        match runDeciderFlow(
            modified_decider_params,
            Some(RankingAlgorithm::SR_BASED_ROUTING), // Use SR_BASED_ROUTING for individual network processing
            dreq.eliminationEnabled,
            false,
        )
        .await
        {
            Ok(decided_gateway) => {
                // Store the first successful result as the main gateway decision
                if first_gateway_result.is_none() {
                    first_gateway_result = Some(decided_gateway.clone());
                }
                logger::debug!("run decider flow: {:?}", decided_gateway);

                // Extract gateway_priority_map and construct super_router output
                if let Some(gateway_priority_map) = decided_gateway.gateway_priority_map {
                    if let Ok(priority_map) =
                        serde_json::from_value::<HashMap<String, f64>>(gateway_priority_map)
                    {
                        for (gateway, score) in priority_map {
                            super_router_priority_map.push(T::SUPERROUTERPRIORITYMAP {
                                gateway,
                                payment_method: network_info.network.to_string(),
                                success_rate: Some(score), // Using the score as success_rate
                                saving: Some(network_info.saving_percentage),
                                combined_score: Some(0.0), // Set to 0 as requested
                            });
                        }
                    }
                }
            }
            Err(error) => {
                logger::warn!(
                    "Failed to get gateway decision for network {:?}: {:?}",
                    network_info.network,
                    error
                );
                // Continue with other networks even if one fails
            }
        }
    }

    // Sort the priority_map by success_rate in descending order
    // super_router_priority_map.sort_by(|a, b| {
    //     let success_rate_a = a.success_rate.unwrap_or(0.0);
    //     let success_rate_b = b.success_rate.unwrap_or(0.0);
    //     success_rate_b.total_cmp(&success_rate_a)
    // });
    // logger::debug!(
    //     "Completed processing all networks in SUPER_ROUTER flow {:?}",
    //     super_router_priority_map
    // );

    // Sort the super_router_priority_map using Euclidean distance
    let sorted_priority_map =
        super::sr_cost_routing::sort_by_euclidean_distance_original(&mut super_router_priority_map);

    // Check if there are any combinations
    if sorted_priority_map.is_empty() {
        logger::error!("No suitable gateway combinations found after sorting");
        return Err(T::ErrorResponse {
            status: "No suitable gateway found".to_string(),
            error_code: "no_gateway_found".to_string(),
            error_message: "No suitable gateway found for routing".to_string(),
            priority_logic_tag: None,
            routing_approach: Some(T::GatewayDeciderApproach::SUPER_ROUTER),
            filter_wise_gateways: None,
            error_info: T::UnifiedError {
                code: "NO_GATEWAY_FOUND".to_string(),
                user_message: "No suitable gateway found for routing".to_string(),
                developer_message: "No suitable gateway found for routing".to_string(),
            },
            priority_logic_output: None,
            is_dynamic_mga_enabled: false,
        });
    }

    // Get the best combination (first in the sorted list)
    let best_entry = &sorted_priority_map[0];
    let best_gateway = best_entry.gateway.clone();
    let best_network = best_entry.payment_method.clone();
    let best_score = best_entry.success_rate.unwrap_or(0.0);
    let best_cost = best_entry.saving.unwrap_or(0.0);
    let best_distance = best_entry.combined_score.unwrap_or(0.0);

    logger::info!(
        tag = "Best_Gateway",
        action = "Best_Gateway",
        "Best Gateway: {}, Network: {}, Score: {}, Cost: {}, Distance: {}",
        best_gateway,
        best_network,
        best_score,
        best_cost,
        best_distance
    );

    logger::debug!(
        "Sorted super_router_priority_map by success_rate: {:?}",
        super_router_priority_map
    );

    // Return the result
    match first_gateway_result {
        Some(mut gateway_result) => {
            // Add super_router output to the result with the sorted priority map
            gateway_result.super_router = Some(T::SUPERROUTEROUTPUT {
                priority_map: sorted_priority_map,
            });
            gateway_result.gateway_priority_map = None;
            // Update the decided gateway to use the best one from our sorted list
            gateway_result.decided_gateway = best_gateway;
            gateway_result.routing_approach = T::GatewayDeciderApproach::SUPER_ROUTER;

            logger::debug!("SUPER_ROUTER flow completed successfully");
            Ok(gateway_result)
        }
        None => {
            logger::error!(
                "No successful gateway decision found for any network in SUPER_ROUTER flow"
            );
            Err(T::ErrorResponse {
                status: "Invalid Request".to_string(),
                error_code: "invalid_request_error".to_string(),
                error_message:
                    "Can't find a suitable gateway to process the transaction using SUPER_ROUTER"
                        .to_string(),
                priority_logic_tag: None,
                routing_approach: Some(T::GatewayDeciderApproach::SUPER_ROUTER),
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "SUPER_ROUTER_GATEWAY_NOT_FOUND".to_string(),
                    user_message: "No gateway found using SUPER_ROUTER algorithm".to_string(),
                    developer_message: "No gateway found using SUPER_ROUTER algorithm".to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            })
        }
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
