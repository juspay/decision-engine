use std::collections::{HashSet, HashMap};  
use std::vec::Vec;  
use std::option::Option;  
use std::string::String;  
use std::sync::Arc;  
// use eulerhs::prelude::*;  
// use eulerhs::language::*;  
use crate::decider::gatewaydecider::types::*;  
// use gatewaydecider::constants as C;  
use crate::decider::gatewaydecider::utils as Utils;  
// use storage::utils::gatewaybankemisupport as S;  
// use storage::utils::gatewaycardinfo as S;  
// use storage::utils::merchantgatewayaccount as S;  
// use storage::utils::merchantgatewaycardinfo as S;  
// use storage::utils::txncardinfo as S;  
use crate::types::card as ETCa;  
use crate::types::currency as Curr;  
use crate::types::feature as ETF;  
use crate::types::gateway as ETG;  
use crate::types::gateway_bank_emi_support as ETGBES;  
use crate::types::gateway_bank_emi_support_v2 as ETGBESV2;  
use crate::types::gateway_card_info as ETGCI;  
use crate::types::merchant as ETM;  
use crate::types::merchant_gateway_account_sub_info as ETMGASI;  
use crate::types::merchant_gateway_card_info as ETMGCI;  
// use utils::redis::feature as Redis;  
use crate::types::order as ETO;  
use crate::types::payment as ETP;  
use crate::types::txn_details::types as ETTD;  
use crate::types::txn_offer_detail as ETTOD;  
use crate::types::txn_offer_info as ETTOI;  
use serde_json::Value as AValue;  
// use gatewaydecider::runner as GDR;  
// use juspay::extra::nonemptytext as NE;  
// use utils::redis::cache as RService;  
use crate::types::gateway as Gateway;  
// use db::common::types::paymentflows as PF;  
// use utils::config::merchantconfig as MerchantConfig;  
use crate::types::merchant_gateway_payment_method_flow as MGPMF;  
use crate::types::gateway_payment_method_flow as GPMF;  
use crate::types::tenant_config as TC;  
use crate::types::country::country_iso as ETCC;  
use crate::types::bank_code::find_bank_code;  

pub fn ord_nub<T>(v: Vec<T>) -> Vec<T>
where
    T: std::cmp::Ord + std::clone::Clone,
{
    let mut v_mut = v;
    v_mut.sort();
    v_mut.dedup();
    v_mut
}

// pub fn getGws() -> DeciderFlow<GatewayList> {  
//     gets(|state| state.functionalGateways.clone())  
// }

//pub fn getGws_rs(this: &DeciderFlow) -> GatewayList {
    //this.writer.functionalGateways.clone()
//}
  
// pub fn setGwsAndMgas(filteredMgas: Vec<ETM::MerchantGatewayAccount>) -> DeciderFlow<()> {  
//     Utils::setMgas(filteredMgas.clone());  
//     modify(|state| {  
//         state.functionalGateways = ord_nub(filteredMgas.iter().map(|mga| mga.gateway.clone()).collect());  
//     });  

// }

pub fn setGwsAndMgas_rs(this: &mut DeciderFlow, filteredMgas: Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>) -> () {
    Utils::set_mgas(this, filteredMgas.clone());
    this.writer.functionalGateways = ord_nub(filteredMgas.iter().map(|mga| mga.gateway.clone()).collect());
}

// pub fn setGws(gws: GatewayList) -> DeciderFlow<()> {  
//     let mMgas = Utils::getMgas();  
//     if let Some(mgas) = mMgas {  
//         Utils::setMgas(mgas.into_iter().filter(|val| gws.contains(&val.gateway)).collect());  
//     }  
//     modify(|state| {  
//         state.functionalGateways = gws;  
//     });  
// }

pub fn setGws_rs(this: &mut DeciderFlow, gws: GatewayList) -> () {
    let mMgas = Utils::get_mgas(this);
    if let Some(mgas) = mMgas {
        Utils::set_mgas(this, mgas.into_iter().filter(|val| gws.contains(&val.gateway)).collect());
    }
    this.writer.functionalGateways = gws;
}
  
// pub fn returnGwListWithLog(fName: DeciderFilterName, doOrNot: bool) -> DeciderFlow<GatewayList> {  
//     let fgws = getGws();  
//     let txnId = asks(|ctx| ctx.dpTxnDetail.txnId.clone());
//     log_debug("GW_Filtering", format!(  
//         "Functional gateways after {} for {} : {:?}",  
//         fName.to_string(),  
//         ETTD::transactionIdText(txnId),  
//         fgws  
//     ));  
//     if doOrNot {  
//         modify(|state| {  
//             state.debugFilterList.push(DebugFilterEntry {  
//                 name: makeFirstLetterSmall(fName.to_string()),  
//                 gateways: fgws.clone(),  
//             });  
//         });  
//     }  
//     fgws  
// }

// pub fn returnGwListWithLog_rs(this: &DeciderFlow, fName: DeciderFilterName, doOrNot: bool) -> GatewayList {
//     let fgws = getGws_rs(this);
//     let txnId = asks(|ctx| ctx.dpTxnDetail.txnId.clone());
//     // log_debug("GW_Filtering", format!(
//     //     "Functional gateways after {} for {} : {:?}",
//     //     fName.to_string(),
//     //     ETTD::transactionIdText(txnId),
//     //     fgws
//     // ));
//     if doOrNot {
//         this.writer.debugFilterList.push(DebugFilterEntry {
//             name: makeFirstLetterSmall(fName.to_string()),
//             gateways: fgws.clone(),
//         });
//     }
//     fgws
// }
  
fn makeFirstLetterSmall(s: String) -> String {  
    let mut chars = s.chars();  
    if let Some(first) = chars.next() {  
        first.to_lowercase().chain(chars).collect()  
    } else {  
        s  
    }  
}
  
// pub fn gwFiltersForEligibility() -> DeciderFlow<GatewayList> {  
//     let _ = getFunctionalGateways();  
//     let gws = filterFunctionalGateways();  
//     if gws.is_empty() {  
//         let txnId = asks(|ctx| ctx.dpTxnDetail.txnId.clone());  
//         let merchantId = asks(|ctx| ctx.dpTxnDetail.merchantId.clone());  
//         log_warning("GW_Filtering", format!(  
//             "There are no functional gateways for {} for merchant: {}",  
//             txnId, merchantId  
//         ));  
//         Utils::logGatewayDeciderApproach(None, None, vec![], NONE, None, vec![], None);  
//         vec![]  
//     } else {  
//         let _ = filterGatewaysForBrand();  
//         let _ = filterGatewaysForValidationType();  
//         let _ = filterGatewaysForEmi();  
//         let _ = filterGatewaysForPaymentMethod();  
//         let _ = filterGatewaysForConsumerFinance();  
//         let _ = filterFunctionalGatewaysForMerchantRequiredFlow();  
//         let _ = filterGatewaysForMGASelectionIntegrity();  
//         logFinalFunctionalGateways()  
//     }  
// }

// pub fn gwFiltersForEligibility_rs(this: &DeciderFlow) -> GatewayList {
//     let _ = getFunctionalGateways_rs(this);
//     let gws = filterFunctionalGateways_rs(this);
//     if gws.is_empty() {
//         let txnId = asks(|ctx| ctx.dpTxnDetail.txnId.clone());
//         let merchantId = asks(|ctx| ctx.dpTxnDetail.merchantId.clone());
//         // log_warning("GW_Filtering", format!(
//         //     "There are no functional gateways for {} for merchant: {}",
//         //     txnId, merchantId
//         // ));
//         // Utils::logGatewayDeciderApproach(None, None, vec![], NONE, None, vec![], None);
//         vec![]
//     } else {
//         let _ = filterGatewaysForBrand_rs(this);
//         let _ = filterGatewaysForValidationType_rs(this);
//         let _ = filterGatewaysForEmi_rs(this);
//         let _ = filterGatewaysForPaymentMethod_rs(this);
//         let _ = filterGatewaysForConsumerFinance_rs(this);
//         let _ = filterFunctionalGatewaysForMerchantRequiredFlow_rs(this);
//         let _ = filterGatewaysForMGASelectionIntegrity_rs(this);
//         // logFinalFunctionalGateways_rs(this)
//     }
// }
  
// pub fn newGwFilters() -> DeciderFlow<(GatewayList, Vec<ETM::MerchantGatewayAccount>)> {  
//     let _ = getFunctionalGateways();  
//     let gws = filterFunctionalGateways();  
//     if gws.is_empty() {  
//         let txnId = asks(|ctx| ctx.dpTxnDetail.txnId.clone());  
//         let merchantId = asks(|ctx| ctx.dpTxnDetail.merchantId.clone());  
//         log_warning("GW_Filtering", format!(  
//             "There are no functional gateways for {} for merchant: {}",  
//             ETTD::transactionIdText(txnId),  
//             merchantId  
//         ));  
//         Utils::logGatewayDeciderApproach(None, None, vec![], NONE, None, vec![], None);  
//         (vec![], vec![])  
//     } else {  
//         let _ = filterGatewaysForBrand();  
//         let _ = filterGatewaysForAuthType();  
//         let _ = filterGatewaysForValidationType();  
//         let _ = filterGatewaysForEmi();  
//         let _ = filterGatewaysForTxnOfferDetails();  
//         let _ = filterGatewaysForPaymentMethod();  
//         let _ = filterGatewaysForTokenProvider();  
//         let _ = filterGatewaysForWallet();  
//         let _ = filterGatewaysForNbOnly();  
//         let _ = filterGatewaysForConsumerFinance();  
//         let _ = filterGatewaysForUpi();  
//         let _ = filterGatewaysForTxnType();  
//         let _ = filterGatewaysForTxnDetailType();  
//         let _ = filterGatewaysForReward();  
//         let _ = filterGatewaysForCash();  
//         let _ = filterFunctionalGatewaysForSplitSettlement();  
//         let _ = filterFunctionalGatewaysForMerchantRequiredFlow();  
//         let _ = filterFunctionalGatewaysForOTMFlow();  
//         let _ = filterGatewaysForMGASelectionIntegrity();  
//         let funcGateways = logFinalFunctionalGateways();  
//         let allMgas = Utils::getIsMerchantEnabedForDynamicMGASelection()  
//             .then(|| Utils::getMgas())  
//             .unwrap_or(None);  
//         (funcGateways, allMgas.unwrap_or_default())  
//     }  
// }  
  
// pub fn getFunctionalGateways() -> DeciderFlow<GatewayList> {  
//     let txnCardInfo = asks(|ctx| ctx.dpTxnCardInfo.clone());  
//     let oref = asks(|ctx| ctx.dpOrder.clone());  
//     let macc = asks(|ctx| ctx.dpMerchantAccount.clone());  
//     let txnId = asks(|ctx| ctx.dpTxnDetail.txnId.clone());  
//     let txnDetail = asks(|ctx| ctx.dpTxnDetail.clone());  
//     let isEdccApplied = asks(|ctx| ctx.dpEDCCApplied.clone());  
//     let enforceGatewayList = asks(|ctx| ctx.dpEnforceGatewayList.clone());  
  
//     log_info(format!(  
//         "enableGatewayReferenceIdBasedRouting is enable or not for txn_id : {}",  
//         txnId  
//     ), format!(  
//         "enableGatewayReferenceIdBasedRouting: {}",  
//         macc.enableGatewayReferenceIdBasedRouting  
//     ));  
  
//     let (meta, plRefIdMap) = Utils::getOrderMetadataAndPLRefIdMap(  
//         macc.enableGatewayReferenceIdBasedRouting,  
//         oref.clone()  
//     );  
  
//     let proceedWithAllMgas = Utils::isEnabledForAllMgas();  
//     let enabledGatewayAccounts = if proceedWithAllMgas {  
//         S::getAllEnabledMgasByMerchantId(macc.merchantId.clone())  
//     } else {  
//         let possibleRefIdsOfMerchant = Utils::getAllPossibleRefIds(meta.clone(), oref.clone(), plRefIdMap.clone());  
//         S::getEnabledMgasByMerchantIdAndRefId(macc.merchantId.clone(), possibleRefIdsOfMerchant)  
//     };  
  
//     let paymentFlowList = Utils::getPaymentFlowListFromTxnDetail(txnDetail.clone());  
//     Utils::setPaymentFlowList(paymentFlowList);  
  
//     let mgas = match (txnDetail.isEmi, enforceGatewayList.clone()) {  
//         (false, _) => enabledGatewayAccounts.clone(),  
//         (_, None) => enabledGatewayAccounts.clone(),  
//         (_, Some(enGatewayList)) if enGatewayList.is_empty() => enabledGatewayAccounts.clone(),  
//         (_, Some(enGatewayList)) => enabledGatewayAccounts.into_iter()  
//             .filter(|mga| enGatewayList.contains(&mga.gateway))  
//             .collect(),  
//     };  
  
//     let edccSupportedGateways = RService::findByNameFromRedis(C::EDCC_SUPPORTED_GATEWAYS)  
//         .unwrap_or_default();  
  
//     let mgas = if txnDetail.currency != oref.currency && isEdccApplied.unwrap_or(false) {  
//         mgas.into_iter()  
//             .filter(|mga| {  
//                 edccSupportedGateways.contains(&mga.gateway)  
//                     && Utils::checkIfEnabledInMga(mga, "DYNAMIC_CURRENCY_CONVERSION", "isEdccSupported")  
//             })  
//             .collect()  
//     } else {  
//         mgas  
//     };  
  
//     let mgas = if proceedWithAllMgas {  
//         mgas  
//     } else {  
//         mgas.into_iter()  
//             .filter(|mga| {  
//                 let gwRefId = Utils::getGatewayReferenceId(meta.clone(), mga.gateway.clone(), oref.clone(), plRefIdMap.clone());  
//                 mga.referenceId == gwRefId  
//             })  
//             .collect()  
//     };  
  
//     validateAndSetDynamicMGAFlag(proceedWithAllMgas, mgas.clone());  
  
//     let mgas = if proceedWithAllMgas {  
//         mgas  
//     } else {  
//         filterMGAsByEnforcedPaymentFlows(mgas)  
//     };  
  
//     if mgas.is_empty() {  
//         setGwsAndMgas(vec![]);  
//         returnGwListWithLog(GetFunctionalGateways, true)  
//     } else {  
//         let currencyFilteredMgas = mgas.into_iter()  
//             .filter(|mga| currencyFilter(txnDetail.currency.clone(), mga.clone()))  
//             .collect::<Vec<_>>();  
  
//         let rpdFilterMGAs = if Utils::isReversePennyDropTxn(txnDetail.clone()) {  
//             currencyFilteredMgas.into_iter()  
//                 .filter(|mga| Utils::checkForReversePennyDropinMGA(mga.clone()))  
//                 .collect()  
//         } else {  
//             currencyFilteredMgas  
//         };  
  
//         returnGwListWithLog(FilterFunctionalGatewaysForCurrency, true);  
  
//         let filteredMgas = if proceedWithAllMgas {  
//             rpdFilterMGAs  
//         } else {  
//             rpdFilterMGAs.into_iter()  
//                 .filter(|mga| {  
//                     let mgaEligibleSeamlessGateways = RService::findByNameFromRedis(C::MGA_ELIGIBLE_SEAMLESS_GATEWAYS)  
//                         .unwrap_or_default();  
//                     isMgaEligible(mga.clone(), txnCardInfo.clone(), txnDetail.txnObjectType.clone(), mgaEligibleSeamlessGateways)  
//                 })  
//                 .collect()  
//         };  
  
//         setGwsAndMgas(filteredMgas.clone());  
//         returnGwListWithLog(FilterFunctionalGatewaysForReversePennyDrop, true)  
//     }  
// }  
  
// fn validateAndSetDynamicMGAFlag(proceedWithAllMgas: bool, mgas: Vec<ETM::MerchantGatewayAccount>) {  
//     let gwts = mgas.iter().map(|mga| mga.gateway.clone()).collect::<Vec<_>>();  
//     if !proceedWithAllMgas && gwts.len() != gwts.iter().collect::<HashSet<_>>().len() {  
//         Utils::setIsMerchantEnabedForDynamicMGASelection(true);  
//     }  
// }  


// pub fn filterMGAsByEnforcedPaymentFlows(  
//     initialMGAS: Vec<MerchantGatewayAccount>,  
// ) -> DeciderFlow<Vec<MerchantGatewayAccount>> {  
//     let gateways: HashSet<Gateway> = initialMGAS.iter().map(|mga| mga.gateway.clone()).collect();  
//     let txnCardInfo = asks(|ctx| ctx.dpTxnCardInfo.clone());  
//     let oref = asks(|ctx| ctx.dpOrder.clone());  
//     let macc = asks(|ctx| ctx.dpMerchantAccount.clone());  
//     let txnDetail = asks(|ctx| ctx.dpTxnDetail.clone());  
//     let (meta, plRefIdMap) = getOrderMetadataAndPLRefIdMap(macc.enableGatewayReferenceIdBasedRouting, &oref);  
//     let txnPaymentFlows: HashSet<Text> = getPaymentFlowListFromTxnDetail(&txnDetail).into_iter().collect();  
  
//     let eligibleMgas = gateways  
//         .into_iter()  
//         .map(|gateway| evaluatePaymentFlowEnforcement(&meta, &oref, &plRefIdMap, &txnPaymentFlows, &gateway, &initialMGAS))  
//         .collect::<Vec<_>>();  
  
//     eligibleMgas.into_iter().flatten().collect()  
// }  
  
// fn evaluatePaymentFlowEnforcement(  
//     meta: &Meta,  
//     oref: &Order,  
//     plRefIdMap: &PLRefIdMap,  
//     txnPaymentFlows: &HashSet<Text>,  
//     gateway: &Gateway,  
//     initialMGAS: &[MerchantGatewayAccount],  
// ) -> Vec<MerchantGatewayAccount> {  
//     let gwRefId = getGatewayReferenceId(meta, gateway, oref, plRefIdMap);  
//     let filteredMgas: Vec<MerchantGatewayAccount> = initialMGAS  
//         .iter()  
//         .filter(|mga| mga.referenceId == gwRefId && mga.gateway == *gateway)  
//         .cloned()  
//         .collect();  
  
//     let enforcedPaymentFlows: HashSet<Text> = filteredMgas  
//         .iter()  
//         .flat_map(|mga| mga.supportedPaymentFlows.as_ref().and_then(|flows| flows.enforcedPaymentFlows.clone()))  
//         .flatten()  
//         .collect();  
  
//     if !enforcedPaymentFlows.is_empty() {  
//         let flowsToEnforce: HashSet<Text> = txnPaymentFlows.intersection(&enforcedPaymentFlows).cloned().collect();  
//         filteredMgas  
//             .into_iter()  
//             .filter(|mga| {  
//                 let mgaEnforcedFlows: HashSet<Text> = mga  
//                     .supportedPaymentFlows  
//                     .as_ref()  
//                     .and_then(|flows| flows.enforcedPaymentFlows.clone())  
//                     .unwrap_or_default()  
//                     .into_iter()  
//                     .collect();  
//                 areEqualArrays(&mgaEnforcedFlows, &flowsToEnforce)  
//             })  
//             .collect()  
//     } else {  
//         filteredMgas  
//     }  
// }  
  
// fn areEqualArrays<T: Eq + Ord>(xs: &HashSet<T>, ys: &HashSet<T>) -> bool {  
//     xs == ys  
// }  
  
// pub fn currencyFilter(orderCurr: Currency, mga: &MerchantGatewayAccount) -> bool {  
//     match getTrueString(&mga.supportedCurrencies) {  
//         Some(supportedCurrencies) => canAcceptCurrency(&supportedCurrencies, orderCurr),  
//         None => orderCurr == Currency::INR,  
//     }  
// }  
  
// fn canAcceptCurrency(supportedMgaCurrencies: &Text, currency: Currency) -> bool {  
//     let currListT: Option<Vec<Text>> = readMaybe(&supportedMgaCurrencies.to_string());  
//     let currList: Vec<Currency> = currListT  
//         .unwrap_or_default()  
//         .into_iter()  
//         .filter_map(|curr| textCurrency(&curr))  
//         .collect();  
  
//     currList.contains(&currency) || (currList.is_empty() && currency == Currency::INR)  
// }  
  
// pub fn isMgaEligible(  
//     mga: &MerchantGatewayAccount,  
//     txnCI: &TxnCardInfo,  
//     mTxnObjType: TxnObjectType,  
//     mgaEligibleSeamlessGateways: &[Gateway],  
// ) -> bool {  
//     validateMga(mga, txnCI, mTxnObjType, mgaEligibleSeamlessGateways)  
// }  
  
// fn validateMga(  
//     mga: &MerchantGatewayAccount,  
//     txnCI: &TxnCardInfo,  
//     mTxnObjType: TxnObjectType,  
//     mgaEligibleSeamlessGateways: &[Gateway],  
// ) -> bool {  
//     if mgaEligibleSeamlessGateways.contains(&mga.gateway) && isCardOrNbTxn(txnCI) {  
//         isSeamless(mga)  
//     } else if isMandateRegister(mTxnObjType) {  
//         isSubscription(mga)  
//     } else if isEmandateRegister(mTxnObjType) {  
//         isEmandateEnabled(mga)  
//     } else if isOnlySubscription(mga) {  
//         false  
//     } else {  
//         true  
//     }  
// }  
  
// fn isCardOrNbTxn(txnCI: &TxnCardInfo) -> bool {  
//     isCardTransaction(txnCI) || isNbTransaction(txnCI)  
// }  
  
// pub fn isEmandateRegister(mTxnObjType: TxnObjectType) -> bool {  
//     mTxnObjType == TxnObjectType::EmandateRegister  
// }  
  
// pub fn isMandateRegister(mTxnObjType: TxnObjectType) -> bool {  
//     mTxnObjType == TxnObjectType::MandateRegister  
// }

// pub async fn filterFunctionalGateways() -> Result<(), Box<dyn std::error::Error>> {  
//     let txnDetail = utils::get_dp_txn_detail().await?;  
//     let txnCardInfo = utils::get_dp_txn_card_info().await?;  
//     let mAcc = utils::get_dp_merchant_account().await?;  
//     let mInternalMeta: Option<InternalMetadata> = txnDetail.internalMetadata.as_ref()  
//         .and_then(|meta| serde_json::from_str(meta).ok());  
  
//     utils::set_internal_metadata(mInternalMeta.clone()).await;  
  
//     // CVV Less Gateway Validations  
//     if utils::is_card_transaction(&txnCardInfo).await {  
//         if let Some(true) = mInternalMeta.as_ref().and_then(|meta| meta.isCvvLessTxn) {  
//             if txnCardInfo.authType == Some(utils::make_secret(ETCa::MOTO)) {  
//                 let st = utils::get_gws().await?;  
//                 let authTypeRestrictedGateways: HashMap<String, Vec<Gateway>> = RService::find_by_name_from_redis(C::AUTH_TYPE_RESTRICTED_GATEWAYS).await?.unwrap_or_default();  
//                 let motoSupportedGateways = txnCardInfo.authType.as_ref()  
//                     .and_then(|auth_type| authTypeRestrictedGateways.get(&utils::unsafe_extract_secret(auth_type)))  
//                     .cloned()  
//                     .unwrap_or_default();  
//                 let filtered_gateways: Vec<Gateway> = st.into_iter()  
//                     .filter(|gw| motoSupportedGateways.contains(gw))  
//                     .collect();  
//                 utils::log_info("filterFunctionalGateways", format!("Functional gateways after filtering for MOTO cvvLessTxns support for txn_id: {}", txnDetail.txnId));  
//                 utils::set_gws(filtered_gateways).await?;  
//             } else {  
//                 if utils::is_token_repeat_txn(&mInternalMeta).await {  
//                     let brand = txnCardInfo.cardSwitchProvider.as_ref()  
//                         .map(|provider| provider.to_uppercase())  
//                         .unwrap_or_else(|| "DEFAULT".to_string());  
//                     let isMerchantEnabledForCvvLessV2Flow = RService::is_redis_feature_enabled(C::CVVLESS_V2_FLOW, &mAcc.merchantId.unMerchantId.to_text()).await?;  
//                     if isMerchantEnabledForCvvLessV2Flow {  
//                         let configResp = MerchantConfig::is_payment_flow_enabled_with_hierarchy_check(  
//                             mAcc.id,  
//                             mAcc.tenantAccountId,  
//                             TC::MERCHANT_CONFIG,  
//                             PF::CVVLESS,  
//                             None,  
//                         ).await?;  
//                         let functionalGateways = if !configResp {  
//                             utils::log_error("CVVLESS-ERROR", "CVVLESS_FLOW_DISABLED");  
//                             Vec::new()  
//                         } else {  
//                             let st = utils::get_gws().await?;  
//                             let mgaList = utils::get_mgas().await?.unwrap_or_default();  
//                             let isBrandSupportsCvvlessTR = is_brand_supports_cvvless(&txnCardInfo, &brand).await?;  
//                             if isBrandSupportsCvvlessTR {  
//                                 let mPmEntryDB = ETP::get_by_name(&brand).await?;  
//                                 if let Some(cardPaymentMethod) = mPmEntryDB {  
//                                     let uniqueGwLs: HashSet<Gateway> = st.into_iter().collect();  
//                                     let allGPMfEntries = GPMF::find_all_gpmf_by_gateway_payment_flow_payment_method(  
//                                         uniqueGwLs.clone(),  
//                                         cardPaymentMethod.id,  
//                                         PF::CVVLESS,  
//                                     ).await?;  
//                                     let gmpfGws: HashSet<Gateway> = allGPMfEntries.iter()  
//                                         .map(|gpmf| gpmf.gateway.clone())  
//                                         .collect();  
//                                     let filteredMga: Vec<MerchantGatewayAccount> = mgaList.into_iter()  
//                                         .filter(|mga| gmpfGws.contains(&mga.gateway))  
//                                         .collect();  
//                                     let mgaIds: Vec<String> = filteredMga.iter()  
//                                         .map(|mga| mga.id.merchantGwAccId.clone())  
//                                         .collect();  
//                                     let gpmfIds: Vec<String> = allGPMfEntries.iter()  
//                                         .map(|gpmf| gpmf.id.clone())  
//                                         .collect();  
//                                     let mgpmfEntries = MGPMF::get_all_mgpmf_by_mga_id_and_gpmf_ids(mgaIds, gpmfIds).await?;  
//                                     let filteredMgaList: HashSet<String> = mgpmfEntries.iter()  
//                                         .map(|mgpmf| mgpmf.merchantGatewayAccountId.clone())  
//                                         .collect();  
//                                     let finalFilteredMga: Vec<MerchantGatewayAccount> = filteredMga.into_iter()  
//                                         .filter(|mga| filteredMgaList.contains(&mga.id.merchantGwAccId))  
//                                         .collect();  
//                                     utils::set_mgas(finalFilteredMga.clone()).await?;  
//                                     finalFilteredMga.into_iter()  
//                                         .map(|mga| mga.gateway)  
//                                         .collect()  
//                                 } else {  
//                                     Vec::new()  
//                                 }  
//                             } else {  
//                                 Vec::new()  
//                             }  
//                         };  
//                         utils::log_info("filterFunctionalGateways", format!("Functional gateways after filtering for token repeat cvvLessTxns support for txn_id: {}", txnDetail.txnId));  
//                         utils::set_gws(functionalGateways).await?;  
//                     } else {  
//                         let isBrandSupportsCvvlessTR = is_brand_supports_cvvless(&txnCardInfo, &brand).await?;  
//                         let functionalGateways = if isBrandSupportsCvvlessTR {  
//                             let mTokenRepeatCvvlessSupportedGateways = utils::get_token_supported_gateways(&txnDetail, &txnCardInfo, "CVV_LESS", &mInternalMeta).await?;  
//                             let filteredGatewayFromMerchantConfig = utils::filtered_gateways_merchant_config(  
//                                 mTokenRepeatCvvlessSupportedGateways.clone(),  
//                                 PF::CVVLESS,  
//                                 &mAcc,  
//                                 &brand,  
//                             ).await?;  
//                             let tokenRepeatCvvlessSupportedGateways = filteredGatewayFromMerchantConfig.unwrap_or_default();  
//                             let st = utils::get_gws().await?;  
//                             st.into_iter()  
//                                 .filter(|gw| tokenRepeatCvvlessSupportedGateways.contains(gw))  
//                                 .collect()  
//                         } else {  
//                             Vec::new()  
//                         };  
//                         utils::log_info("filterFunctionalGateways", format!("Functional gateways after filtering for token repeat cvvLessTxns support for txn_id: {}", txnDetail.txnId));  
//                         utils::set_gws(functionalGateways).await?;  
//                     }  
//                 } else {  
//                     let cardBrandToCvvLessTxnSupportedGateways: HashMap<String, Vec<Gateway>> = RService::find_by_name_from_redis(C::CARD_BRAND_TO_CVVLESS_TXN_SUPPORTED_GATEWAYS).await?.unwrap_or_default();  
//                     let cvvLessTxnSupportedCommonGateways: Vec<Gateway> = RService::find_by_name_from_redis(C::CVVLESS_TXN_SUPPORTED_COMMON_GATEWAYS).await?.unwrap_or_default();  
//                     let cvvLessTxnSupportedGateways = cvvLessTxnSupportedCommonGateways.into_iter()  
//                         .chain(cardBrandToCvvLessTxnSupportedGateways.get(&txnCardInfo.paymentMethod).cloned().unwrap_or_default())  
//                         .collect::<HashSet<_>>()  
//                         .into_iter()  
//                         .collect::<Vec<_>>();  
//                     if !cvvLessTxnSupportedGateways.is_empty() {  
//                         let st = utils::get_gws().await?;  
//                         let filtered_gateways: Vec<Gateway> = st.into_iter()  
//                             .filter(|gw| cvvLessTxnSupportedGateways.contains(gw))  
//                             .collect();  
//                         utils::log_info("filterFunctionalGateways", format!("Functional gateways after filtering for cvvLessTxns for txn_id: {}", txnDetail.txnId));  
//                         utils::set_gws(filtered_gateways).await?;  
//                     }  
//                 }  
//             }  
//         }  
//     }  
  
//     // Card token based repeat transaction gateway filter  
//     if utils::is_card_transaction(&txnCardInfo).await && utils::is_token_repeat_txn(&mInternalMeta).await {  
//         if let Some(secAuthType) = txnCardInfo.authType {  
//             if utils::unsafe_extract_secret(&secAuthType) == ETCa::OTP {  
//                 let mTokenRepeatOtpSupportedGateways = utils::get_token_supported_gateways(&txnDetail, &txnCardInfo, "OTP", &mInternalMeta).await?;  
//                 let st = utils::get_gws().await?;  
//                 let tokenRepeatOtpSupportedGateways = mTokenRepeatOtpSupportedGateways.unwrap_or_default();  
//                 let filtered_gateways: Vec<Gateway> = st.into_iter()  
//                     .filter(|gw| tokenRepeatOtpSupportedGateways.contains(gw))  
//                     .collect();  
//                 utils::set_gws(filtered_gateways).await?;  
//             }  
//         }  
//         let mTokenRepeatSupportedGateways = utils::get_token_supported_gateways(&txnDetail, &txnCardInfo, "CARD", &mInternalMeta).await?;  
//         let st = utils::get_gws().await?;  
//         let tokenRepeatSupportedGateways = mTokenRepeatSupportedGateways.unwrap_or_default();  
//         let filtered_gateways: Vec<Gateway> = if tokenRepeatSupportedGateways.is_empty() {  
//             st  
//         } else {  
//             st.into_iter()  
//                 .filter(|gw| tokenRepeatSupportedGateways.contains(gw))  
//                 .collect()  
//         };  
//         utils::set_gws(filtered_gateways).await?;  
//     }  
  
//     // Amex BTA Card based gateway filter  
//     if utils::is_card_transaction(&txnCardInfo).await && txnCardInfo.authType == Some(utils::make_secret(ETCa::MOTO)) {  
//         let paymentFlowList = utils::get_payment_flow_list_from_txn_detail(&txnDetail).await?;  
//         let st = utils::get_gws().await?;  
//         if paymentFlowList.contains(&"TA_FILE".to_string()) {  
//             let taOfflineEnabledGateways: HashSet<Gateway> = RService::find_by_name_from_redis(C::TA_OFFLINE_ENABLED_GATEWAYS).await?.unwrap_or_default().into_iter().collect();  
//             let filtered_gateways: Vec<Gateway> = st.into_iter()  
//                 .filter(|gw| taOfflineEnabledGateways.contains(gw))  
//                 .collect();  
//             utils::set_gws(filtered_gateways).await?;  
//         }  
//     }  
  
//     let st = utils::get_gws().await?;  
//     utils::log_debug("filterFunctionalGateways", format!("Functional gateways before filtering for MerchantContainer for txn_id: {}", txnDetail.txnId));  
//     let merchantContainerSupportedGateways: Vec<Gateway> = RService::find_by_name_from_redis(C::MERCHANT_CONTAINER_SUPPORTED_GATEWAYS).await?.unwrap_or_default();  
//     let filtered_gateways: Vec<Gateway> = if txnCardInfo.paymentMethodType == ETP::MerchantContainer {  
//         st.into_iter()  
//             .filter(|gw| merchantContainerSupportedGateways.contains(gw))  
//             .collect()  
//     } else {  
//         st.into_iter()  
//             .filter(|gw| !merchantContainerSupportedGateways.contains(gw))  
//             .collect()  
//     };  
//     utils::set_gws(filtered_gateways).await?;  
//     utils::return_gw_list_with_log("FilterFunctionalGateways", true).await?;  
//     Ok(())  
// }  
  
// async fn is_brand_supports_cvvless(txnCardInfo: &TxnCardInfo, brand: &str) -> Result<bool, Box<dyn std::error::Error>> {  
//     if brand == "RUPAY" {  
//         check_cvv_less_support_rupay(txnCardInfo).await  
//     } else {  
//         Ok(true)  
//     }  
// }  
  
// async fn check_cvv_less_support_rupay(txnCardInfo: &TxnCardInfo) -> Result<bool, Box<dyn std::error::Error>> {  
//     let bankCode = utils::fetch_juspay_bank_code(txnCardInfo).await?;  
//     let mCardType = txnCardInfo.cardType.as_ref().map(|card_type| ETCa::card_type_to_text(card_type));  
//     if let Some(bCode) = bankCode {  
//         let feature_key = C::get_token_repeat_cvv_less_bank_code_key(&txnCardInfo.cardSwitchProvider);  
//         let dimension = format!("{}::{}", bCode, mCardType.unwrap_or_default().to_uppercase());  
//         Redis::is_feature_enabled_by_dimension(&feature_key, &dimension).await  
//     } else {  
//         Ok(false)  
//     }  
// }  

// pub async fn filterGatewaysForBrand() -> DeciderFlow<GatewayList> {  
//     let st = getGws().await;  
//     let cardBrand = getCardBrand().await;  
//     let new_st = filterByCardBrand(st, cardBrand).await;  
//     setGws(new_st).await;  
//     returnGwListWithLog(FilterFunctionalGatewaysForBrand, true).await  
// }  
  
// async fn filterByCardBrand(st: GatewayList, cardBrand: Option<String>) -> GatewayList {  
//     let amexSupportedGateways: HashSet<Gateway> = findByNameFromRedis(AMEX_SUPPORTED_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
//     let amexNotSupportedGateways: HashSet<Gateway> = findByNameFromRedis(AMEX_NOT_SUPPORTED_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
//     let sodexoOnlyGateways: HashSet<Gateway> = findByNameFromRedis(SODEXO_ONLY_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
//     let sodexoAlsoGateways: HashSet<Gateway> = findByNameFromRedis(SODEXO_ALSO_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
  
//     match cardBrand.as_deref() {  
//         Some("AMEX") => st.into_iter().filter(|gw| amexSupportedGateways.contains(gw)).collect(),  
//         Some("SODEXO") => st.into_iter().filter(|gw| sodexoOnlyGateways.contains(gw) || sodexoAlsoGateways.contains(gw)).collect(),  
//         Some(_) if cardBrand != Some("SODEXO".to_string()) => st.into_iter()  
//             .filter(|gw| !amexNotSupportedGateways.contains(gw))  
//             .filter(|gw| !sodexoOnlyGateways.contains(gw))  
//             .collect(),  
//         _ => st.into_iter().filter(|gw| !amexNotSupportedGateways.contains(gw)).collect(),  
//     }  
// }  
  
// pub async fn filterGatewaysForAuthType() -> DeciderFlow<GatewayList> {  
//     let st = getGws().await;  
//     let m_mgas = getMgas().await;  
//     let mga_list = m_mgas.unwrap_or_default();  
//     let txn_detail = dpTxnDetail().await;  
//     let txn_card_info = dpTxnCardInfo().await;  
//     let macc = dpMerchantAccount().await;  
//     let dynamic_mga_enabled = getIsMerchantEnabledForDynamicMGASelection().await;  
  
//     if let Some(card_isin) = txn_card_info.cardIsin {  
//         if txn_card_info.authType == Some(makeSecret(ETCa::OTP)) {  
//             setGwsAndMgas(  
//                 mga_list.into_iter()  
//                     .filter(|mga| checkIfEnabledInMga(mga, "CARD_DOTP", "cardDirectOtpEnabled") && st.contains(&mga.gateway))  
//                     .collect(),  
//             ).await;  
//         }  
//         if txn_card_info.authType == Some(makeSecret(ETCa::MOTO)) {  
//             setGwsAndMgas(  
//                 mga_list.into_iter()  
//                     .filter(|mga| checkIfEnabledInMga(mga, "CARD_MOTO", "cardMotoEnabled") && st.contains(&mga.gateway))  
//                     .collect(),  
//             ).await;  
//         }  
//         if txn_card_info.authType == Some(makeSecret(ETCa::NO_THREE_DS)) {  
//             setGwsAndMgas(  
//                 mga_list.into_iter()  
//                     .filter(|mga| checkIfNoDsEnabledInMga(mga, "CARD_NO_3DS", "cardNo3DsEnabled") && st.contains(&mga.gateway))  
//                     .collect(),  
//             ).await;  
//         }  
//         if txn_card_info.authType == Some(makeSecret(ETCa::VIES)) {  
//             setGwsAndMgas(  
//                 mga_list.into_iter()  
//                     .filter(|mga| isViesEnabled(mga) && st.contains(&mga.gateway))  
//                     .collect(),  
//             ).await;  
//         }  
  
//         let mb_feature = getFeatureEnabled("DISABLE_DECIDER_BIN_ELIGIBILITY_CHECK", macc.merchantId, true).await;  
//         if mb_feature.is_none() {  
//             let stt = getGws().await;  
//             let atm_pin_card_info_restricted_gateways: HashSet<Gateway> = findByNameFromRedis(ATM_PIN_CARD_INFO_RESTRICTED_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
//             let otp_card_info_restricted_gateways: HashSet<Gateway> = findByNameFromRedis(OTP_CARD_INFO_RESTRICTED_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
//             let otp_card_info_supported_gateways: HashSet<Gateway> = findByNameFromRedis(OTP_CARD_INFO_SUPPORTED_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
//             let moto_card_info_supported_gateways: HashSet<Gateway> = findByNameFromRedis(MOTO_CARD_INFO_SUPPORTED_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
//             let auth_type_restricted_gateways: HashMap<AuthType, GatewayList> = findByNameFromRedis(AUTH_TYPE_RESTRICTED_GATEWAYS).await.unwrap_or_default().into_iter().collect();  
  
//             let (card_info_check_needed_gateways, card_info_check_not_needed_gateways) = partition(  
//                 stt,  
//                 |gw| isGatewayCardInfoCheckNeeded(&txn_card_info, &atm_pin_card_info_restricted_gateways, &otp_card_info_supported_gateways, &moto_card_info_supported_gateways, gw),  
//             );  
  
//             let auth_type_supported_gws: GatewayList = card_info_check_not_needed_gateways.into_iter()  
//                 .filter(|gw| isAuthTypeSupportedGateway(&txn_card_info, &atm_pin_card_info_restricted_gateways, &otp_card_info_restricted_gateways, &auth_type_restricted_gateways, gw))  
//                 .collect();  
  
//             let gci_validated_gws = isAuthTypeSupported(&macc, card_isin, card_info_check_needed_gateways, txn_card_info.authType).await;  
  
//             logDebugV("filterGatewaysForAuthType", format!(  
//                 "Functional gateways after filtering after DISABLE_DECIDER_BIN_ELIGIBILITY_CHECK check: {:?}",  
//                 gci_validated_gws.iter().chain(auth_type_supported_gws.iter()).collect::<Vec<_>>(),  
//             ));  
  
//             setGws(gci_validated_gws.into_iter().chain(auth_type_supported_gws.into_iter()).collect()).await;  
//         }  
//     }  
  
//     returnGwListWithLog(FilterFunctionalGatewaysForAuthType, true).await  
// }  
  
// fn isGatewayCardInfoCheckNeeded(  
//     txn_card_info: &TxnCardInfo,  
//     atm_pin_card_info_restricted_gateways: &HashSet<Gateway>,  
//     otp_card_info_supported_gateways: &HashSet<Gateway>,  
//     moto_card_info_supported_gateways: &HashSet<Gateway>,  
//     gateway: &Gateway,  
// ) -> bool {  
//     (txn_card_info.authType == Some(makeSecret(ETCa::ATMPIN)) && atm_pin_card_info_restricted_gateways.contains(gateway))  
//         || (txn_card_info.authType == Some(makeSecret(ETCa::OTP)) && otp_card_info_supported_gateways.contains(gateway))  
//         || (txn_card_info.authType == Some(makeSecret(ETCa::MOTO)) && moto_card_info_supported_gateways.contains(gateway))  
// }  
  
// fn isAuthTypeSupportedGateway(  
//     txn_card_info: &TxnCardInfo,  
//     atm_pin_card_info_restricted_gateways: &HashSet<Gateway>,  
//     otp_card_info_restricted_gateways: &HashSet<Gateway>,  
//     auth_type_restricted_gateways: &HashMap<AuthType, GatewayList>,  
//     gateway: &Gateway,  
// ) -> bool {  
//     match auth_type_restricted_gateways.get(&unsafeExtractSecret(txn_card_info.authType.unwrap_or_default())) {  
//         Some(gws) => gws.contains(gateway),  
//         None => txn_card_info.authType == Some(makeSecret(ETCa::VIES))  
//             || (!txn_card_info.authType.map_or(false, |auth| auth != makeSecret(ETCa::ATMPIN) && atm_pin_card_info_restricted_gateways.contains(gateway))  
//                 && !txn_card_info.authType.map_or(false, |auth| auth != makeSecret(ETCa::OTP) && otp_card_info_restricted_gateways.contains(gateway))),  
//     }  
// }  
  
// pub async fn isAuthTypeSupported(  
//     ma: &MerchantAccount,  
//     cardbin: String,  
//     gws: GatewayList,  
//     mauth: Option<Secret<AuthType>>,  
// ) -> GatewayList {  
//     let bin_list = getBinList(Some(cardbin));  
//     let enabled_gcis = filterGatewaysCardInfo(ma, bin_list, gws, mauth, None).await;  
//     enabled_gcis.into_iter().filter_map(|gci| gci.gateway).collect()  
// }  

// pub async fn filterFunctionalGatewaysForOTMFlow() -> DeciderFlow<GatewayList> {  
//     let st = getGws().await;  
//     let txn_detail = dpTxnDetail().await;  
//     let macc = dpMerchantAccount().await;  
//     let order_reference = dpOrder().await;  
//     let txn_card_info = dpTxnCardInfo().await;  
//     let m_mgas = getMgas().await;  
//     let payment_flow_list = getPaymentFlowListFromTxnDetail(&txn_detail);  
//     let is_otm_flow = payment_flow_list.contains(&"ONE_TIME_MANDATE".to_string());  
//     let internal_tracking_info = txn_detail.internalTrackingInfo;  
  
//     if is_otm_flow {  
//         let (metadata, pl_ref_id_map) = getOrderMetadataAndPLRefIdMap(macc.enableGatewayReferenceIdBasedRouting, &order_reference).await;  
//         let possible_ref_ids_of_merchant = getAllPossibleRefIds(&metadata, &order_reference, &pl_ref_id_map).await;  
//         let mgas = getEnabledMgasByMerchantIdAndRefId(macc.merchantId, possible_ref_ids_of_merchant).await  
//             .into_iter()  
//             .filter(isOTMEnabled)  
//             .collect::<Vec<_>>();  
  
//         let eligible_mga_post_filtering = mgas.into_iter()  
//             .filter(|mga| st.contains(&mga.gateway))  
//             .collect::<Vec<_>>();  
  
//         let gw_list = eligible_mga_post_filtering.iter().map(|mga| mga.gateway.clone()).collect::<Vec<_>>();  
//         let maybe_jbc = findJuspayBankCode(txn_card_info.paymentMethod).await;  
  
//         match maybe_jbc {  
//             Some(jbc) => {  
//                 let all_gpmf_entries = GPMF::findAllGPMFByCountryCodeGwPfIdPmtJbcid(  
//                     ETCC::IND,  
//                     &gw_list,  
//                     PF::ONE_TIME_MANDATE,  
//                     txn_card_info.paymentMethodType,  
//                     jbc.id,  
//                 ).await;  
  
//                 let mga_ids = eligible_mga_post_filtering.iter().map(|mga| mga.id.merchantGwAccId.clone()).collect::<Vec<_>>();  
//                 let gpmf_ids = all_gpmf_entries.iter().map(|gpmf| gpmf.id.clone()).collect::<Vec<_>>();  
//                 let mgpmf_entries = MGPMF::getAllMgpmfByMgaIdAndGpmfIds(&mga_ids, &gpmf_ids).await;  
  
//                 let mgpmf_mga_id_entries = mgpmf_entries.iter().map(|mgpmf| mgpmf.merchantGatewayAccountId.clone()).collect::<Vec<_>>();  
//                 let eligible_mga_post_filtering_otm = eligible_mga_post_filtering.into_iter()  
//                     .filter(|mga| mgpmf_mga_id_entries.contains(&mga.id.merchantGwAccId))  
//                     .collect::<Vec<_>>();  
  
//                 let gw_list_post_otm_filtering = eligible_mga_post_filtering_otm.iter().map(|mga| mga.gateway.clone()).collect::<Vec<_>>();  
//                 setMgas(eligible_mga_post_filtering_otm).await;  
//                 setGws(gw_list_post_otm_filtering).await;  
//             }  
//             None => setGws(st).await,  
//         }  
//     } else {  
//         setGws(st).await;  
//     }  
  
//     returnGwListWithLog(FilterFunctionalGatewaysForOTM, true).await  
// }  
  
// pub async fn filterGatewaysForValidationType() -> DeciderFlow<GatewayList> {  
//     let st = getGws().await;  
//     let txn_detail = dpTxnDetail().await;  
//     let txn_card_info = dpTxnCardInfo().await;  
//     let macc = dpMerchantAccount().await;  
//     let order_reference = dpOrder().await;  
//     let (metadata, pl_ref_id_map) = getOrderMetadataAndPLRefIdMap(macc.enableGatewayReferenceIdBasedRouting, &order_reference).await;  
//     let possible_ref_ids_of_merchant = getAllPossibleRefIds(&metadata, &order_reference, &pl_ref_id_map).await;  
  
//     if isMandateTransaction(&txn_detail) && isCardTransaction(&txn_card_info) {  
//         let card_mandate_bin_filter_excluded_gateways = findByNameFromRedis(CARD_MANDATE_BIN_FILTER_EXCLUDED_GATEWAYS).await.unwrap_or_default();  
//         let bin_wise_filter_excluded_gateways = intersect(&card_mandate_bin_filter_excluded_gateways, &st);  
//         let bin_list = getBinList(txn_card_info.cardIsin.clone());  
//         let m_new_gateways = filterGatewaysCardInfo(&macc, bin_list, st.clone(), None, Some(CARD_MANDATE)).await  
//             .into_iter()  
//             .filter_map(|gci| gci.gateway)  
//             .collect::<Vec<_>>();  
  
//         logDebugV("filterGatewaysForValidationType", format!(  
//             "Functional gateways after filtering after filterGatewaysCardInfo: {:?}",  
//             m_new_gateways,  
//         ));  
  
//         let new_gateways = ordNub(m_new_gateways.into_iter().chain(bin_wise_filter_excluded_gateways.into_iter()).collect::<Vec<_>>());  
//         let m_internal_meta = getInternalMetaData().await;  
  
//         setGws(new_gateways.clone()).await;  
  
//         if isTokenRepeatTxn(&m_internal_meta) {  
//             let m_token_repeat_mandate_supported_gateways = getTokenSupportedGateways(&txn_detail, &txn_card_info, "MANDATE", &m_internal_meta).await;  
//             let gws = m_token_repeat_mandate_supported_gateways.map_or(new_gateways.clone(), |token_repeat_mandate_supported_gateways| {  
//                 new_gateways.into_iter().filter(|gw| token_repeat_mandate_supported_gateways.contains(gw)).collect()  
//             });  
  
//             let final_gws = if gws.is_empty() { new_gateways.clone() } else { gws };  
//             logDebugV("filterGatewaysForValidationType", format!(  
//                 "Functional gateways after filtering for token repeat Mandate support: {:?}",  
//                 final_gws,  
//             ));  
//             setGws(final_gws).await;  
//         }  
  
//         if !txn_detail.expressCheckout && !isTokenRepeatTxn(&m_internal_meta) {  
//             let m_mandate_guest_checkout_supported_gateways = findByNameFromRedis(getmandateGuestCheckoutKey(txn_card_info.cardSwitchProvider.clone())).await;  
//             let gws = m_mandate_guest_checkout_supported_gateways.map_or(new_gateways.clone(), |mandate_guest_checkout_supported_gateways| {  
//                 new_gateways.into_iter().filter(|gw| mandate_guest_checkout_supported_gateways.contains(gw)).collect()  
//             });  
  
//             let final_gws = if gws.is_empty() { new_gateways.clone() } else { gws };  
//             logDebugV("filterGatewaysForValidationType", format!(  
//                 "Functional gateways after filtering for Mandate Guest Checkout support: {:?}",  
//                 final_gws,  
//             ));  
//             setGws(final_gws).await;  
//         }  
//     } else if isTpvTransaction(&txn_detail) || (isEmandateTransaction(&txn_detail) && isEmandateSupportedPaymentMethod(&txn_card_info)) {  
//         let (validation_type, e_mgas) = if isEmandateTransaction(&txn_detail) {  
//             let e_mgas = getEMandateEnabledMGA(macc.merchantId, possible_ref_ids_of_merchant.clone()).await;  
//             let v_type = if isTpvMandateTransaction(&txn_detail) { TPV_EMANDATE } else { EMANDATE };  
//             (v_type, e_mgas)  
//         } else {  
//             let e_mgas = getEnabledMgasByMerchantIdAndRefId(macc.merchantId, possible_ref_ids_of_merchant.clone()).await;  
//             (TPV, e_mgas)  
//         };  
  
//         let payment_flow_list = getPaymentFlowListFromTxnDetail(&txn_detail);  
//         let is_otm_flow = payment_flow_list.contains(&"ONE_TIME_MANDATE".to_string());  
  
//         if (validation_type == TPV_EMANDATE || validation_type == EMANDATE) && is_otm_flow {  
//             // No specific action for OTM flow in this case  
//         } else {  
//             let enabled_gateway_accounts = e_mgas.into_iter()  
//                 .filter(|mga| predicate(mga, &mga.gateway, &metadata, &order_reference, &pl_ref_id_map).await)  
//                 .collect::<Vec<_>>();  
  
//             let amount = effectiveAmountWithTxnAmount(&txn_detail).await;  
//             let merchant_gateway_card_infos = filterGatewaysForPaymentMethodAndValidationType(  
//                 &macc,  
//                 &txn_card_info,  
//                 enabled_gateway_accounts.clone(),  
//                 validation_type,  
//                 txn_detail.txnId.to_string(),  
//             ).await;  
  
//             let merchant_gateway_card_infos_filtered = filterGatewayCardInfoForMaxRegisterAmount(  
//                 &txn_detail,  
//                 &txn_card_info,  
//                 merchant_gateway_card_infos.clone(),  
//                 amount,  
//             );  
  
//             let gci_ids = merchant_gateway_card_infos_filtered.iter().map(|gci| gci.gatewayCardInfoId.clone()).collect::<Vec<_>>();  
//             let nst = ETGCI::getAllByMgciIds(gci_ids).await  
//                 .into_iter()  
//                 .filter_map(|gci| gci.gateway)  
//                 .collect::<Vec<_>>();  
  
//             logDebugV("filterGatewaysForValidationType", format!(  
//                 "NST for filterGatewaysForValidationType for txn_id {:?}: {:?}",  
//                 txn_detail.txnId,  
//                 nst,  
//             ));  
  
//             let new_st = nst.into_iter().collect::<Vec<_>>();  
//             setGwsAndMgas(  
//                 enabled_gateway_accounts.into_iter()  
//                     .filter(|mga| new_st.contains(&mga.gateway))  
//                     .filter(|mga| merchant_gateway_card_infos_filtered.iter().any(|gci| gci.merchantGatewayAccountId == Some(mga.id)))  
//                     .collect::<Vec<_>>(),  
//             ).await;  
//         }  
//     } else {  
//         let tpv_only_supported_gateways = findByNameFromRedis(TPV_ONLY_SUPPORTED_GATEWAYS).await.unwrap_or_default();  
//         if !tpv_only_supported_gateways.is_empty() && !intersect(&tpv_only_supported_gateways, &st).is_empty() {  
//             let tpv_only_mgas = groupIntoMap(|mga| mga.gateway.clone(), getTpvOnlyGatewayAccounts(possible_ref_ids_of_merchant.clone()).await);  
//             let all_mgas = groupIntoMap(|mga| mga.gateway.clone(), getEnabledMgasByMerchantIdAndRefId(macc.merchantId, possible_ref_ids_of_merchant.clone()).await);  
  
//             let gateways_to_be_removed = tpv_only_mgas.iter()  
//                 .filter(|(gw, v)| all_mgas.get(gw).map_or(false, |all_v| all_v.len() == v.len()))  
//                 .map(|(gw, _)| gw.clone())  
//                 .collect::<Vec<_>>();  
  
//             setGws(st.into_iter().filter(|gw| !gateways_to_be_removed.contains(gw)).collect::<Vec<_>>()).await;  
//         }  
//     }  
  
//     returnGwListWithLog(FilterFunctionalGatewaysForValidationType, true).await  
// }  
  
// async fn predicate(  
//     mga: &MerchantGatewayAccount,  
//     gw: &Gateway,  
//     metadata: &HashMap<String, String>,  
//     order_ref: &Order,  
//     pl_ref_id_map: &HashMap<String, String>,  
// ) -> bool {  
//     let gw_ref_id = getGatewayReferenceId(metadata, gw, order_ref, pl_ref_id_map).await;  
//     mga.referenceId == gw_ref_id  
// }  
  
// pub async fn filterGatewaysCardInfo(  
//     merchant_account: &MerchantAccount,  
//     card_bins: Vec<Option<String>>,  
//     enabled_gateways: GatewayList,  
//     m_auth_type: Option<Secret<AuthType>>,  
//     m_validation_type: Option<ValidationType>,  
// ) -> Vec<GatewayCardInfo> {  
//     if !enabled_gateways.is_empty() && card_bins.iter().all(|bin| bin.is_some()) && (m_auth_type.is_some() || m_validation_type.is_some()) {  
//         if m_validation_type == Some(CARD_MANDATE) {  
//             let merchant_wise_mandate_supported_gateway = getMerchantWiseMandateBinEligibleGateways(merchant_account, enabled_gateways.clone()).await;  
//             let merchant_wise_mandate_supported_gateway_opt = merchant_wise_mandate_supported_gateway.iter().map(Some).collect::<Vec<_>>();  
  
//             let merchant_wise_eligible_gateway_card_info = if !merchant_wise_mandate_supported_gateway.is_empty() {  
//                 getSupportedGatewayCardInfoForBins(  
//                     merchant_account,  
//                     card_bins.clone(),  
//                 ).await.into_iter()  
//                     .filter(|ci| merchant_wise_mandate_supported_gateway_opt.contains(&Some(ci.gateway.clone())) && ci.validationType == Some(CardMandate))  
//                     .collect::<Vec<_>>()  
//             } else {  
//                 Vec::new()  
//             };  
  
//             let eligible_gateway_card_info = getEnabledGatewayCardInfoForGateways(  
//                 card_bins.clone(),  
//                 enabled_gateways.into_iter().filter(|gw| !merchant_wise_mandate_supported_gateway.contains(gw)).collect::<Vec<_>>(),  
//             ).await.into_iter()  
//                 .filter(|ci| ci.validationType == Some(CardMandate))  
//                 .collect::<Vec<_>>();  
  
//             merchant_wise_eligible_gateway_card_info.into_iter().chain(eligible_gateway_card_info.into_iter()).collect()  
//         } else {  
//             let (merchant_validation_required_gws, gci_validation_gws) = partitionM(  
//                 |gw| isMerchantWiseAuthTypeCheckNeeded(merchant_account, m_auth_type.clone(), m_validation_type.clone(), gw),  
//                 enabled_gateways.clone(),  
//             ).await;  
  
//             let gcis = getEnabledGatewayCardInfoForGateways(card_bins.clone(), enabled_gateways.clone()).await;  
  
//             let gcis_without_merchant_validation = gcis.iter()  
//                 .filter(|gci| gci_validation_gws.contains(&gci.gateway.clone()))  
//                 .cloned()  
//                 .collect::<Vec<_>>();  
  
//             let gcis_with_merchant_validation = gcis.iter()  
//                 .filter(|gci| merchant_validation_required_gws.contains(&gci.gateway.clone()))  
//                 .cloned()  
//                 .collect::<Vec<_>>();  
  
//             let gci_ids = gcis_with_merchant_validation.iter().map(|gci| gci.id.clone()).collect::<Vec<_>>();  
//             let mgcis_enabled_gcis = if gci_ids.is_empty() {  
//                 Vec::new()  
//             } else {  
//                 findAllMgcisByMaccAndGciPId(merchant_account.id.clone(), gci_ids).await.into_iter()  
//                     .filter(|mgci| !mgci.disabled)  
//                     .collect::<Vec<_>>()  
//             };  
  
//             let mgcis_enabled_gci_ids = mgcis_enabled_gcis.iter().map(|mgci| mgci.gatewayCardInfoId.clone()).collect::<Vec<_>>();  
//             let gcis_after_merchant_validation = gcis_with_merchant_validation.into_iter()  
//                 .filter(|gci| mgcis_enabled_gci_ids.contains(&gci.id))  
//                 .collect::<Vec<_>>();  
  
//             let eligible_gateway_card_infos = gcis_without_merchant_validation.into_iter().chain(gcis_after_merchant_validation.into_iter()).collect::<Vec<_>>();  
  
//             match m_validation_type {  
//                 Some(v_type) => eligible_gateway_card_infos.into_iter()  
//                     .filter(|ci| ci.validationType.map_or(false, |vt| vt.to_string() == v_type.to_string()))  
//                     .collect(),  
//                 None => eligible_gateway_card_infos.into_iter()  
//                     .filter(|ci| ci.authType.map_or(false, |at| at.to_string() == m_auth_type.map_or(String::new(), |auth| auth.to_string())))  
//                     .collect(),  
//             }  
//         }  
//     } else {  
//         Vec::new()  
//     }  
// }

// pub async fn filterGatewaysForTxnOfferDetails() -> DeciderFlow<GatewayList> {  
//     let functional_gateways = getGws().await?;  
//     let txn_offer_details = asks(|ctx| ctx.dpTxnOfferDetails).await;  
//     let txn_detail = asks(|ctx| ctx.dpTxnDetail).await;  
  
//     match txn_offer_details {  
//         Some(txn_offer_details) => {  
//             let filtered_gws = txn_offer_details.iter().fold(functional_gateways.clone(), |gw_list_acc, txn_offer_detail| {  
//                 filterByGatewayRule(&txn_detail, gw_list_acc, txn_offer_detail)  
//             }).await;  
  
//             if functional_gateways.len() != filtered_gws.len() {  
//                 setGws(filtered_gws.clone()).await?;  
//             }  
//             returnGwListWithLog("FilterFunctionalGatewaysForTxnOfferDetails", true).await  
//         }  
//         None => returnGwListWithLog("FilterFunctionalGatewaysForTxnOfferDetails", true).await,  
//     }  
// }  
  
// async fn filterByGatewayRule(txn_detail: &TxnDetail, gw_list_acc: GatewayList, txn_offer_detail: &ETOD::TxnOfferDetail) -> GatewayList {  
//     match &txn_offer_detail.gatewayInfo {  
//         None => gw_list_acc,  
//         Some(txt) => {  
//             match serde_json::from_str::<GatewayRule>(&txt) {  
//                 Ok(gateway_rule) if gateway_rule.force_routing.unwrap_or(false) => {  
//                     let txn_offer_details_gws = GDR::convertTextToGateway(  
//                         gateway_rule.gateway_info.iter().map(|info| info.name.clone()).collect()  
//                     );  
//                     gw_list_acc.intersection(&txn_offer_details_gws).cloned().collect()  
//                 }  
//                 Err(err) => {  
//                     logDebugT(  
//                         format!("For txn with id = {}", txn_detail.txnId),  
//                         format!("offerId = {}, parsing result is {:?}", txn_offer_detail.offerId, err)  
//                     ).await;  
//                     gw_list_acc  
//                 }  
//             }  
//         }  
//     }  
// }  
  
// pub async fn filterGatewaysForEmi() -> DeciderFlow<GatewayList> {  
//     let functional_gateways = getGws().await?;  
//     let merchant_acc = asks(|ctx| ctx.dpMerchantAccount).await;  
//     let txn_card_info = asks(|ctx| ctx.dpTxnCardInfo).await;  
//     let txn_detail = asks(|ctx| ctx.dpTxnDetail).await;  
  
//     logDebugT(  
//         "GW_Filtering",  
//         format!(  
//             "For txn with id = {} isEmi = {}",  
//             txn_detail.txnId,  
//             txn_detail.isEmi  
//         )  
//     ).await;  
  
//     if txn_detail.isEmi {  
//         let si_on_emi_card_supported_gateways: HashSet<_> = RService::findByNameFromRedis(C::SI_ON_EMI_CARD_SUPPORTED_GATEWAYS)  
//             .await  
//             .unwrap_or_default()  
//             .into_iter()  
//             .collect();  
  
//         let st = if Utils::isMandateTransaction(&txn_detail) {  
//             functional_gateways  
//                 .into_iter()  
//                 .filter(|gw| si_on_emi_card_supported_gateways.contains(gw))  
//                 .collect()  
//         } else {  
//             functional_gateways  
//         };  
  
//         let gws = if Utils::checkNoOrLowCostEmi(&txn_card_info) {  
//             let no_or_low_cost_emi_supported_gateways: HashSet<_> = RService::findByNameFromRedis(C::NO_OR_LOW_COST_EMI_SUPPORTED_GATEWAYS)  
//                 .await  
//                 .unwrap_or_default()  
//                 .into_iter()  
//                 .collect();  
//             st.into_iter()  
//                 .filter(|gw| no_or_low_cost_emi_supported_gateways.contains(gw))  
//                 .collect()  
//         } else {  
//             st  
//         };  
  
//         let juspay_bank_code = Utils::getJuspayBankCodeFromInternalMetadata(&txn_detail);  
//         let gws = if Utils::isCardTransaction(&txn_card_info) && !gws.is_empty() {  
//             if Utils::checkIfBinIsEligibleForEmi(  
//                 &txn_card_info.cardIsin,  
//                 &juspay_bank_code,  
//                 txn_card_info.cardType.as_ref().map(ETCa::cardTypeToText)  
//             ).await {  
//                 gws  
//             } else {  
//                 vec![]  
//             }  
//         } else {  
//             gws  
//         };  
  
//         let m_internal_meta = Utils::getInternalMetaData().await;  
//         let scope = if Utils::isCardTransaction(&txn_card_info) && Utils::isNetworkTokenRepeatTxn(&m_internal_meta) {  
//             "NETWORK_TOKEN"  
//         } else if Utils::isCardTransaction(&txn_card_info) && Utils::isIssuerTokenRepeatTxn(&m_internal_meta) {  
//             "ISSUER_TOKEN"  
//         } else if Utils::isCardTransaction(&txn_card_info) && Utils::isAltIdBasedTxn(&m_internal_meta) {  
//             "ALT_ID"  
//         } else if txn_detail.emiBank.as_deref().map_or(false, |bank| bank.ends_with("_CLEMI")) {  
//             "CARDLESS"  
//         } else {  
//             "CARD"  
//         };  
  
//         let gws = if !gws.is_empty() {  
//             logDebugT(  
//                 format!(  
//                     "filterGatewaysForEmi gateway list before getGatewayBankEmiSupport for txn_id :{}",  
//                     txn_detail.txnId  
//                 ),  
//                 format!("where gateway is : {:?}", gws)  
//             ).await;  
  
//             let gbes_v2_flag = Redis::isRedisFeatureEnabled(C::GBES_V2_ENABLED, merchant_acc.merchantId.to_string()).await;  
//             if gbes_v2_flag {  
//                 let gbes_v2s = S::getGatewayBankEmiSupportV2(  
//                     &txn_detail.emiBank,  
//                     &gws,  
//                     scope,  
//                     txn_detail.emiTenure.map(|tenure| tenure as i32)  
//                 ).await;  
//                 if gbes_v2s.is_empty() {  
//                     logInfoV(  
//                         "GBESV2 Entry Not Found",  
//                         format!(  
//                             "GBESV2 Entry Not Found For emiBank - {:?}, gateways - {:?}, scope - {}, tenure - {:?}",  
//                             txn_detail.emiBank, gws, scope, txn_detail.emiTenure  
//                         )  
//                     ).await;  
//                 }  
//                 extractGatewaysV2(gbes_v2s)  
//             } else {  
//                 extractGateways(S::getGatewayBankEmiSupport(  
//                     &txn_detail.emiBank,  
//                     &gws,  
//                     scope  
//                 ).await)  
//             }  
//         } else {  
//             gws  
//         };  
  
//         setGws(gws).await?;  
//     } else if Utils::isCardTransaction(&txn_card_info) {  
//         let card_emi_explicit_gateways: HashSet<_> = RService::findByNameFromRedis(C::CARD_EMI_EXPLICIT_GATEWAYS)  
//             .await  
//             .unwrap_or_default()  
//             .into_iter()  
//             .collect();  
//         setGws(  
//             functional_gateways  
//                 .into_iter()  
//                 .filter(|gw| !card_emi_explicit_gateways.contains(gw))  
//                 .collect()  
//         ).await?;  
//     }  
  
//     returnGwListWithLog("FilterFunctionalGatewaysForEmi", true).await  
// }  
  
// fn extractGateways(gbes: Vec<ETGBES::GatewayBankEmiSupport>) -> GatewayList {  
//     gbes.into_iter().map(|gb| gb.gateway).collect()  
// }  
  
// fn extractGatewaysV2(gbes_v2: Vec<ETGBESV2::GatewayBankEmiSupportV2>) -> GatewayList {  
//     gbes_v2.into_iter().map(|gb| gb.gateway).collect()  
// }  

// pub async fn filterGatewaysForPaymentMethod() -> DeciderFlow<GatewayList> {  
//     let st = getGws().await?;  
//     let txn = asks(|ctx| ctx.dpTxnDetail).await;  
//     let merchant_acc = asks(|ctx| ctx.dpMerchantAccount).await;  
//     let txn_card_info = asks(|ctx| ctx.dpTxnCardInfo).await;  
//     let oref = asks(|ctx| ctx.dpOrder).await;  
  
//     let is_dynamic_mga_enabled = Utils::getIsMerchantEnabedForDynamicMGASelection().await;  
//     let (metadata, pl_ref_id_map) = Utils::getOrderMetadataAndPLRefIdMap(  
//         merchant_acc.enableGatewayReferenceIdBasedRouting,  
//         &oref,  
//     ).await;  
  
//     let proceed_with_all_mgas = Utils::isEnabledForAllMgas().await;  
//     let mgas = if proceed_with_all_mgas {  
//         S::getAllEnabledMgasByMerchantId(merchant_acc.merchantId).await  
//     } else {  
//         let possible_ref_ids_of_merchant = Utils::getAllPossibleRefIds(&metadata, &oref, &pl_ref_id_map).await;  
//         S::getEnabledMgasByMerchantIdAndRefId(merchant_acc.merchantId, possible_ref_ids_of_merchant).await  
//     };  
  
//     let eligible_mgas: Vec<_> = mgas.into_iter().filter(|mga| st.contains(&mga.gateway)).collect();  
  
//     if st.is_empty() || Utils::isEmandateTransaction(&txn) || Utils::isTpvTransaction(&txn) {  
//         logDebugV(  
//             "filterGatewaysForPaymentMethod",  
//             format!("For txn: {}, Skipped", txn.txnId),  
//         ).await;  
//     } else {  
//         if Utils::isCardTransaction(&txn_card_info) {  
//             let m_payment_method = Utils::getCardBrand().await;  
//             let maybe_payment_method = if txn_card_info.paymentMethod.is_empty() {  
//                 m_payment_method  
//             } else {  
//                 Some(txn_card_info.paymentMethod.clone())  
//             };  
  
//             if let Some(payment_method) = maybe_payment_method {  
//                 let (rem, rem_mgas) = getGatewaysAcceptingPaymentMethod(  
//                     &oref,  
//                     &merchant_acc,  
//                     &eligible_mgas,  
//                     &st,  
//                     &payment_method,  
//                     proceed_with_all_mgas,  
//                     is_dynamic_mga_enabled,  
//                 ).await;  
  
//                 logDebugV(  
//                     "filterGatewaysForPaymentMethod",  
//                     format!(  
//                         "For txn: {}, Remaining gateways after getGatewaysAcceptingPaymentMethod: {:?}",  
//                         txn.txnId, rem  
//                     ),  
//                 ).await;  
  
//                 setGwsAndMgas(rem_mgas).await?;  
//             }  
//         } else {  
//             logDebugV(  
//                 "filterGatewaysForPaymentMethod",  
//                 format!("For txn: {}, Not card transaction", txn.txnId),  
//             ).await;  
  
//             let pm = getPaymentMethodForNonCardTransaction(&txn_card_info);  
//             let v2_integration_not_supported_gateways: HashSet<_> = RService::findByNameFromRedis(C::V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS)  
//                 .await  
//                 .unwrap_or_default()  
//                 .into_iter()  
//                 .collect();  
  
//             let upi_intent_not_supported_gateways: HashSet<_> = RService::findByNameFromRedis(C::UPI_INTENT_NOT_SUPPORTED_GATEWAYS)  
//                 .await  
//                 .unwrap_or_default()  
//                 .into_iter()  
//                 .collect();  
  
//             let (st, filtered_mgas) = if pm == "UPI_PAY" || pm == "UPI_QR" {  
//                 if !st.is_empty() && !v2_integration_not_supported_gateways.is_disjoint(&st) {  
//                     filterGatewaysForUpiPayBasedOnSupportedFlow(  
//                         &st,  
//                         &eligible_mgas,  
//                         &v2_integration_not_supported_gateways,  
//                         &upi_intent_not_supported_gateways,  
//                     ).await  
//                 } else {  
//                     (st, eligible_mgas)  
//                 }  
//             } else {  
//                 (st, eligible_mgas)  
//             };  
  
//             let (_, rem_mgas) = getGatewaysAcceptingPaymentMethod(  
//                 &oref,  
//                 &merchant_acc,  
//                 &filtered_mgas,  
//                 &st,  
//                 &pm,  
//                 proceed_with_all_mgas,  
//                 is_dynamic_mga_enabled,  
//             ).await;  
  
//             setGwsAndMgas(rem_mgas).await?;  
//         }  
//     }  
  
//     returnGwListWithLog("FilterFunctionalGatewaysForPaymentMethod", true).await  
// }  
  
// async fn getGatewaysAcceptingPaymentMethod(  
//     oref: &Order,  
//     merchant_acc: &MerchantAccount,  
//     eligible_mgas: &[MerchantGatewayAccount],  
//     gateways: &GatewayList,  
//     payment_method: &str,  
//     proceed_with_all_mgas: bool,  
//     is_dynamic_mga_enabled: bool,  
// ) -> (GatewayList, Vec<MerchantGatewayAccount>) {  
//     let filtered_mgas: Vec<_> = eligible_mgas  
//         .iter()  
//         .filter(|mga| canAcceptPaymentMethod(mga, payment_method) && gateways.contains(&mga.gateway))  
//         .cloned()  
//         .collect();  
  
//     let gateways: GatewayList = filtered_mgas.iter().map(|mga| mga.gateway.clone()).collect();  
  
//     (gateways, filtered_mgas)  
// }  
  
// fn getPaymentMethodForNonCardTransaction(txn_card_info: &TxnCardInfo) -> String {  
//     if matches!(  
//         txn_card_info.paymentMethodType,  
//         Some(ETP::ConsumerFinance | ETP::UPI | ETP::Reward | ETP::Cash)  
//     ) {  
//         txn_card_info.paymentMethod.clone()  
//     } else {  
//         txn_card_info.cardIssuerBankName.clone().unwrap_or_default()  
//     }  
// }  
  
// fn canAcceptPaymentMethod(mga: &MerchantGatewayAccount, pm: &str) -> bool {  
//     if let Some(payment_methods) = Utils::getValue("paymentMethods", &mga.paymentMethods) {  
//         payment_methods.contains(&pm.to_string())  
//     } else {  
//         false  
//     }  
// } 

// pub fn filterGatewaysForTokenProvider() -> DeciderFlow<GatewayList> {  
//     let st = getGws();  
//     let vault = asks(|ctx| ctx.dpVaultProvider.clone());  
//     let txn_id = asks(|ctx| ctx.dpTxnDetail.txnId.clone());  
//     log_debug_t(  
//         "filterGatewaysForTokenProvider",  
//         format!(  
//             "Vault provider for txn {} = {:?}",  
//             review_transaction_id_text(txn_id),  
//             vault  
//         ),  
//     );  
  
//     match vault {  
//         None => return_gw_list_with_log(FilterFunctionalGatewaysForTokenProvider, false),  
//         Some(ETCa::Juspay) => return_gw_list_with_log(FilterFunctionalGatewaysForTokenProvider, true),  
//         Some(v) => {  
//             let token_provider_gateway_mapping = RService::find_by_name_from_redis(C::TOKEN_PROVIDER_GATEWAY_MAPPING)  
//                 .unwrap_or_default();  
//             let new_st = st  
//                 .into_iter()  
//                 .filter(|gateway| {  
//                     token_provider_gateway_mapping.iter().any(|mapping| {  
//                         mapping.0 == v && mapping.1 == *gateway  
//                     })  
//                 })  
//                 .collect::<Vec<_>>();  
//             set_gws(new_st);  
//             return_gw_list_with_log(FilterFunctionalGatewaysForTokenProvider, true);  
//         }  
//     }  
// }  
  
// pub fn filterGatewaysForWallet() -> DeciderFlow<GatewayList> {  
//     let st = getGws();  
//     let txn_card_info = asks(|ctx| ctx.dpTxnCardInfo.clone());  
//     let upi_only_gateways = RService::find_by_name_from_redis(C::UPI_ONLY_GATEWAYS)  
//         .unwrap_or_default()  
//         .into_iter()  
//         .collect::<HashSet<_>>();  
//     let wallet_only_gateways = RService::find_by_name_from_redis(C::WALLET_ONLY_GATEWAYS)  
//         .unwrap_or_default()  
//         .into_iter()  
//         .collect::<HashSet<_>>();  
//     let wallet_also_gateways = RService::find_by_name_from_redis(C::WALLET_ALSO_GATEWAYS)  
//         .unwrap_or_default()  
//         .into_iter()  
//         .collect::<HashSet<_>>();  
  
//     let new_st = match txn_card_info.cardType {  
//         Some(ETCa::Wallet) => st  
//             .into_iter()  
//             .filter(|gateway| {  
//                 wallet_only_gateways.contains(gateway)  
//                     || wallet_also_gateways.contains(gateway)  
//                     || (S::is_google_pay_txn(&txn_card_info) && upi_only_gateways.contains(gateway))  
//             })  
//             .collect::<Vec<_>>(),  
//         _ => st  
//             .into_iter()  
//             .filter(|gateway| !wallet_only_gateways.contains(gateway))  
//             .collect::<Vec<_>>(),  
//     };  
  
//     set_gws(new_st);  
//     return_gw_list_with_log(FilterFunctionalGatewaysForWallet, true);  
// }  
  
// pub fn filterGatewaysForNbOnly() -> DeciderFlow<GatewayList> {  
//     let st = getGws();  
//     let txn_card_info = asks(|ctx| ctx.dpTxnCardInfo.clone());  
  
//     if txn_card_info.cardType != Some(ETCa::NB) {  
//         let nb_only_gateways = RService::find_by_name_from_redis(C::NB_ONLY_GATEWAYS)  
//             .unwrap_or_default()  
//             .into_iter()  
//             .collect::<HashSet<_>>();  
//         let new_st = st  
//             .into_iter()  
//             .filter(|gateway| !nb_only_gateways.contains(gateway))  
//             .collect::<Vec<_>>();  
//         set_gws(new_st);  
//     }  
  
//     return_gw_list_with_log(FilterFunctionalGatewaysForNbOnly, true);  
// }  
  
// pub fn filterFunctionalGatewaysForMerchantRequiredFlow() -> DeciderFlow<GatewayList> {  
//     let st = getGws();  
//     let txn_detail = asks(|ctx| ctx.dpTxnDetail.clone());  
//     let payment_flow_list = Utils::get_payment_flow_list_from_txn_detail(&txn_detail);  
  
//     let is_mf_order = payment_flow_list.contains(&"MUTUAL_FUND".to_string());  
//     let is_cb_order = payment_flow_list.contains(&"CROSS_BORDER_PAYMENT".to_string());  
//     let is_sbmd = payment_flow_list.contains(&"SINGLE_BLOCK_MULTIPLE_DEBIT".to_string());  
  
//     let mf_filtered_gw = filter_gateways_for_flow(is_mf_order, C::MUTUAL_FUND_FLOW_SUPPORTED_GATEWAYS, st);  
//     let mf_and_cb_filtered_gw =  
//         filter_gateways_for_flow(is_cb_order, C::CROSS_BORDER_FLOW_SUPPORTED_GATEWAYS, mf_filtered_gw);  
//     let filtered_gw = filter_gateways_for_flow(is_sbmd, C::SBMD_SUPPORTED_GATEWAYS, mf_and_cb_filtered_gw);  
  
//     set_gws(filtered_gw);  
//     return_gw_list_with_log(FilterFunctionalGatewaysForMerchantRequiredFlow, true);  
  
//     fn filter_gateways_for_flow(  
//         condition: bool,  
//         redis_key: &str,  
//         gateways: Vec<Gateway>,  
//     ) -> Vec<Gateway> {  
//         if condition {  
//             let supported_gateways = RService::find_by_name_from_redis(redis_key)  
//                 .unwrap_or_default()  
//                 .into_iter()  
//                 .collect::<HashSet<_>>();  
//             gateways  
//                 .into_iter()  
//                 .filter(|gateway| supported_gateways.contains(gateway))  
//                 .collect::<Vec<_>>()  
//         } else {  
//             gateways  
//         }  
//     }  
// }  
  
// pub fn filterGatewaysForMGASelectionIntegrity() -> DeciderFlow<GatewayList> {  
//     let is_dynamic_mga_enabled = Utils::get_is_merchant_enabled_for_dynamic_mga_selection();  
  
//     if !is_dynamic_mga_enabled {  
//         return return_gw_list_with_log(FilterGatewaysForMGASelectionIntegrity, true);  
//     }  
  
//     filter_for_emi_tenure_specific_mgas();  
//     let mgas = Utils::get_mgas().unwrap_or_default();  
//     let st = getGws();  
//     let txn_detail = asks(|ctx| ctx.dpTxnDetail.clone());  
  
//     let filtered_mgas = mgas  
//         .into_iter()  
//         .filter(|mga| st.contains(&mga.gateway))  
//         .collect::<Vec<_>>();  
  
//     let gwts = validate_only_one_mga(filtered_mgas, txn_detail, st);  
//     set_gws(gwts);  
//     return_gw_list_with_log(FilterGatewaysForMGASelectionIntegrity, true);  
  
//     fn validate_only_one_mga(  
//         mgas: Vec<MGA>,  
//         txn_detail: TransactionDetail,  
//         st: Vec<Gateway>,  
//     ) -> Vec<GatewayTransaction> {  
//         st.into_iter()  
//             .filter_map(|gwt| {  
//                 let count = mgas.iter().filter(|mga| mga.gateway == gwt).count();  
//                 if count == 1 {  
//                     Some(gwt)  
//                 } else {  
//                     log_error_v(  
//                         "INVALID_MGA_CONFIGURATION",  
//                         format!(  
//                             "txn_id: {}, gwt: {}",  
//                             txn_detail.txnId,  
//                             gwt  
//                         ),  
//                     );  
//                     None  
//                 }  
//             })  
//             .collect::<Vec<_>>()  
//     }  
// } 

// pub fn filter_for_emi_tenure_specific_mgas() -> DeciderFlow<GatewayList> {  
//     let txn_detail = asks(|ctx| ctx.dp_txn_detail);  
//     if txn_detail.is_emi {  
//         let st = get_gws();  
//         let mgas = utils::get_mgas().unwrap_or_default();  
//         let filtered_mgas = mgas.into_iter().filter(|gw_account| {  
//             if st.contains(&gw_account.gateway) {  
//                 if c::gateways_with_tenure_based_creds.contains(&gw_account.gateway.to_string()) {  
//                     let acc_details = unsafe_extract_secret(&gw_account.account_details);  
//                     match serde_json::from_str::<EMIAccountDetails>(&acc_details) {  
//                         Ok(emi_details) => {  
//                             get_emi(emi_details.is_emi) == txn_detail.is_emi  
//                                 && get_tenure(emi_details.emi_tenure)  
//                                     == txn_detail.emi_tenure.unwrap_or(0) as i32  
//                         }  
//                         _ => true,  
//                     }  
//                 } else {  
//                     true  
//                 }  
//             } else {  
//                 false  
//             }  
//         }).collect();  
//         set_gws_and_mgas(filtered_mgas);  
//     }  
//     return_gw_list_with_log(FilterGatewaysForEMITenureSpecficGatewayCreds, true)  
// }  
  
// fn get_emi(is_emi: Option<AValue>) -> bool {  
//     match is_emi {  
//         Some(AValue::Bool(true)) => true,  
//         Some(AValue::String(string_bool)) => string_bool.to_lowercase() == "true",  
//         _ => false,  
//     }  
// }  
  
// fn get_tenure(tenure: Option<i32>) -> i32 {  
//     tenure.unwrap_or(0)  
// }  
  
// pub fn filter_gateways_for_consumer_finance() -> DeciderFlow<GatewayList> {  
//     let st = get_gws();  
//     let txn_card_info = asks(|ctx| ctx.dp_txn_card_info);  
//     let consumer_finance_only_gateways: HashSet<_> = find_by_name_from_redis(CONSUMER_FINANCE_ONLY_GATEWAYS)  
//         .unwrap_or_default()  
//         .into_iter()  
//         .collect();  
//     if txn_card_info.payment_method_type == ConsumerFinance {  
//         let consumer_finance_also_gateways: HashSet<_> = find_by_name_from_redis(CONSUMER_FINANCE_ALSO_GATEWAYS)  
//             .unwrap_or_default()  
//             .into_iter()  
//             .collect();  
//         let consumer_finance_support_gateways = consumer_finance_only_gateways  
//             .union(&consumer_finance_also_gateways)  
//             .cloned()  
//             .collect();  
//         set_gws(st.into_iter().filter(|gw| consumer_finance_support_gateways.contains(gw)).collect());  
//     } else {  
//         set_gws(st.into_iter().filter(|gw| !consumer_finance_only_gateways.contains(gw)).collect());  
//     }  
//     return_gw_list_with_log(FilterFunctionalGatewaysForConsumerFinance, true)  
// }  
  
// pub fn filter_gateways_for_upi() -> DeciderFlow<GatewayList> {  
//     let st = get_gws();  
//     let txn_card_info = asks(|ctx| ctx.dp_txn_card_info);  
//     let txn_detail = asks(|ctx| ctx.dp_txn_detail);  
//     let upi_only_gateways: HashSet<_> = find_by_name_from_redis(UPI_ONLY_GATEWAYS)  
//         .unwrap_or_default()  
//         .into_iter()  
//         .collect();  
//     if txn_card_info.payment_method_type == UPI {  
//         let upi_also_gateways: HashSet<_> = find_by_name_from_redis(UPI_ALSO_GATEWAYS)  
//             .unwrap_or_default()  
//             .into_iter()  
//             .collect();  
//         let upi_support_gateways = upi_only_gateways  
//             .union(&upi_also_gateways)  
//             .cloned()  
//             .collect();  
//         set_gws(st.into_iter().filter(|gw| upi_support_gateways.contains(gw)).collect());  
//     } else if !s::is_google_pay_txn(&txn_card_info) {  
//         set_gws(st.into_iter().filter(|gw| !upi_only_gateways.contains(gw)).collect());  
//     }  
//     return_gw_list_with_log(FilterFunctionalGatewaysForUpi, true)  
// }  
  
// pub fn filter_gateways_for_txn_type() -> DeciderFlow<GatewayList> {  
//     let mut st = get_gws();  
//     let m_txn_type = asks(|ctx| ctx.dp_txn_type);  
//     let txn_card_info = asks(|ctx| ctx.dp_txn_card_info);  
//     if let Some(txn_type) = get_true_string(m_txn_type) {  
//         let mgas = utils::get_mgas().unwrap_or_default();  
//         let (st, curr_mgas) = if txn_card_info.payment_method_type == UPI  
//             && txn_card_info.payment_method == "UPI"  
//         {  
//             let functional_mgas: Vec<_> = mgas  
//                 .into_iter()  
//                 .filter(|mga| {  
//                     st.contains(&mga.gateway)  
//                         && is_txn_type_enabled(&mga.supported_txn_type, "UPI", &txn_type)  
//                 })  
//                 .collect();  
//             (functional_mgas.iter().map(|mga| mga.gateway).collect(), functional_mgas)  
//         } else {  
//             (st, mgas)  
//         };  
//         let v2_integration_not_supported_gateways: Vec<_> =  
//             find_by_name_from_redis(V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS).unwrap_or_default();  
//         let upi_intent_not_supported_gateways: Vec<_> =  
//             find_by_name_from_redis(UPI_INTENT_NOT_SUPPORTED_GATEWAYS).unwrap_or_default();  
//         let (_, filtered_mgas) = if ["UPI_PAY", "UPI_QR"].contains(&txn_type.as_str())  
//             && !intersect(&st, &(v2_integration_not_supported_gateways  
//                 .iter()  
//                 .chain(&upi_intent_not_supported_gateways)  
//                 .cloned()  
//                 .collect()))  
//                 .is_empty()  
//         {  
//             filter_gateways_for_upi_pay_based_on_supported_flow(  
//                 st,  
//                 curr_mgas,  
//                 v2_integration_not_supported_gateways,  
//                 upi_intent_not_supported_gateways,  
//             )  
//         } else {  
//             (st, curr_mgas)  
//         };  
//         let txn_type_gateway_mapping: Vec<_> =  
//             find_by_name_from_redis(TXN_TYPE_GATEWAY_MAPPING).unwrap_or_default();  
//         set_gws_and_mgas(  
//             filtered_mgas  
//                 .into_iter()  
//                 .filter(|mga| {  
//                     get_txn_type_supported_gateways(&txn_type, &txn_type_gateway_mapping)  
//                         .contains(&mga.gateway)  
//                 })  
//                 .collect(),  
//         );  
//     }  
//     return_gw_list_with_log(FilterFunctionalGatewaysForTxnType, true)  
// }  
  
// fn get_txn_type_supported_gateways(  
//     txn_type: &str,  
//     txn_type_gateway_mapping: &[(String, Vec<String>)],  
// ) -> Vec<String> {  
//     txn_type_gateway_mapping  
//         .iter()  
//         .find(|(key, _)| key == txn_type)  
//         .map(|(_, gateways)| gateways.clone())  
//         .unwrap_or_default()  
// }  
  
// pub fn filter_gateways_for_upi_pay_based_on_supported_flow(  
//     gws: GatewayList,  
//     mgas: Vec<MerchantGatewayAccount>,  
//     v2_integration_not_supported_gateways: GatewayList,  
//     upi_intent_not_supported_gateways: GatewayList,  
// ) -> DeciderFlow<(GatewayList, Vec<MerchantGatewayAccount>)> {  
//     let upd_mgas: Vec<_> = mgas  
//         .into_iter()  
//         .filter(|mga| {  
//             if !v2_integration_not_supported_gateways.contains(&mga.gateway) {  
//                 true  
//             } else {  
//                 let value = is_payment_flow_enabled_in_mga(mga, "V2_INTEGRATION")  
//                     .map(|enabled| if enabled { "true" } else { "0" })  
//                     .or_else(|| {  
//                         get_value("shouldUseV2Integration", unsafe_extract_secret(&mga.account_details))  
//                     });  
//                 value == Some("true".to_string()) || value == Some("1".to_string())  
//             }  
//         })  
//         .collect();  
//     let upd_mgas: Vec<_> = upd_mgas  
//         .into_iter()  
//         .filter(|mga| {  
//             if !upi_intent_not_supported_gateways.contains(&mga.gateway) {  
//                 true  
//             } else {  
//                 let value = get_value("isUpiIntentEnabled", unsafe_extract_secret(&mga.account_details));  
//                 value == Some("true".to_string())  
//             }  
//         })  
//         .collect();  
//     let gateways = upd_mgas.iter().map(|mga| mga.gateway.clone()).collect();  
//     (gateways, upd_mgas)  
// }  

// pub fn filter_gateways_for_txn_detail_type() -> DeciderFlow<GatewayList> {  
//     let st = get_gws();  
//     let m_txn_type = asks(|ctx| ctx.dp_txn_detail.txn_type.clone());  
//     let txn_type = m_txn_type.map_or(String::new(), |t| t.to_text());  
//     let txn_detail_type_restricted_gateways = r_service::find_by_name_from_redis(C::TXN_DETAIL_TYPE_RESTRICTED_GATEWAYS)  
//         .unwrap_or_default();  
//     let filter_gws = if txn_type == "ZERO_AUTH" {  
//         st.iter()  
//             .filter(|gw| get_zero_auth_supported_gateways(&txn_type, &txn_detail_type_restricted_gateways).contains(gw))  
//             .cloned()  
//             .collect()  
//     } else {  
//         st  
//     };  
//     set_gws(filter_gws);  
//     return_gw_list_with_log(FilterFunctionalGatewaysForTxnDetailType, true);  
  
//     fn get_zero_auth_supported_gateways(txn_type: &str, txn_detail_type_restricted_gateways: &[(&str, Vec<Gateway>)]) -> Vec<Gateway> {  
//         txn_detail_type_restricted_gateways  
//             .iter()  
//             .find(|mapping| mapping.0 == txn_type)  
//             .map_or_else(Vec::new, |mapping| mapping.1.clone())  
//     }  
// }  
  
// pub fn filter_gateways_for_reward() -> DeciderFlow<GatewayList> {  
//     let st = get_gws();  
//     let payment_method_type = asks(|ctx| ctx.dp_txn_card_info.payment_method_type.clone());  
//     let card_type = asks(|ctx| ctx.dp_txn_card_info.card_type.clone());  
//     let reward_also_gateways: HashSet<Gateway> = r_service::find_by_name_from_redis(C::REWARD_ALSO_GATEWAYS)  
//         .unwrap_or_default()  
//         .into_iter()  
//         .collect();  
//     let reward_only_gateways: HashSet<Gateway> = r_service::find_by_name_from_redis(C::REWARD_ONLY_GATEWAYS)  
//         .unwrap_or_default()  
//         .into_iter()  
//         .collect();  
//     let filtered_gws = if card_type == Some(ETCa::Reward) || payment_method_type == ETP::Reward {  
//         st.into_iter()  
//             .filter(|gw| reward_also_gateways.contains(gw) || reward_only_gateways.contains(gw))  
//             .collect()  
//     } else {  
//         st.into_iter()  
//             .filter(|gw| !reward_only_gateways.contains(gw))  
//             .collect()  
//     };  
//     set_gws(filtered_gws);  
//     return_gw_list_with_log(FilterFunctionalGatewaysForReward, true);  
// }  
  
// pub fn filter_gateways_for_cash() -> DeciderFlow<GatewayList> {  
//     let st = get_gws();  
//     let payment_method_type = asks(|ctx| ctx.dp_txn_card_info.payment_method_type.clone());  
//     if payment_method_type != ETP::Cash {  
//         let cash_only_gateways: HashSet<Gateway> = r_service::find_by_name_from_redis(C::CASH_ONLY_GATEWAYS)  
//             .unwrap_or_default()  
//             .into_iter()  
//             .collect();  
//         let filtered_gws = st.into_iter().filter(|gw| !cash_only_gateways.contains(gw)).collect();  
//         set_gws(filtered_gws);  
//     }  
//     return_gw_list_with_log(FilterFunctionalGatewaysForCash, true);  
// }  
  
// pub fn filter_functional_gateways_for_split_settlement() -> DeciderFlow<GatewayList> {  
//     let oref = asks(|ctx| ctx.dp_order.clone());  
//     let txn_id = asks(|ctx| ctx.dp_txn_detail.txn_id.clone());  
//     let e_split_settlement_details = utils::get_split_settlement_details();  
//     let macc = asks(|ctx| ctx.dp_merchant_account.clone());  
//     log_debug_v(  
//         "enableGatewayReferenceIdBasedRouting in splitsettlement",  
//         format!(  
//             "enableGatewayReferenceIdBasedRouting: for txnId {} is {}",  
//             txn_id, macc.enable_gateway_reference_id_based_routing  
//         ),  
//     );  
//     let (metadata, pl_ref_id_map) =  
//         utils::get_order_metadata_and_pl_ref_id_map(macc.enable_gateway_reference_id_based_routing, &oref);  
//     let possible_ref_ids_of_merchant = utils::get_all_possible_ref_ids(&metadata, &oref, &pl_ref_id_map);  
  
//     match e_split_settlement_details {  
//         Ok(split_settlement_details) => {  
//             let given_sub_mids: Vec<_> = split_settlement_details.vendor.split.iter().map(|v| v.sub_mid.clone()).collect();  
//             let given_sub_mids_size = given_sub_mids.len();  
//             if given_sub_mids.is_empty() {  
//                 log_debug_t("SplitSettlement", "Empty givenSubMids - skipping SplitSettlement filter");  
//             } else {  
//                 let st = get_gws();  
//                 let enabled_gateway_accounts = s::get_enabled_mgas_by_merchant_id_and_ref_id(  
//                     macc.merchant_id.clone(),  
//                     possible_ref_ids_of_merchant.clone(),  
//                 );  
//                 let filtered_gateway_accounts: Vec<_> = enabled_gateway_accounts  
//                     .into_iter()  
//                     .filter_map(|gwacc| {  
//                         let gw_ref_id = utils::get_gateway_reference_id(&metadata, gwacc.gateway.clone(), &oref, &pl_ref_id_map);  
//                         if gwacc.reference_id == gw_ref_id && st.contains(&gwacc.gateway) {  
//                             Some(gwacc)  
//                         } else {  
//                             None  
//                         }  
//                     })  
//                     .collect();  
//                 let merchant_gateway_account_sub_infos = etmgasi::find_all_mgasi_by_maga_ids(  
//                     filtered_gateway_accounts.iter().map(|mga| mga.id.clone()).collect(),  
//                 );  
//                 let merchant_gateway_account_list_map: std::collections::HashMap<_, _> = filtered_gateway_accounts  
//                     .into_iter()  
//                     .fold(std::collections::HashMap::new(), |mut acc, mga| {  
//                         let sub_infos: Vec<_> = merchant_gateway_account_sub_infos  
//                             .iter()  
//                             .filter(|mgasi| {  
//                                 mgasi.merchant_gateway_account_id == mga.id  
//                                     && mgasi.sub_id_type == SubIdType::Vendor  
//                                     && mgasi.sub_info_type == SubInfoType::SplitSettlement  
//                                     && !mgasi.disabled  
//                             })  
//                             .map(|mgasi| mgasi.juspay_sub_account_id.clone())  
//                             .collect();  
//                         acc.insert((mga.id.clone(), mga.gateway.clone()), sub_infos);  
//                         acc  
//                     });  
//                 let map_keys: Vec<_> = merchant_gateway_account_list_map  
//                     .iter()  
//                     .filter(|(_, v)| intersect(&given_sub_mids, v).len() == given_sub_mids_size)  
//                     .map(|(k, _)| k.clone())  
//                     .collect();  
//                 let mga_ids: Vec<_> = ord_nub(map_keys.iter().map(|k| k.0.clone()).collect());  
//                 let all_mgas = utils::get_mgas().unwrap_or_default();  
//                 set_gws_and_mgas(all_mgas.into_iter().filter(|mga| mga_ids.contains(&mga.id)).collect());  
//             }  
//         }  
//         Err(msg) => {  
//             log_debug_t(  
//                 format!("SplitSettlement for txn_id {}", txn_id),  
//                 format!("Skipping SplitSettlement filter : {}", msg),  
//             );  
//             let st = get_gws();  
//             let split_settlement_supported_gateways = r_service::find_by_name_from_redis(C::SPLIT_SETTLEMENT_SUPPORTED_GATEWAYS)  
//                 .unwrap_or_default();  
//             if !intersect(&split_settlement_supported_gateways, &st).is_empty() {  
//                 let mgas = s::get_split_settlement_only_gateway_accounts(possible_ref_ids_of_merchant.clone());  
//                 let all_mgas = utils::get_mgas().unwrap_or_default();  
//                 set_gws_and_mgas(  
//                     all_mgas  
//                         .into_iter()  
//                         .filter(|mga| !mgas.iter().any(|mga_filter| mga_filter.id == mga.id))  
//                         .collect(),  
//                 );  
//             }  
//         }  
//     }  
//     return_gw_list_with_log(FilterFunctionalGatewaysForSplitSettlement, true);  
// }  
  
// pub fn log_final_functional_gateways() -> DeciderFlow<GatewayList> {  
//     return_gw_list_with_log(FinalFunctionalGateways, false);  
// }