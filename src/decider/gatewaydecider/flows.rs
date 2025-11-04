use super::runner::get_gateway_priority;
use super::types::UnifiedError;
use axum::response::IntoResponse;
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
use super::gw_filter as GF;
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
use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::constants as C;
use crate::logger;
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::gateway_card_info::ValidationType;
use crate::types::merchant as ETM;
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;
use crate::types::payment::payment_method_type_const::*;
use crate::types::txn_details::types as ETTD;
// use utils::errors::predefined_errors as Errs;
// use juspay::extra::parsing::{Parsed, parse};
// use juspay::extra::secret::{SecretContext, makeSecret};
// use juspay::extra::json as JSON;
// use juspay::extra::non_empty_text as NE;

// pub fn deciderFullPayloadEndpoint(
//     headers: Framework::Headers,
//     session_id: String,
//     request: T::ApiDeciderFullRequest,
// ) -> impl L::MonadFlow<Framework::Response> {
//     L::setLoggerContext("x-request-id", getSessionId(&headers, &session_id));
//     let response = deciderFullPayloadEndpointParser(request.captures);
//     match response {
//         Ok(gw) => Framework::json(gw),
//         Err(err) => Framework::jsonWithCode(400, err),
//     }
// }

// fn getSessionId(headers: &Framework::Headers, session_id: &String) -> String {
//     match Framework::getHeaderValue("x-request-id", headers) {
//         Some(id) => id,
//         None => session_id.clone(),
//     }
// }

// pub fn deciderFullPayloadEndpointParser(
//     request: T::ApiDeciderRequest,
// ) -> impl L::MonadFlow<Result<T::DecidedGateway, T::ErrorResponse>> {
//     let parsed_request = V::parseApiDeciderRequest(request);
//     match parsed_request {
//         Parsed::Result(req) => {
//             let merchant_acc = match ETM::loadMerchantByMerchantId(req.orderReference.merchantId) {
//                 Some(acc) => acc,
//                 None => {
// logger::error!(
//     tag = "getMaccByMerchantId",
//     "Merchant account for id: {}",
//     req.orderReference.merchantId
// );
//                     L::throwException(Errs::internalError(
//                         Some("merchant account with the given merchant id not found."),
//                         Some("merchant account with the given merchant id not found."),
//                         None,
//                     ));
//                 }
//             };
//             L::setLoggerContext("merchant_id", Utils::getMId(req.orderReference.merchantId));
//             if let Some(tenant_id) = merchant_acc.tenantAccountId {
//                 L::setLoggerContext("tenant_id", tenant_id);
//             }
//             L::setLoggerContext("txn_uuid", req.txnDetail.txnUuid);
//             L::setLoggerContext("order_id", req.orderReference.orderId.unOrderId);
//             L::setLoggerContext("txn_creation_time", req.txnDetail.dateCreated.to_string());
//             let resolve_bin = match Utils::fetchExtendedCardBin(req.txnCardInfo) {
//                 Some(card_bin) => Some(card_bin),
//                 None => match req.txnCardInfo.cardIsin {
//                     Some(c_isin) => {
//                         let res_bin = Utils::getCardBinFromTokenBin(6, c_isin);
//                         Some(res_bin)
//                     }
//                     None => req.txnCardInfo.cardIsin,
//                 },
//             };
// logger::debug!(
//     tag = "resolveBin of txnCardInfo",
//     "{:?}",
//     resolve_bin.clone()
// );
//             let request = T::transformRequest(req, merchant_acc, resolve_bin);
// logger::debug!(
//     tag = "enforeced gateway list",
//     "{}",
//     request.enforceGatewayList.to_string()
// );
//             let decider_response = deciderFullPayloadHSFunction(request);
//             decider_response
//         }
//         Parsed::Failed(err) => T::handleLeftCase(err.to_string()),
//     }
// }

pub trait ResponseDecider {
    type DecidedGateway: IntoResponse;
    type ErrorResponse: IntoResponse;
}

pub async fn decider_full_payload_hs_function(
    dreq: T::DomainDeciderRequest,
) -> Result<(T::DecidedGateway, Vec<(String, Vec<String>)>), T::ErrorResponse> {
    let merchant_prefs = match ETM::merchant_iframe_preferences::getMerchantIPrefsByMId(
        dreq.txn_detail.merchant_id.0.clone(),
    )
    .await
    {
        Some(prefs) => prefs,
        None => {
            logger::error!(
                tag = "getMerchantPrefsByMId",
                action = "getMerchantPrefsByMId",
                "Merchant iframe preferences not found for id: {:?}",
                dreq.txn_detail.merchant_id
            );
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
        }
    };
    let enforced_gateway_filter = handle_enforced_gateway(dreq.enforce_gateway_list);
    let resolve_bin = match Utils::fetch_extended_card_bin(&dreq.txn_card_info.clone()) {
        Some(card_bin) => Some(card_bin),
        None => match dreq.txn_card_info.card_isin {
            Some(c_isin) => {
                let res_bin = Utils::get_card_bin_from_token_bin(6, c_isin.as_str()).await;
                Some(res_bin)
            }
            None => dreq.txn_card_info.card_isin.clone(),
        },
    };
    logger::debug!(
        action = "resolveBin of txnCardInfo",
        tag = "resolveBin of txnCardInfo",
        "{:?}",
        resolve_bin.clone()
    );
    let m_vault_provider = Utils::get_vault_provider(dreq.card_token.as_deref());
    let update_txn_card_info = TxnCardInfo {
        card_isin: resolve_bin,
        ..dreq.txn_card_info
    };

    let decider_params = T::DeciderParams {
        dpMerchantAccount: dreq.merchant_account,
        dpOrder: dreq.order_reference,
        dpTxnDetail: dreq.txn_detail,
        dpTxnOfferDetails: dreq.txn_offer_details,
        dpTxnCardInfo: update_txn_card_info,
        dpTxnOfferInfo: None,
        dpVaultProvider: m_vault_provider,
        dpTxnType: dreq.txn_type,
        dpMerchantPrefs: merchant_prefs,
        dpOrderMetadata: dreq.order_metadata,
        dpEnforceGatewayList: enforced_gateway_filter,
        dpPriorityLogicOutput: dreq.priority_logic_output,
        dpPriorityLogicScript: dreq.priority_logic_script,
        dpEDCCApplied: dreq.is_edcc_applied,
        dpShouldConsumeResult: dreq.should_consume_result,
    };
    run_decider_flow(decider_params, true).await
}

fn handle_enforced_gateway(gateway_list: Option<Vec<String>>) -> Option<Vec<String>> {
    match gateway_list {
        None => None,
        Some(list) if list.is_empty() => None,
        list => list,
    }
}

// pub async fn getSupportedGws(
//     dreq: T::DomainDeciderRequest,
// ) -> impl L::MonadFlow<Result<(Vec<Gateway>, Vec<(Gateway, String)>), (String, Vec<(Gateway, String)>)>> {
//     let merchant_prefs = match ETM::getMerchantIPrefsByMId(dreq.txnDetail.merchantId).await {
//         Some(prefs) => prefs,
//         None => {
// logger::error!(
//     tag = "getMerchantPrefsByMId",
//     "Merchant iframe preferences not found for id: {}",
//     dreq.txnDetail.merchantId
// );
//             L::throwException(Errs::internalError(
//                 Some("merchant iframe preferences not found"),
//                 Some("merchant iframe preferences not found with the given merchant id"),
//                 None,
//             ));
//         }
//     };
//     let enforced_gateway_filter = handleEnforcedGateway(dreq.enforceGatewayList);
//     let resolve_bin = match Utils::fetchExtendedCardBin(dreq.txnCardInfo) {
//         Some(card_bin) => Some(card_bin),
//         None => match dreq.txnCardInfo.cardIsin {
//             Some(c_isin) => {
//                 let res_bin = Utils::getCardBinFromTokenBin(6, c_isin);
//                 Some(res_bin)
//             }
//             None => dreq.txnCardInfo.cardIsin,
//         },
//     };
// logger::debug!(
//     tag = "resolveBin of txnCardInfo",
//     "{:?}",
//     resolve_bin.clone()
// );
//     let m_vault_provider = Utils::getVaultProvider(dreq.cardToken);
//     let update_txn_card_info = dreq.txnCardInfo.clone().with_card_isin(resolve_bin);
//     let decider_params = T::DeciderParams {
//         merchantAccount: dreq.merchantAccount,
//         orderReference: dreq.orderReference,
//         txnDetail: dreq.txnDetail,
//         txnOfferDetails: None,
//         txnCardInfo: update_txn_card_info,
//         cardToken: None,
//         vaultProvider: m_vault_provider,
//         txnType: dreq.txn_type,
//         merchantPrefs: merchant_prefs,
//         orderMetadata: dreq.orderMetadata,
//         enforceGatewayList: enforced_gateway_filter,
//         priorityLogicOutput: dreq.priorityLogicOutput,
//         priorityLogicScript: dreq.priorityLogicScript,
//         isEdccApplied: dreq.isEdccApplied,
//     };
//     runGwListFlow(decider_params).await
// }

// pub async fn runGwListFlow(
//     deciderParams: T::DeciderParams,
// ) -> impl L::MonadFlow<Result<(Vec<Gateway>, Vec<(Gateway, String)>), (String, Vec<(Gateway, String)>)>> {

//     let txnCreationTime = deciderParams
//         .dpTxnDetail
//         .date_created
//         .clone()
//         .to_string()
//         .replace(" ", "T")
//         .replace(" UTC", "Z");
//     let mut logger = HashMap::new();
//     let mut deciderState = T::initial_decider_state(txnCreationTime.clone());
//     let (functional_gateways, decider_state) = GF::gwFiltersForEligibility(deciderParams.clone(), &mut deciderState).await;
//     let mut decider_flow =
//         T::initial_decider_flow(deciderParams.clone(), &mut logger, &mut deciderState).await;
//     if functional_gateways.is_empty() {
//         let failure_reason = getDeciderFailureReason(
//             &mut decider_flow,
//             &decider_state.debug_filter_list,
//             None
//         )
//         .await;

//         let gw_wise_failure_reason = getDeciderFailureReasonGwWise(
//             &mut decider_flow,
//             &decider_state.debug_filter_list,
//             None
//         )
//         .await;

//         Err((failure_reason, gw_wise_failure_reason))
//     } else {
//         let gw_wise_failure_reason = getDeciderFailureReasonGwWise(
//             &mut decider_flow,
//             &decider_state.debug_filter_list,
//             None
//         )
//         .await;

//         Ok((functional_gateways, gw_wise_failure_reason))
//     }
// }

pub async fn run_decider_flow(
    deciderParams: T::DeciderParams,
    is_legacy_decider_flow: bool,
) -> Result<(T::DecidedGateway, Vec<(String, Vec<String>)>), T::ErrorResponse> {
    let txnCreationTime = deciderParams
        .dpTxnDetail
        .date_created
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
    let (functionalGateways, allMgas) = GF::newGwFilters(&mut decider_flow).await?;

    logger::info!(
        tag = "GW_Filtering",
        action = "GW_Filtering",
        "{:?}",
        sortedFilterList(&decider_flow.writer.debugFilterList.clone())
    );

    let preferredGateway = deciderParams
        .dpTxnDetail
        .gateway
        .clone()
        .or(deciderParams.dpOrder.preferred_gateway.clone());
    let gatewayMgaIdMap = get_gateway_to_mga_id_map_f(&allMgas, &functionalGateways);

    logger::warn!(
        action = "PreferredGateway",
        tag = "PreferredGateway",
        "Preferred gateway provided by merchant for {:?} = {:?}",
        &deciderParams.dpTxnDetail.txn_id,
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
                    gateway_mga_id_map: Some(gatewayMgaIdMap),
                    debit_routing_output: None,
                    is_rust_based_decider: deciderParams.dpShouldConsumeResult.unwrap_or(false),
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
                    deciderParams.dpTxnDetail.txn_id
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
            let gwPLogic = match deciderParams.dpPriorityLogicOutput {
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
            };

            let gatewayPriorityList = add_preferred_gateways_to_priority_list(
                gwPLogic.gws.clone(),
                preferredGateway.clone(),
            );
            logger::info!(
                tag = "gatewayPriorityList",
                action = "gatewayPriorityList",
                "Gateway priority for merchant for {:?} = {:?}",
                &deciderParams.dpTxnDetail.txn_id,
                gatewayPriorityList
            );

            let (mut functionalGateways, updatedPriorityLogicOutput) = if gwPLogic.is_enforcement {
                logger::info!(
                    tag = "gatewayPriorityList",
                    action = "Enforcing Priority Logic",
                    "Enforcing Priority Logic for {:?}",
                    deciderParams.dpTxnDetail.txn_id
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
                    &deciderParams.dpTxnDetail.txn_id,
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
                &deciderParams.dpTxnDetail.txn_id,
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
                None,
                None,
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
                        &deciderParams.dpTxnDetail.txn_id,
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
                            priority_logic_tag: updatedPriorityLogicOutput
                                .priority_logic_tag
                                .clone(),
                            routing_approach: finalDeciderApproach.clone(),
                            gateway_before_evaluation: topGatewayBeforeSRDowntimeEvaluation.clone(),
                            priority_logic_output: Some(updatedPriorityLogicOutput),
                            debit_routing_output: None,
                            reset_approach: decider_flow.writer.reset_approach.clone(),
                            routing_dimension: decider_flow.writer.routing_dimension.clone(),
                            routing_dimension_level: decider_flow
                                .writer
                                .routing_dimension_level
                                .clone(),
                            is_scheduled_outage: decider_flow.writer.isScheduledOutage,
                            is_dynamic_mga_enabled: decider_flow.writer.is_dynamic_mga_enabled,
                            gateway_mga_id_map: Some(gatewayMgaIdMap),
                            is_rust_based_decider: deciderParams
                                .dpShouldConsumeResult
                                .unwrap_or(false),
                        }),
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
        &deciderParams.dpTxnDetail.txn_uuid.clone(),
    ]
    .concat();
    let updated_gateway_scoring_data = T::GatewayScoringData {
        routingApproach: Some(decider_flow.writer.gwDeciderApproach.clone().to_string()),
        is_legacy_decider_flow,
        ..decider_flow.writer.gateway_scoring_data.clone()
    };
    let app_state = get_tenant_app_state().await;
    if deciderParams.dpShouldConsumeResult.unwrap_or(false) {
        app_state
            .redis_conn
            .setx(
                &key,
                serde_json::to_string(&updated_gateway_scoring_data.clone())
                    .unwrap_or_default()
                    .as_str(),
                C::GATEWAY_SCORE_KEYS_TTL,
            )
            .await
            .unwrap_or_default();
        updated_gateway_scoring_data;
    }
    match dResult {
        Ok(result) => Ok((
            result,
            sortedFilterList(&decider_flow.writer.debugFilterList.clone()),
        )),
        Err((
            debugFilterList,
            _,
            priorityLogicTag,
            finalDeciderApproach,
            priorityLogicOutput,
            isDynamicMGAEnabled,
        )) => {
            let developerMessage = getDeciderFailureReason(
                &mut decider_flow,
                debugFilterList,
                priorityLogicOutput.clone(),
            )
            .await;

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
                    developer_message: developerMessage,
                },
                priority_logic_output: priorityLogicOutput,
                is_dynamic_mga_enabled: isDynamicMGAEnabled,
            })
        }
    }
}

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
    preferredGatewayM: Option<String>,
) -> Vec<String> {
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

fn getValidationType(
    txnDetail: &ETTD::TxnDetail,
    txnCardInfo: &TxnCardInfo,
) -> Option<ValidationType> {
    if is_mandate_transaction(txnDetail) && is_card_transaction(txnCardInfo) {
        Some(ValidationType::CardMandate)
    } else if is_tpv_transaction(txnDetail) || is_emandate_transaction(txnDetail) {
        if is_emandate_transaction(txnDetail) {
            Some(if is_tpv_mandate_transaction(txnDetail) {
                ValidationType::TpvMandate
            } else {
                ValidationType::Emandate
            })
        } else {
            Some(ValidationType::Tpv)
        }
    } else {
        None
    }
}

// pub async fn getDeciderFailureReasonGwWise(
//     decider_flow: &mut T::DeciderFlow<'_>,
//     debug_filter_list: Vec<T::DebugFilterEntry>,
//     priority_logic_output: Option<T::GatewayPriorityLogicOutput>,
// ) -> Vec<(Gateway, String)> {
//     let filter_list_sorted = sortedFilterList(&debug_filter_list);

//     let configured_gateways_m = filter_list_sorted
//         .iter()
//         .find(|(filter_name, _)| filter_name == "filterFunctionalGatewaysForCurrency");
//     let configured_gateways = configured_gateways_m
//         .map(|(_, gateways)| gateways.clone())
//         .unwrap_or_default();
//     let mut results = Vec::new();
//     for gateway in configured_gateways {
//         let gw_eliminated_filter = filter_list_sorted
//             .iter()
//             .find(|(_, filter_gateways)| !filter_gateways.contains(&gateway));

//         let failure_reason = getFailureReasonWithFilter(
//             decider_flow,
//             debug_filter_list.clone(),
//             priority_logic_output.clone(),
//             gw_eliminated_filter.cloned(),
//         )
//         .await;

//         if !decider_flow.writer.functionalGateways.contains(&gateway) {
//             results.push((gateway, failure_reason));
//         }
//     }

//     results
// }

fn filterList(debugFilterList: &[T::DebugFilterEntry]) -> Vec<(String, Vec<String>)> {
    debugFilterList
        .iter()
        .map(|entry| (entry.filterName.clone(), entry.gateways.clone()))
        .collect()
}

pub async fn getDeciderFailureReason(
    decider_flow: &mut T::DeciderFlow<'_>,
    debug_filter_list: Vec<T::DebugFilterEntry>,
    priority_logic_output: Option<T::GatewayPriorityLogicOutput>,
) -> String {
    let sorted_filters = sortedFilterList(&debug_filter_list);
    let filter_with_empty_list = sorted_filters.iter().find(|(_, list)| list.is_empty());
    getFailureReasonWithFilter(
        decider_flow,
        debug_filter_list,
        priority_logic_output,
        filter_with_empty_list.cloned(),
    )
    .await
}

pub async fn getFailureReasonWithFilter(
    decider_flow: &mut T::DeciderFlow<'_>,
    debug_filter_list: Vec<T::DebugFilterEntry>,
    priority_logic_output: Option<T::GatewayPriorityLogicOutput>,
    filter_entry: Option<(String, Vec<String>)>,
) -> String {
    let txn_detail = &decider_flow.get().dpTxnDetail;
    let txn_card_info = &decider_flow.get().dpTxnCardInfo;
    let macc = &decider_flow.get().dpMerchantAccount;
    let order_reference = &decider_flow.get().dpOrder;
    let m_internal_meta = &decider_flow.writer.internalMetaData;
    let m_card_brand = &decider_flow.writer.cardBrand;
    let vault_provider_m = &decider_flow.get().dpVaultProvider;
    let m_txn_type: Option<String> = decider_flow.get().dpTxnType.clone();
    let stored_card_vault_provider = m_internal_meta
        .as_ref()
        .and_then(|meta| meta.storedCardVaultProvider.clone());
    let scope = format!(
        "{} ",
        stored_card_vault_provider
            .as_ref()
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| "card".to_string())
    );
    let preferred_gateway_m = txn_detail
        .gateway
        .clone()
        .or_else(|| order_reference.preferred_gateway.clone());
    let filter_list = filterList(&debug_filter_list);
    let configured_gateways_m = filter_list
        .iter()
        .find(|(filter_name, _)| filter_name == "filterFunctionalGatewaysForCurrency");
    let configured_gateways = configured_gateways_m
        .map(|(_, gateways)| gateways.clone())
        .unwrap_or_default();
    let juspay_bank_code_m = Utils::get_juspay_bank_code_from_internal_metadata(txn_detail);

    match filter_entry
        .as_ref()
        .map(|(name, _)| name.as_str())
        .unwrap_or("NO_EMPTY")
    {
        "getFunctionalGateways" => {
            let reference_ids = Utils::get_all_ref_ids(
                decider_flow.writer.metadata.clone().unwrap_or_default(),
                priority_logic_output
                    .map(|logic| logic.gateway_reference_ids.clone())
                    .unwrap_or_default(),
            )
            .await;
            format!(
                "No gateways are configured with the referenceIds {} to proceed transaction ",
                json!(reference_ids)
            )
        }
        "filterFunctionalGatewaysForCurrency" => {
            format!(
                "No functional gateways after filtering for currency {}",
                json!(txn_detail.currency)
            )
        }
        "filterFunctionalGatewaysForBrand" => {
            format!(
                "No functional gateways after filtering for brand {}",
                m_card_brand.clone().unwrap_or_default()
            )
        }
        "filterFunctionalGatewaysForAuthType" => {
            format!(
                "No functional gateways after filtering for authType {}",
                txn_card_info
                    .auth_type
                    .as_ref()
                    .map(|auth_type| auth_type.clone().to_string())
                    .unwrap_or_default()
            )
        }
        "filterFunctionalGatewaysForValidationType" => {
            format!(
                "No functional gateways after filtering for validationType {}",
                getValidationType(txn_detail, txn_card_info)
                    .map(|val_type| val_type.to_string())
                    .unwrap_or_default()
            )
        }
        "filterFunctionalGatewaysForEmi" => {
            let emi_type = format!(
                "{} ",
                Utils::fetch_emi_type(txn_card_info)
                    .map(|emi| emi.to_lowercase())
                    .unwrap_or_else(|| "emi".to_string())
            );
            let emi_bank = format!("{} ", txn_detail.emi_bank.clone().unwrap_or_default());
            if Utils::is_card_transaction(txn_card_info) && txn_detail.is_emi != Some(true) {
                "Gateways configured supports only emi transaction.".to_string()
            } else if Utils::is_card_transaction(txn_card_info) {
                let is_bin_eligible = Utils::check_if_bin_is_eligible_for_emi(
                    txn_card_info.card_isin.clone(),
                    juspay_bank_code_m,
                    txn_card_info
                        .clone()
                        .card_type
                        .map(|card_type| card_type_to_text(&card_type)),
                )
                .await;
                if is_bin_eligible {
                    format!(
                        "No functional gateways supporting {}{}{}transaction.",
                        emi_bank, scope, emi_type
                    )
                } else {
                    "Bin doesn't support emi transaction.".to_string()
                }
            } else {
                format!(
                    "No functional gateways supporting {}{}{}transaction.",
                    emi_bank, scope, emi_type
                )
            }
        }
        "filterFunctionalGatewaysForPaymentMethod" => {
            format!(
                "No functional gateways supporting {} payment method.",
                txn_card_info.payment_method
            )
        }
        "filterFunctionalGatewaysForTokenProvider" => {
            let vault_provider = vault_provider_m.clone().unwrap_or(VaultProvider::Juspay);
            format!(
                "No functional gateways supporting {} saved cards.",
                vault_provider
            )
        }
        "filterFunctionalGatewaysForWallet" => {
            if txn_card_info.card_type == Some(CardType::Wallet) {
                "No functional gateways supporting wallet transaction.".to_string()
            } else {
                "Gateways configured supports only wallet transaction.".to_string()
            }
        }
        "filterFunctionalGatewaysForNbOnly" => {
            if txn_card_info.card_type == Some(CardType::Nb) {
                "No functional gateways supporting Net Banking transaction.".to_string()
            } else {
                "Gateways configured supports only Net Banking transaction.".to_string()
            }
        }
        "filterFunctionalGatewaysForConsumerFinance" => {
            if txn_card_info.payment_method_type == CONSUMER_FINANCE {
                "No functional gateways supporting Consumer Finance transaction.".to_string()
            } else {
                "Gateways configured supports only Consumer Finance transaction.".to_string()
            }
        }
        "filterFunctionalGatewaysForUpi" => {
            if txn_card_info.payment_method_type == UPI {
                "No functional gateways supporting UPI transaction.".to_string()
            } else if !is_google_pay_txn(txn_card_info.clone()) {
                "Gateways configured supports only UPI transaction.".to_string()
            } else {
                "No functional gateways".to_string()
            }
        }
        "filterFunctionalGatewaysForTxnType" => match m_txn_type {
            None => "No functional gateways".to_string(),
            Some(txn_type) => format!(
                "No functional gateways supporting {} transaction.",
                txn_type
            ),
        },
        "filterFunctionalGatewaysForTxnDetailType" => {
            format!(
                "No functional gateways supporting {:?}transaction.",
                txn_detail.txn_type
            )
        }
        "filterFunctionalGatewaysForReward" => {
            if txn_card_info.card_type == Some(CardType::Reward)
                || txn_card_info.payment_method_type == REWARD
            {
                "No functional gateways supporting Reward transaction.".to_string()
            } else {
                "Gateways configured supports only Reward transaction.".to_string()
            }
        }
        "filterFunctionalGatewaysForCash" => {
            if txn_card_info.payment_method_type == CASH {
                "No functional gateways supporting CASH transaction.".to_string()
            } else {
                "Gateways configured supports only CASH transaction.".to_string()
            }
        }
        "filterFunctionalGatewaysForSplitSettlement" => {
            "No functional gateways after validating split.".to_string()
        }
        "filterFunctionalGatewaysForOTMFlow" => {
            "No functional gateways after filtering for OTM flow.".to_string()
        }
        "filterFunctionalGateways" => {
            if Utils::is_card_transaction(txn_card_info) {
                if m_internal_meta
                    .as_ref()
                    .and_then(|meta| meta.isCvvLessTxn)
                    .unwrap_or(false)
                    && txn_card_info.auth_type == Some(AuthType::Moto)
                {
                    format!(
                        "No functional gateways supporting cvv less {}repeat moto transaction.",
                        scope
                    )
                } else if m_internal_meta
                    .as_ref()
                    .and_then(|meta| meta.isCvvLessTxn)
                    .unwrap_or(false)
                {
                    format!(
                        "No functional gateways supporting cvv less {} transaction.",
                        scope
                    )
                } else if Utils::is_token_repeat_txn(m_internal_meta.clone()) {
                    format!("No functional gateways supporting {}transaction.", scope)
                } else {
                    "No functional gateways supporting transaction.".to_string()
                }
            } else {
                "No functional gateways supporting transaction.".to_string()
            }
        }
        "preferredGateway" => match preferred_gateway_m {
            Some(preferred_gateway) => {
                if configured_gateways.contains(&preferred_gateway) {
                    format!("{} is not supporting this transaction.", preferred_gateway)
                } else {
                    format!("{} is not configured.", preferred_gateway)
                }
            }
            None => "No functional gateways supporting this transaction.".to_string(),
        },
        "filterEnforcement" => {
            "Priority logic enforced gateways are not supporting this transaction.".to_string()
        }
        "filterFunctionalGatewaysForMerchantRequiredFlow" => {
            let payment_flow_list = Utils::get_payment_flow_list_from_txn_detail(txn_detail);
            let is_mf_order = payment_flow_list.contains(&"MUTUAL_FUND".to_string());
            let is_cb_order = payment_flow_list.contains(&"CROSS_BORDER_PAYMENT".to_string());
            let is_sbmd = payment_flow_list.contains(&"SINGLE_BLOCK_MULTIPLE_DEBIT".to_string());
            let message = if is_mf_order {
                "Mutual Fund transaction flow"
            } else if is_cb_order {
                "Cross Border transaction flow"
            } else if is_sbmd {
                "Single Block Multiple Debit"
            } else {
                "Merchant requested payment flows "
            };
            format!("No functional gateways after filtering for {}", message)
        }
        "filterGatewaysForMGASelectionIntegrity" => {
            "Conflicting configurations found or no functional gateways supporting this transaction"
                .to_string()
        }
        "filterGatewaysForEMITenureSpecficGatewayCreds" => {
            "No functional gateways supporting for emi.".to_string()
        }
        "FilterFunctionalGatewaysForReversePennyDrop" => {
            "No functional gateways after filtering for Reverse Penny Drop transaction ".to_string()
        }
        _ => "No functional gateways supporting this transaction.".to_string(),
    }
}

fn sortedFilterList(debugFilterList: &[T::DebugFilterEntry]) -> Vec<(String, Vec<String>)> {
    let mut list = filterList(debugFilterList);
    list.sort_by(|(a, _), (b, _)| {
        Utils::decider_filter_order(a).cmp(&Utils::decider_filter_order(b))
    });
    list
}
