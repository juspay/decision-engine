use std::collections::HashMap;
use std::vec::Vec;
use std::string::String;
use std::option::Option;
use serde_json::json;
use serde_json::Value as AValue;
use eulerhs::prelude::*;
use eulerhs::language as L;
use eulerhs::framework as Framework;
use gatewaydecider::flow::*;
use gatewaydecider::runner::{EvaluationResult, evalScript, handleFallbackLogic};
use gatewaydecider::types as T;
use gatewaydecider::gwfilter as GF;
use gatewaydecider::gwscoring as GS;
use gatewaydecider::validators as V;
use gatewaydecider::utils as Utils;
use optics_core::{preview, review};
use types::card::{TxnCardInfo, TxnCardInfoPId};
use types::gateway::{Gateway, gatewayToText};
use types::merchant as ETM;
use types::order as ETO;
use types::payment as ETP;
use types::order_metadata_v2 as ETOMV2;
use types::txn_detail as ETTD;
use utils::errors::predefined_errors as Errs;
use juspay::extra::parsing::{Parsed, parse};
use juspay::extra::secret::{SecretContext, makeSecret};
use juspay::extra::json as JSON;
use juspay::extra::non_empty_text as NE;

pub fn deciderFullPayloadEndpoint(
    headers: Framework::Headers,
    session_id: String,
    request: T::ApiDeciderFullRequest,
) -> impl L::MonadFlow<Framework::Response> {
    L::setLoggerContext("x-request-id", getSessionId(&headers, &session_id));
    let response = deciderFullPayloadEndpointParser(request.captures);
    match response {
        Ok(gw) => Framework::json(gw),
        Err(err) => Framework::jsonWithCode(400, err),
    }
}

fn getSessionId(headers: &Framework::Headers, session_id: &String) -> String {
    match Framework::getHeaderValue("x-request-id", headers) {
        Some(id) => id,
        None => session_id.clone(),
    }
}

pub fn deciderFullPayloadEndpointParser(
    request: T::ApiDeciderRequest,
) -> impl L::MonadFlow<Result<T::DecidedGateway, T::ErrorResponse>> {
    let parsed_request = V::parseApiDeciderRequest(request);
    match parsed_request {
        Parsed::Result(req) => {
            let merchant_acc = match ETM::loadMerchantByMerchantId(req.orderReference.merchantId) {
                Some(acc) => acc,
                None => {
                    L::logErrorV("getMaccByMerchantId", format!("Merchant account for id: {}", req.orderReference.merchantId));
                    L::throwException(Errs::internalError(
                        Some("merchant account with the given merchant id not found."),
                        Some("merchant account with the given merchant id not found."),
                        None,
                    ));
                }
            };
            L::setLoggerContext("merchant_id", Utils::getMId(req.orderReference.merchantId));
            if let Some(tenant_id) = merchant_acc.tenantAccountId {
                L::setLoggerContext("tenant_id", tenant_id);
            }
            L::setLoggerContext("txn_uuid", req.txnDetail.txnUuid);
            L::setLoggerContext("order_id", req.orderReference.orderId.unOrderId);
            L::setLoggerContext("txn_creation_time", req.txnDetail.dateCreated.to_string());
            let resolve_bin = match Utils::fetchExtendedCardBin(req.txnCardInfo) {
                Some(card_bin) => Some(card_bin),
                None => match req.txnCardInfo.cardIsin {
                    Some(c_isin) => {
                        let res_bin = Utils::getCardBinFromTokenBin(6, c_isin);
                        Some(res_bin)
                    }
                    None => req.txnCardInfo.cardIsin,
                },
            };
            L::logDebugV("resolveBin of txnCardInfo", resolve_bin.clone());
            let request = T::transformRequest(req, merchant_acc, resolve_bin);
            L::logDebugT("enforeced gateway list: ", request.enforceGatewayList.to_string());
            let decider_response = deciderFullPayloadHSFunction(request);
            decider_response
        }
        Parsed::Failed(err) => T::handleLeftCase(err.to_string()),
    }
}

pub fn deciderFullPayloadHSFunction(
    dreq: T::DomainDeciderRequest,
) -> impl L::MonadFlow<Result<T::DecidedGateway, T::ErrorResponse>> {
    let merchant_prefs = match ETM::getMerchantIPrefsByMId(dreq.txnDetail.merchantId) {
        Some(prefs) => prefs,
        None => {
            L::logErrorV("getMerchantPrefsByMId", format!("Merchant iframe preferences not found for id: {}", dreq.txnDetail.merchantId));
            L::throwException(Errs::internalError(
                Some("merchant iframe preferences not found"),
                Some("merchant iframe preferences not found with the given merchant id"),
                None,
            ));
        }
    };
    let enforced_gateway_filter = handleEnforcedGateway(dreq.enforceGatewayList);
    let resolve_bin = match Utils::fetchExtendedCardBin(dreq.txnCardInfo) {
        Some(card_bin) => Some(card_bin),
        None => match dreq.txnCardInfo.cardIsin {
            Some(c_isin) => {
                let res_bin = Utils::getCardBinFromTokenBin(6, c_isin);
                Some(res_bin)
            }
            None => dreq.txnCardInfo.cardIsin,
        },
    };
    L::logDebugV("resolveBin of txnCardInfo", resolve_bin.clone());
    let m_vault_provider = Utils::getVaultProvider(dreq.cardToken);
    let update_txn_card_info = dreq.txnCardInfo.clone().with_card_isin(resolve_bin);
    let decider_params = T::DeciderParams {
        merchantAccount: dreq.merchantAccount,
        orderReference: dreq.orderReference,
        txnDetail: dreq.txnDetail,
        txnOfferDetails: dreq.txnOfferDetails,
        txnCardInfo: update_txn_card_info,
        cardToken: None,
        vaultProvider: m_vault_provider,
        txnType: dreq.txnType,
        merchantPrefs: merchant_prefs,
        orderMetadata: dreq.orderMetadata,
        enforceGatewayList: enforced_gateway_filter,
        priorityLogicOutput: dreq.priorityLogicOutput,
        priorityLogicScript: dreq.priorityLogicScript,
        isEdccApplied: dreq.isEdccApplied,
    };
    runDeciderFlow(decider_params)
}

fn handleEnforcedGateway(gateway_list: Option<Vec<Gateway>>) -> Option<Vec<Gateway>> {
    match gateway_list {
        None => None,
        Some(list) if list.is_empty() => None,
        list => list,
    }
}

pub fn getSupportedGws(
    dreq: T::DomainDeciderRequest,
) -> impl L::MonadFlow<Result<Vec<Gateway>, String>> {
    let merchant_prefs = match ETM::getMerchantIPrefsByMId(dreq.txnDetail.merchantId) {
        Some(prefs) => prefs,
        None => {
            L::logErrorV("getMerchantPrefsByMId", format!("Merchant iframe preferences not found for id: {}", dreq.txnDetail.merchantId));
            L::throwException(Errs::internalError(
                Some("merchant iframe preferences not found"),
                Some("merchant iframe preferences not found with the given merchant id"),
                None,
            ));
        }
    };
    let enforced_gateway_filter = handleEnforcedGateway(dreq.enforceGatewayList);
    let resolve_bin = match Utils::fetchExtendedCardBin(dreq.txnCardInfo) {
        Some(card_bin) => Some(card_bin),
        None => match dreq.txnCardInfo.cardIsin {
            Some(c_isin) => {
                let res_bin = Utils::getCardBinFromTokenBin(6, c_isin);
                Some(res_bin)
            }
            None => dreq.txnCardInfo.cardIsin,
        },
    };
    L::logDebugV("resolveBin of txnCardInfo", resolve_bin.clone());
    let m_vault_provider = Utils::getVaultProvider(dreq.cardToken);
    let update_txn_card_info = dreq.txnCardInfo.clone().with_card_isin(resolve_bin);
    let decider_params = T::DeciderParams {
        merchantAccount: dreq.merchantAccount,
        orderReference: dreq.orderReference,
        txnDetail: dreq.txnDetail,
        txnOfferDetails: None,
        txnCardInfo: update_txn_card_info,
        cardToken: None,
        vaultProvider: m_vault_provider,
        txnType: dreq.txnType,
        merchantPrefs: merchant_prefs,
        orderMetadata: dreq.orderMetadata,
        enforceGatewayList: enforced_gateway_filter,
        priorityLogicOutput: dreq.priorityLogicOutput,
        priorityLogicScript: dreq.priorityLogicScript,
        isEdccApplied: dreq.isEdccApplied,
    };
    runGwListFlow(decider_params)
}

pub fn runGwListFlow(
    decider_params: T::DeciderParams,
) -> impl L::MonadFlow<Result<Vec<Gateway>, String>> {
    let txn_creation_time = T::replace(" ", "T", T::replace(" UTC", "Z", decider_params.txnDetail.dateCreated.to_string()));
    let (functional_gateways, decider_state) = runStateT(runReaderT(GF::gwFiltersForEligibility, decider_params), T::initialDeciderState(txn_creation_time));
    if functional_gateways.is_empty() {
        Err(getDeciderFailureReason(decider_params, decider_state, decider_state.debugFilterList, None))
    } else {
        Ok(functional_gateways)
    }
}
  
pub async fn runDeciderFlow(  
    deciderParams: T::DeciderParams,  
) -> Result<Result<T::DecidedGateway, T::ErrorResponse>, Box<dyn std::error::Error>> {  
    let txnCreationTime = deciderParams.dpTxnDetail.dateCreated.replace(" ", "T").replace(" UTC", "Z");  
    let deciderState = Arc::new(Mutex::new(T::initialDeciderState(txnCreationTime.clone())));  
  
    let ((functionalGateways, allMgas), deciderState) = spawn_blocking(move || {  
        let deciderState = deciderState.clone();  
        let deciderParams = deciderParams.clone();  
        let result = tokio::runtime::Handle::current().block_on(async move {  
            let mut state = deciderState.lock().await;  
            let result = GF::newGwFilters(&deciderParams, &mut state).await;  
            (result, state.clone())  
        });  
        result  
    }).await??;  
  
    L::logInfoV("GW_Filtering", &sortedFilterList(&deciderState.debugFilterList)).await;  
  
    let preferredGateway = deciderParams.dpTxnDetail.gateway.or(deciderParams.dpOrder.preferredGateway);  
    let gatewayMgaIdMap = getGatewayToMGAIdMapF(&allMgas, &functionalGateways);  
  
    L::logInfoT("PreferredGateway", &format!(  
        "Preferred gateway provided by merchant for {} = {}",  
        transactionIdText(&deciderParams.dpTxnDetail.txnId),  
        preferredGateway.map_or("None".to_string(), |pgw| pgw.to_string())  
    )).await;  
  
    let dResult = match (preferredGateway, deciderParams.dpMerchantPrefs.dynamicSwitchingEnabled) {  
        (Some(pgw), false) => {  
            if functionalGateways.contains(&pgw) {  
                Utils::logGatewayDeciderApproach(  
                    Some(&pgw),  
                    None,  
                    &[],  
                    T::MERCHANT_PREFERENCE,  
                    None,  
                    &functionalGateways,  
                    None,  
                    &deciderParams,  
                    &mut deciderState.lock().await  
                ).await;  
                Ok(T::DecidedGateway {  
                    decided_gateway: pgw,  
                    gateway_priority_map: Some(json!(HashMap::from([(pgw.to_string(), 1.0)]))),  
                    filter_wise_gateways: None,  
                    priority_logic_tag: None,  
                    routing_approach: T::MERCHANT_PREFERENCE,  
                    gateway_before_evaluation: None,  
                    priority_logic_output: None,  
                    reset_approach: T::NO_RESET,  
                    routing_dimension: None,  
                    routing_dimension_level: None,  
                    is_scheduled_outage: false,  
                    is_dynamic_mga_enabled: deciderState.lock().await.isDynamicMGAEnabled,  
                    gateway_mga_id_map: Some(gatewayMgaIdMap),  
                })  
            } else {  
                let mut state = deciderState.lock().await;  
                state.debugFilterList.push(T::DebugFilterEntry {  
                    filterName: "preferredGateway".to_string(),  
                    gateways: vec![],  
                });  
                L::logWarningV("PreferredGateway", &format!(  
                    "Preferred gateway {} functional/valid for merchant {} in txn {}",  
                    pgw,  
                    deciderParams.dpMerchantAccount.merchantId,  
                    deciderParams.dpTxnDetail.txnId  
                )).await;  
                Utils::logGatewayDeciderApproach(  
                    None,  
                    None,  
                    &[],  
                    T::NONE,  
                    None,  
                    &functionalGateways,  
                    None,  
                    &deciderParams,  
                    &mut state  
                ).await;  
                Err(T::ErrorResponse {  
                    debugFilterList: state.debugFilterList.clone(),  
                    debugScoringList: state.debugScoringList.clone(),  
                    priorityLogicTag: None,  
                    routing_approach: T::NONE,  
                    priority_logic_output: None,  
                    is_dynamic_mga_enabled: state.isDynamicMGAEnabled,  
                })  
            }  
        }  
        _ => {  
            let gwPLogic = match deciderParams.dpPriorityLogicOutput {  
                Some(ref plOp) => plOp.clone(),  
                None => Runner::getGatewayPriority(  
                    &deciderParams.dpMerchantAccount,  
                    &deciderParams.dpOrder,  
                    &deciderParams.dpTxnDetail,  
                    &deciderParams.dpTxnCardInfo,  
                    &deciderState.lock().await.internalMetaData,  
                    &deciderParams.dpOrderMetadata.metadata,  
                    &deciderParams.dpPriorityLogicScript  
                ).await?,  
            };  
  
            let gatewayPriorityList = addPreferredGatewaysToPriorityList(&gwPLogic.gws, preferredGateway);  
            L::logInfoV("gatewayPriorityList", &format!(  
                "Gateway priority for merchant for {} = {:?}",  
                transactionIdText(&deciderParams.dpTxnDetail.txnId),  
                gatewayPriorityList  
            )).await;  
  
            let (functionalGateways, deciderState, updatedPriorityLogicOutput) = if gwPLogic.isEnforcement {  
                L::logInfoT("gatewayPriorityList", &format!(  
                    "Enforcing Priority Logic for {}",  
                    transactionIdText(&deciderParams.dpTxnDetail.txnId)  
                )).await;  
                let (res, priorityLogicOutput) = filterFunctionalGatewaysWithEnforcment(  
                    &functionalGateways,  
                    &gatewayPriorityList,  
                    &gwPLogic,  
                    preferredGateway,  
                    &deciderParams,  
                    &mut deciderState.lock().await  
                ).await?;  
                L::logInfoT("gatewayPriorityList", &format!(  
                    "Functional gateways after filtering for Enforcement Logic for {} : {:?}",  
                    transactionIdText(&deciderParams.dpTxnDetail.txnId),  
                    res  
                )).await;  
                let mut state = deciderState.lock().await;  
                state.debugFilterList.push(T::DebugFilterEntry {  
                    filterName: "filterEnforcement".to_string(),  
                    gateways: res.clone(),  
                });  
                (res, state.clone(), priorityLogicOutput)  
            } else {  
                (functionalGateways.clone(), deciderState.lock().await.clone(), gwPLogic)  
            };  
  
            let uniqueFunctionalGateways = functionalGateways.into_iter().collect::<Vec<_>>();  
            L::logInfoV("PriorityLogicOutput", &updatedPriorityLogicOutput).await;  
            L::logInfoT("GW_Filtering", &format!(  
                "Functional gateways after {} for {} : {:?}",  
                T::FilterByPriorityLogic,  
                transactionIdText(&deciderParams.dpTxnDetail.txnId),  
                uniqueFunctionalGateways  
            )).await;  
  
            let (currentGatewayScoreMap, st) = GS::scoringFlow(  
                &uniqueFunctionalGateways,  
                &updatedPriorityLogicOutput.gws,  
                &deciderParams,  
                &mut deciderState.lock().await  
            ).await?;  
  
            L::logInfoV("GW_Scoring", &st.debugScoringList.iter().map(|scoreData| {  
                (scoreData.scoringName.clone(), scoreData.gatewayScores.clone())  
            }).collect::<HashMap<_, _>>()).await;  
  
            let scoreList = currentGatewayScoreMap.iter().collect::<Vec<_>>();  
            L::logDebugT("scoreList", &format!("{:?}", scoreList)).await;  
  
            let gatewayPriorityMap = Some(json!(scoreList.iter().map(|(gw, score)| {  
                (gw.to_string(), *score)  
            }).collect::<HashMap<_, _>>()));  
  
            match scoreList.as_slice() {  
                [] => Err(T::ErrorResponse {  
                    debugFilterList: st.debugFilterList.clone(),  
                    debugScoringList: st.debugScoringList.clone(),  
                    priorityLogicTag: updatedPriorityLogicOutput.priorityLogicTag.clone(),  
                    routing_approach: T::NONE,  
                    priority_logic_output: Some(updatedPriorityLogicOutput),  
                    is_dynamic_mga_enabled: deciderState.lock().await.isDynamicMGAEnabled,  
                }),  
                gs => {  
                    let (_, maxScore) = Utils::getMaxScoreGateway(&currentGatewayScoreMap);  
                    let decidedGateway = Utils::randomGatewaySelectionForSameScore(&currentGatewayScoreMap, maxScore).await?;  
                    L::logDebugT("decidedGateway after randomGatewaySelectionForSameScore", &format!("{:?}", decidedGateway)).await;  
  
                    let stateBindings = (  
                        st.srElminiationApproachInfo.clone(),  
                        st.isOptimizedBasedOnSRMetricEnabled,  
                        st.isSrV3MetricEnabled,  
                        st.topGatewayBeforeSRDowntimeEvaluation.clone(),  
                        st.isPrimaryGateway,  
                        st.experimentTag.clone()  
                    );  
  
                    let (srEliminationInfo, isOptimizedBasedOnSRMetricEnabled, isSrV3MetricEnabled, topGatewayBeforeSRDowntimeEvaluation, isPrimaryGateway, experimentTag) = stateBindings;  
  
                    let finalDeciderApproach = Utils::getGatewayDeciderApproach(&currentGatewayScoreMap, &st.gwDeciderApproach).await?;  
                    Utils::logGatewayDeciderApproach(  
                        decidedGateway.as_ref(),  
                        topGatewayBeforeSRDowntimeEvaluation.as_ref(),  
                        &srEliminationInfo,  
                        finalDeciderApproach.clone(),  
                        isPrimaryGateway,  
                        &uniqueFunctionalGateways,  
                        experimentTag.as_ref(),  
                        &deciderParams,  
                        &mut st  
                    ).await;  
  
                    L::logInfoT("Decided Gateway", &format!(  
                        "Gateway decided for {} = {:?}",  
                        transactionIdText(&deciderParams.dpTxnDetail.txnId),  
                        decidedGateway  
                    )).await;  
  
                    addMetricsToStream(  
                        Some(decidedGateway.as_ref()),  
                        finalDeciderApproach.clone(),  
                        updatedPriorityLogicOutput.priorityLogicTag.clone(),  
                        &st,  
                        &deciderParams,  
                        &currentGatewayScoreMap  
                    ).await?;  
  
                    L::logInfoV("GATEWAY_PRIORITY_MAP", &gatewayPriorityMap).await;  
  
                    match decidedGateway {  
                        Some(decideGatewayOutout) => Ok(T::DecidedGateway {  
                            decided_gateway: decideGatewayOutout,  
                            gateway_priority_map: gatewayPriorityMap,  
                            filter_wise_gateways: None,  
                            priority_logic_tag: updatedPriorityLogicOutput.priorityLogicTag.clone(),  
                            routing_approach: finalDeciderApproach.clone(),  
                            gateway_before_evaluation: topGatewayBeforeSRDowntimeEvaluation.clone(),  
                            priority_logic_output: Some(updatedPriorityLogicOutput),  
                            reset_approach: st.resetApproach.clone(),  
                            routing_dimension: st.routingDimension.clone(),  
                            routing_dimension_level: st.routingDimensionLevel.clone(),  
                            is_scheduled_outage: st.isScheduledOutage,  
                            is_dynamic_mga_enabled: deciderState.lock().await.isDynamicMGAEnabled,  
                            gateway_mga_id_map: Some(gatewayMgaIdMap),  
                        }),  
                        None => Err(T::ErrorResponse {  
                            debugFilterList: st.debugFilterList.clone(),  
                            debugScoringList: st.debugScoringList.clone(),  
                            priorityLogicTag: updatedPriorityLogicOutput.priorityLogicTag.clone(),  
                            routing_approach: finalDeciderApproach.clone(),  
                            priority_logic_output: Some(updatedPriorityLogicOutput),  
                            is_dynamic_mga_enabled: deciderState.lock().await.isDynamicMGAEnabled,  
                        })  
                    }  
                }  
            }  
        }  
    };  
  
    match dResult {  
        Ok(result) => Ok(Ok(result)),  
        Err(err) => {  
            let userMessage = getDeciderFailureReason(  
                &deciderParams,  
                &deciderState.lock().await,  
                &deciderState.lock().await.debugFilterList,  
                None  
            ).await?;  
            Ok(Err(T::ErrorResponse {  
                debugFilterList: deciderState.lock().await.debugFilterList.clone(),  
                debugScoringList: deciderState.lock().await.debugScoringList.clone(),  
                priorityLogicTag: None,  
                routing_approach: T::NONE,  
                priority_logic_output: None,  
                is_dynamic_mga_enabled: deciderState.lock().await.isDynamicMGAEnabled,  
                user_message: userMessage,  
            }))  
        }  
    }  
}  
  
fn getGatewayToMGAIdMapF(allMgas: &[T::MGA], gateways: &[Gateway]) -> AValue {  
    json!(gateways.iter().map(|x| {  
        (x.to_string(), allMgas.iter().find(|mga| mga.gateway == *x).map(|mga| mga.id))  
    }).collect::<HashMap<_, _>>())  
}  
  
fn addPreferredGatewaysToPriorityList(gwPriority: &[Gateway], preferredGatewayM: Option<Gateway>) -> Vec<Gateway> {  
    match preferredGatewayM {  
        None => gwPriority.to_vec(),  
        Some(pgw) => {  
            let mut list = gwPriority.to_vec();  
            list.retain(|&gw| gw != pgw);  
            list.insert(0, pgw);  
            list  
        }  
    }  
}  
  
async fn filterFunctionalGatewaysWithEnforcment(  
    fGws: &[Gateway],  
    priorityGws: &[Gateway],  
    plOp: &T::GatewayPriorityLogicOutput,  
    preferredGw: Option<Gateway>,  
    deciderParams: &T::DeciderParams,  
    deciderState: &mut T::DeciderState  
) -> Result<(Vec<Gateway>, T::GatewayPriorityLogicOutput), Box<dyn std::error::Error>> {  
    let enforcedGateways = fGws.iter().filter(|&gw| priorityGws.contains(gw)).cloned().collect::<Vec<_>>();  
    if enforcedGateways.is_empty() && deciderParams.dpPriorityLogicOutput.is_none() {  
        let mCardInfo = getCardInfoByBin(&deciderParams.dpTxnCardInfo.cardIsin).await?;  
        let updatedPlOp = handleFallbackLogic(  
            &deciderParams.dpMerchantAccount,  
            &deciderParams.dpOrder,  
            &deciderParams.dpTxnDetail,  
            &deciderParams.dpTxnCardInfo,  
            mCardInfo.as_ref(),  
            &deciderState.internalMetaData,  
            &deciderParams.dpOrderMetadata.metadata,  
            plOp,  
            T::NULL_AFTER_ENFORCE  
        ).await?;  
        let fallBackGwPriority = addPreferredGatewaysToPriorityList(&updatedPlOp.gws, preferredGw);  
        if updatedPlOp.isEnforcement {  
            let updatedEnforcedGateways = fGws.iter().filter(|&gw| fallBackGwPriority.contains(gw)).cloned().collect::<Vec<_>>();  
            if updatedEnforcedGateways.is_empty() {  
                let updatedPlOp = handleFallbackLogic(  
                    &deciderParams.dpMerchantAccount,  
                    &deciderParams.dpOrder,  
                    &deciderParams.dpTxnDetail,  
                    &deciderParams.dpTxnCardInfo,  
                    mCardInfo.as_ref(),  
                    &deciderState.internalMetaData,  
                    &deciderParams.dpOrderMetadata.metadata,  
                    &updatedPlOp,  
                    T::NULL_AFTER_ENFORCE  
                ).await?;  
                Ok((updatedEnforcedGateways, updatedPlOp))  
            } else {  
                Ok((updatedEnforcedGateways, updatedPlOp))  
            }  
        } else {  
            Ok((fGws.to_vec(), updatedPlOp))  
        }  
    } else {  
        Ok((enforcedGateways, plOp.clone()))  
    }  
}  
  
fn makeFirstLetterSmall(s: &str) -> String {  
    let mut chars = s.chars();  
    match chars.next() {  
        None => String::new(),  
        Some(f) => f.to_lowercase().collect::<String>() + chars.as_str(),  
    }  
}  
  
fn defaultDecidedGateway(  
    gw: Gateway,  
    gpm: Option<AValue>,  
    priorityLogicTag: Option<String>,  
    finalDeciderApproach: T::RoutingApproach,  
    topGatewayBeforeSRDowntimeEvaluation: Option<Gateway>,  
    priorityLogicOutput: Option<T::GatewayPriorityLogicOutput>,  
    resetApproach: T::ResetApproach,  
    routingDimension: Option<String>,  
    routingDimensionLevel: Option<String>,  
    isScheduledOutage: bool,  
    isDynamicMGAEnabled: bool,  
    gatewayMgaIdMap: Option<AValue>  
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
        gateway_mga_id_map: gatewayMgaIdMap,  
    }  
}  
  
async fn addMetricsToStream(  
    decidedGateway: Option<&Gateway>,  
    finalDeciderApproach: T::RoutingApproach,  
    mPriorityLogicTag: Option<String>,  
    st: &T::DeciderState,  
    deciderParams: &T::DeciderParams,  
    currentGatewayScoreMap: &HashMap<Gateway, f64>  
) -> Result<(), Box<dyn std::error::Error>> {  
    Utils::pushToStream(  
        decidedGateway,  
        finalDeciderApproach,  
        mPriorityLogicTag,  
        currentGatewayScoreMap,  
        deciderParams,  
        st  
    ).await  
}  
  
fn getValidationType(txnDetail: &T::TxnDetail, txnCardInfo: &T::TxnCardInfo) -> Option<String> {  
    if isMandateTransaction(txnDetail) && isCardTransaction(txnCardInfo) {  
        Some(T::CARD_MANDATE.to_string())  
    } else if isTpvTransaction(txnDetail) || isEmandateTransaction(txnDetail) {  
        if isEmandateTransaction(txnDetail) {  
            Some(if isTpvMandateTransaction(txnDetail) {  
                T::TPV_EMANDATE.to_string()  
            } else {  
                T::EMANDATE.to_string()  
            })  
        } else {  
            Some(T::TPV.to_string())  
        }  
    } else {  
        None  
    }  
}  
  
fn filterList(debugFilterList: &[T::DebugFilterEntry]) -> Vec<(String, Vec<Gateway>)> {  
    debugFilterList.iter().map(|entry| {  
        (entry.filterName.clone(), entry.gateways.clone())  
    }).collect()  
}  

pub async fn getDeciderFailureReason(  
    deciderParams: T::DeciderParams,  
    deciderState: T::DeciderState,  
    debugFilterList: Vec<T::DebugFilterEntry>,  
    priorityLogicOutput: Option<T::GatewayPriorityLogicOutput>,  
) -> Result<String, Box<dyn std::error::Error>> {  
    let filterWithEmptyList = debugFilterList.iter().find(|entry| entry.gateways.is_empty());  
    let txnDetail = &deciderParams.dpTxnDetail;  
    let txnCardInfo = &deciderParams.dpTxnCardInfo;  
    let macc = &deciderParams.dpMerchantAccount;  
    let orderReference = &deciderParams.dpOrder;  
    let mInternalMeta = &deciderState.internalMetaData;  
    let mCardBrand = &deciderState.cardBrand;  
    let vaultProviderM = &deciderParams.dpVaultProvider;  
    let mTxnType: Option<String> = deciderParams.dpTxnType.clone();  
    let storedCardVaultProvider = mInternalMeta.as_ref().and_then(|meta| meta.storedCardVaultProvider.clone());  
    let scope = format!("{} ", storedCardVaultProvider.as_deref().unwrap_or("card").to_lowercase());  
    let preferredGatewayM = txnDetail.gateway.or(orderReference.preferredGateway);  
    let configuredGatewaysM = debugFilterList.iter().find(|(filterName, _)| filterName == "getFunctionalGateways");  
    let configuredGateways = configuredGatewaysM.map_or(Vec::new(), |(_, gateways)| gateways.clone());  
    let juspaybankCodeM = getJuspayBankCodeFromInternalMetadata(txnDetail);  
  
    match filterWithEmptyList.map_or("NO_EMPTY", |entry| entry.filterName.as_str()) {  
        "getFunctionalGateways" => {  
            let referenceIds = getAllRefIds(deciderState.metadata.as_ref().unwrap_or(&HashMap::new()), priorityLogicOutput.as_ref().map_or(&HashMap::new(), |plo| &plo.gatewayReferenceIds));  
            Ok(format!("No gateways are configured with the referenceIds {} to proceed transaction", json!(referenceIds)))  
        }  
        "filterFunctionalGatewaysForCurrency" => Ok(format!("No functional gateways after filtering for currency {}", txnDetail.currency)),  
        "filterFunctionalGatewaysForBrand" => Ok(format!("No functional gateways after filtering for brand {}", mCardBrand.as_deref().unwrap_or(""))),  
        "filterFunctionalGatewaysForAuthType" => Ok(format!("No functional gateways after filtering for authType {}", txnCardInfo.authType.as_ref().map_or("", |authType| authType))),  
        "filterFunctionalGatewaysForValidationType" => Ok(format!("No functional gateways after filtering for validationType {}", getValidationType(txnDetail, txnCardInfo).map_or("", |vt| &vt))),  
        "filterFunctionalGatewaysForEmi" => {  
            let emiType = format!("{} ", utils::fetchEmiType(txnCardInfo).unwrap_or("emi").to_lowercase());  
            let emiBank = format!("{} ", txnDetail.emiBank.as_deref().unwrap_or(""));  
            if isCardTransaction(txnCardInfo) && !txnDetail.isEmi {  
                Ok("Gateways configured supports only emi transaction.".to_string())  
            } else if isCardTransaction(txnCardInfo) {  
                let isBinEligible = utils::checkIfBinIsELigibleForEmi(&txnCardInfo.cardIsin, juspaybankCodeM, txnCardInfo.cardType.as_ref().map(|ct| cardTypeToText(ct)));  
                if isBinEligible {  
                    Ok(format!("No functional gateways supporting {}{}{} transaction.", emiBank, scope, emiType))  
                } else {  
                    Ok("Bin doesn't support emi transaction.".to_string())  
                }  
            } else {  
                Ok(format!("No functional gateways supporting {}{}{} transaction.", emiBank, scope, emiType))  
            }  
        }  
        "filterFunctionalGatewaysForPaymentMethod" => Ok(format!("No functional gateways supporting {} payment method.", txnCardInfo.paymentMethod)),  
        "filterFunctionalGatewaysForTokenProvider" => {  
            let vaultProvider = vaultProviderM.as_ref().unwrap_or(&Juspay);  
            Ok(format!("No functional gateways supporting {} saved cards.", vaultProvider))  
        }  
        "filterFunctionalGatewaysForWallet" => {  
            if txnCardInfo.cardType == Some(Wallet) {  
                Ok("No functional gateways supporting wallet transaction.".to_string())  
            } else {  
                Ok("Gateways configured supports only wallet transaction.".to_string())  
            }  
        }  
        "filterFunctionalGatewaysForNbOnly" => {  
            if txnCardInfo.cardType == Some(NB) {  
                Ok("No functional gateways supporting Net Banking transaction.".to_string())  
            } else {  
                Ok("Gateways configured supports only Net Banking transaction.".to_string())  
            }  
        }  
        "filterFunctionalGatewaysForConsumerFinance" => {  
            if txnCardInfo.paymentMethodType == ConsumerFinance {  
                Ok("No functional gateways supporting Consumer Finance transaction.".to_string())  
            } else {  
                Ok("Gateways configured supports only Consumer Finance transaction.".to_string())  
            }  
        }  
        "filterFunctionalGatewaysForUpi" => {  
            if txnCardInfo.paymentMethodType == UPI {  
                Ok("No functional gateways supporting UPI transaction.".to_string())  
            } else if !utils::isGooglePayTxn(txnCardInfo) {  
                Ok("Gateways configured supports only UPI transaction.".to_string())  
            } else {  
                Ok("No functional gateways".to_string())  
            }  
        }  
        "filterFunctionalGatewaysForTxnType" => {  
            if let Some(txnType) = mTxnType {  
                Ok(format!("No functional gateways supporting {} transaction.", txnType))  
            } else {  
                Ok("No functional gateways".to_string())  
            }  
        }  
        "filterFunctionalGatewaysForTxnDetailType" => Ok(format!("No functional gateways supporting {} transaction.", txnDetail.txnType)),  
        "filterFunctionalGatewaysForReward" => {  
            if txnCardInfo.cardType == Some(Reward) || txnCardInfo.paymentMethodType == ETPReward {  
                Ok("No functional gateways supporting Reward transaction.".to_string())  
            } else {  
                Ok("Gateways configured supports only Reward transaction.".to_string())  
            }  
        }  
        "filterFunctionalGatewaysForCash" => {  
            if txnCardInfo.paymentMethodType == Cash {  
                Ok("No functional gateways supporting CASH transaction.".to_string())  
            } else {  
                Ok("Gateways configured supports only CASH transaction.".to_string())  
            }  
        }  
        "filterFunctionalGatewaysForSplitSettlement" => Ok("No functional gateways after validating split.".to_string()),  
        "filterFunctionalGatewaysForOTMFlow" => Ok("No functional gateways after filtering for OTM flow.".to_string()),  
        "filterFunctionalGateways" => {  
            if isCardTransaction(txnCardInfo) {  
                if mInternalMeta.as_ref().and_then(|meta| meta.isCvvLessTxn).unwrap_or(false) && txnCardInfo.authType == Some(MOTO) {  
                    Ok(format!("No functional gateways supporting cvv less {} repeat moto transaction.", scope))  
                } else if mInternalMeta.as_ref().and_then(|meta| meta.isCvvLessTxn).unwrap_or(false) {  
                    Ok(format!("No functional gateways supporting cvv less {} transaction.", scope))  
                } else if utils::isTokenRepeatTxn(mInternalMeta) {  
                    Ok(format!("No functional gateways supporting {} transaction.", scope))  
                } else {  
                    Ok("No functional gateways supporting transaction.".to_string())  
                }  
            } else {  
                Ok("No functional gateways supporting transaction.".to_string())  
            }  
        }  
        "preferredGateway" => {  
            if let Some(preferredGateway) = preferredGatewayM {  
                if configuredGateways.contains(&preferredGateway) {  
                    Ok(format!("{} is not supporting this transaction.", preferredGateway))  
                } else {  
                    Ok(format!("{} is not configured.", preferredGateway))  
                }  
            } else {  
                Ok("No functional gateways supporting this transaction.".to_string())  
            }  
        }  
        "filterEnforcement" => Ok("Priority logic enforced gateways are not supporting this transaction.".to_string()),  
        "filterFunctionalGatewaysForMerchantRequiredFlow" => {  
            let paymentFlowList = utils::getPaymentFlowListFromTxnDetail(txnDetail);  
            let isMFOrder = paymentFlowList.contains(&"MUTUAL_FUND".to_string());  
            let isCBOrder = paymentFlowList.contains(&"CROSS_BORDER_PAYMENT".to_string());  
            let isSBMD = paymentFlowList.contains(&"SINGLE_BLOCK_MULTIPLE_DEBIT".to_string());  
            let message = if isMFOrder {  
                "Mutual Fund transaction flow"  
            } else if isCBOrder {  
                "Cross Border transaction flow"  
            } else if isSBMD {  
                "Single Block Multiple Debit"  
            } else {  
                "Merchant requested payment flows "  
            };  
            Ok(format!("No functional gateways after filtering for {}", message))  
        }  
        "filterGatewaysForMGASelectionIntegrity" => Ok("Conflicting configurations found or no functional gateways supporting this transaction".to_string()),  
        "filterGatewaysForEMITenureSpecficGatewayCreds" => Ok("No functional gateways supporting for emi.".to_string()),  
        "FilterFunctionalGatewaysForReversePennyDrop" => Ok("No functional gateways after filtering for Reverse Penny Drop transaction ".to_string()),  
        _ => Ok("No functional gateways supporting this transaction.".to_string()),  
    }  
} 
  
fn sortedFilterList(debugFilterList: &[T::DebugFilterEntry]) -> Vec<(String, Vec<Gateway>)> {  
    let mut list = filterList(debugFilterList);  
    list.sort_by(|(a, _), (b, _)| {  
        utils::deciderFilterOrder(a).cmp(&utils::deciderFilterOrder(b))  
    });  
    list  
}  
