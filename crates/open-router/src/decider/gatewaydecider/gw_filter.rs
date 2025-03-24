use crate::decider::gatewaydecider::types::*;
use crate::types::merchant as ETM;
use crate::decider::gatewaydecider::utils as Utils;
use std::vec::Vec;
use crate::types::txn_details::types as ETTD;
use crate::types::gateway as ETG;
use crate::types::merchant::merchant_account::*;
use crate::types::card::*;
use crate::types::gateway::Gateway;
use crate::types::gateway_card_info as ETGCI;
use crate::decider::storage::utils::gateway_card_info as ETGCIS;
use crate::types::gateway_card_info::GatewayCardInfo;
use crate::types::merchant_gateway_card_info as ETMGCI;
use crate::types::card::txn_card_info::AuthType;
use std::collections::{HashMap, HashSet};
use crate::types::payment_flow::PaymentFlow;
use crate::types::order::Order;
// use crate::types::metadata::Meta;
// use crate::types::pl_ref_id_map::PLRefIdMap;
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::txn_details::types::TxnObjectType;

/// Checks if a transaction is either a card or netbanking transaction
fn isCardOrNbTxn(txnCI: &TxnCardInfo) -> bool {  
    Utils::is_card_transaction(txnCI) || Utils::is_nb_transaction(txnCI)  
}  
  
/// Checks if the transaction object type is an e-mandate register
pub fn isEmandateRegister(mTxnObjType: TxnObjectType) -> bool {  
    mTxnObjType == TxnObjectType::EmandateRegister  
}  
  
/// Checks if the transaction object type is a mandate register
pub fn isMandateRegister(mTxnObjType: TxnObjectType) -> bool {  
    mTxnObjType == TxnObjectType::MandateRegister  
}

fn validateMga(  
    mga: &MerchantGatewayAccount,  
    txnCI: &TxnCardInfo,  
    mTxnObjType: TxnObjectType,  
    mgaEligibleSeamlessGateways: &[Gateway],  
) -> bool {  
    if mgaEligibleSeamlessGateways.contains(&mga.gateway) && isCardOrNbTxn(txnCI) {  
        Utils::is_seamless(mga)  
    } else if isMandateRegister(mTxnObjType.clone()) {  
        Utils::is_subscription(mga)  
    } else if isEmandateRegister(mTxnObjType) {  
        Utils::is_emandate_enabled(mga)  
    } else if Utils::is_only_subscription(mga) {  
        false  
    } else {  
        true  
    }  
}

pub fn isMgaEligible(
    mga: &MerchantGatewayAccount,
    txnCI: &TxnCardInfo,
    mTxnObjType: TxnObjectType,
    mgaEligibleSeamlessGateways: &[Gateway],
) -> bool {
    validateMga(mga, txnCI, mTxnObjType, mgaEligibleSeamlessGateways)
}

fn areEqualArrays<T: Eq + Ord>(mut xs: Vec<T>, mut ys: Vec<T>) -> bool {   
    xs.sort();  
    ys.sort();  
    xs == ys  
} 

/// Evaluates which merchant gateway accounts support the required payment flows
/// for a specific gateway
fn evaluatePaymentFlowEnforcement(  
    meta: HashMap<String, String>,  
    oref: Order,  
    pl_ref_id_map: HashMap<String, String>,  
    txn_payment_flows: &Vec<String>,  
    gateway: &Gateway,  
    initial_mgas: &[ETM::merchant_gateway_account::MerchantGatewayAccount],  
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {  
    // Get gateway reference ID
    let gw_ref_id = Utils::get_gateway_reference_id(meta, gateway, oref, pl_ref_id_map);
    
    // Filter MGAs that match the gateway and reference ID
    let filtered_mgas: Vec<_> = initial_mgas  
        .iter()  
        .filter(|mga| mga.referenceId == gw_ref_id && mga.gateway == *gateway)  
        .cloned()  
        .collect();  
  
    // Extract all enforced payment flows from the filtered MGAs
    let enforced_payment_flows: Vec<String> = filtered_mgas  
        .iter()  
        .flat_map(|mga| {  
            mga.supported_payment_flows  
                .as_ref()  
                .and_then(|flows| flows.enforcedPaymentFlows.as_ref())  
                .map(|v| v.clone())
                .unwrap_or_else(Vec::new)  
        })  
        .collect::<HashSet<_>>()  
        .into_iter()  
        .collect();  
  
    // If there are enforced payment flows, filter MGAs that match the required flows
    if !enforced_payment_flows.is_empty() {  
        // Find which transaction payment flows are among the enforced ones
        let flows_to_enforce: Vec<String> = txn_payment_flows  
            .iter()  
            .filter(|flow| enforced_payment_flows.contains(flow))  
            .cloned()  
            .collect();  
  
        // Return only MGAs that exactly match the flows to enforce
        filtered_mgas  
            .into_iter()  
            .filter(|mga| {  
                let mga_enforced_flows = mga  
                    .supported_payment_flows  
                    .as_ref()  
                    .and_then(|flows| flows.enforcedPaymentFlows.as_ref())  
                    .cloned()
                    .unwrap_or_else(Vec::new);
                    
                    areEqualArrays(mga_enforced_flows, flows_to_enforce.clone())  
            })  
            .collect()  
    } else {  
        // If no enforced flows, return all filtered MGAs
        filtered_mgas  
    }  
}

pub fn filterMGAsByEnforcedPaymentFlows(  
    this: &mut DeciderFlow,
    initial_mgas: Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,  
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {  
    // Extract unique gateways from the merchant gateway accounts
    let gateways: Vec<Gateway> = initial_mgas
        .iter()
        .map(|mga| &mga.gateway)
        .collect::<HashSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    
    // Get context from DeciderFlow
    let txn_card_info = this.get().dpTxnCardInfo.clone();  
    let oref = this.get().dpOrder.clone();  
    let macc = this.get().dpMerchantAccount.clone();  
    let txn_detail = this.get().dpTxnDetail.clone();  
    
    // Get order metadata and PL reference ID map
    let (meta, pl_ref_id_map) = Utils::get_order_metadata_and_pl_ref_id_map(
            this,
            macc.enableGatewayReferenceIdBasedRouting, 
            &oref
        );  
  
    // Extract unique payment flows from transaction details
    let txn_payment_flows: Vec<String> = Utils::get_payment_flow_list_from_txn_detail(&txn_detail)
        .into_iter()
        .collect::<HashSet<_>>()  
        .into_iter()  
        .collect();
  
    // Evaluate payment flow enforcement for each gateway
    let eligible_mgas: Vec<Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>> = gateways  
        .into_iter()  
        .map(|gateway| {  
            evaluatePaymentFlowEnforcement(  
                meta.clone(),  
                oref.clone(),  
                pl_ref_id_map.clone(),  
                &txn_payment_flows,  
                &gateway,  
                &initial_mgas,  
            )  
        })  
        .collect();  
  
    // Flatten the results and return
    eligible_mgas.into_iter().flatten().collect()  
}

/// Sets the functional gateways in the DeciderFlow and updates related merchant gateway accounts
pub fn setGws(this: &mut DeciderFlow, gws: Vec<ETG::Gateway>) -> () {
    // Get the merchant gateway accounts
    let m_mgas = Utils::get_mgas(this);
    
    // Filter merchant gateway accounts based on gateway list
    if let Some(mgas) = m_mgas {
        let filtered_mgas = mgas.into_iter()
            .filter(|val| gws.contains(&val.gateway))
            .collect();
        Utils::set_mgas(this, filtered_mgas);
    }
    
    // Update the functional gateways in the DeciderFlow
    this.writer.functionalGateways = gws;
}

pub async fn filterGatewaysCardInfo(
    this: &mut DeciderFlow<'_>,
    merchant_account: MerchantAccount,
    card_bins: Vec<Option<String>>,
    enabled_gateways: Vec<Gateway>,
    m_auth_type: Option<AuthType>,
    m_validation_type: Option<ValidationType>,
) -> Vec<GatewayCardInfo> {
    let appState = this.state().clone();
    if !enabled_gateways.is_empty()
        && card_bins.iter().all(|bin| bin.is_some())
        && (m_auth_type.is_some() || m_validation_type.is_some())
    {
        if m_validation_type.clone().map(|vt| vt == ValidationType::CARD_MANDATE).unwrap_or(false) {
            let merchant_wise_mandate_supported_gateway: Vec<Gateway> =
                Utils::get_merchant_wise_mandate_bin_eligible_gateways(
                    &merchant_account,
                    &enabled_gateways,
                ).await;
            let merchant_wise_mandate_supported_gateway_prime: Vec<Option<Gateway>> =
                merchant_wise_mandate_supported_gateway
                    .iter()
                    .map(|g| Some(g.clone()))
                    .collect();

            let merchant_wise_eligible_gateway_card_info = if !merchant_wise_mandate_supported_gateway.is_empty() {
                ETGCIS::getSupportedGatewayCardInfoForBins(&appState, merchant_account, card_bins.clone())
                    .await.into_iter()
                    .filter(|ci| {
                        ci.gateway.is_some() && merchant_wise_mandate_supported_gateway_prime.contains(&ci.gateway)
                            && ci.validationType == Some(ETGCI::ValidationType::CardMandate)
                    })
                    .collect::<Vec<GatewayCardInfo>>()
            } else {
                vec![]
            };

            let filtered_gateways: Vec<Gateway> = enabled_gateways
                .iter()
                .filter(|g| !merchant_wise_mandate_supported_gateway.contains(g))
                .cloned()
                .collect();

            let eligible_gateway_card_info_prime = ETGCI::get_enabled_gateway_card_info_for_gateways(
                card_bins,
                filtered_gateways,
            ).await
            .into_iter()
            .filter(|ci| ci.validationType == Some(ETGCI::ValidationType::CardMandate))
            .collect::<Vec<GatewayCardInfo>>();

            let eligible_gateway_card_info = eligible_gateway_card_info_prime
                .into_iter()
                .chain(merchant_wise_eligible_gateway_card_info.into_iter())
                .collect::<Vec<_>>();

            eligible_gateway_card_info
        } else {
            let mut merchant_validation_required_gws = Vec::new();
            let mut gci_validation_gws = Vec::new();

            for gw in enabled_gateways.iter().cloned() {
                if Utils::is_merchant_wise_auth_type_check_needed(
                    &merchant_account,
                    m_auth_type.as_ref(),
                    m_validation_type.as_ref(),
                    &gw,
                ).await {
                    merchant_validation_required_gws.push(gw);
                } else {
                    gci_validation_gws.push(gw);
                }
            }

            let gcis = ETGCI::get_enabled_gateway_card_info_for_gateways(card_bins, enabled_gateways).await;

            let gcis_without_merchant_validation = gcis
                .iter()
                .filter(|gci| {
                    gci_validation_gws.contains(&gci.gateway.clone().unwrap_or(ETG::Gateway::NONE))
                })
                .cloned()
                .collect::<Vec<_>>();

            let gcis_with_merchant_validation = gcis
                .iter()
                .filter(|gci| {
                    merchant_validation_required_gws.contains(&gci.gateway.clone().unwrap_or(ETG::Gateway::NONE))
                })
                .cloned()
                .collect::<Vec<_>>();

            let gci_ids = gcis_with_merchant_validation
                .iter()
                .map(|gci| gci.id.clone())
                .collect::<Vec<_>>();

            let mgcis_enabled_gcis = if gci_ids.is_empty() {
                vec![]
            } else {
                ETMGCI::find_all_mgcis_by_macc_and_gci_p_id(merchant_account.id.clone(), gci_ids.clone()).await
            };

            let mgcis_enabled_gci_ids = mgcis_enabled_gcis
                .iter()
                .filter(|mgci| !mgci.disabled)
                .map(|mgci| mgci.gatewayCardInfoId.clone())
                .collect::<Vec<_>>();

            let gcis_after_merchant_validation = gcis
                .iter()
                .filter(|gci| mgcis_enabled_gci_ids.contains(&gci.id))
                .cloned()
                .collect::<Vec<_>>();

            let eligible_gateway_card_infos = gcis_without_merchant_validation
                .into_iter()
                .chain(gcis_after_merchant_validation.into_iter())
                .collect::<Vec<_>>();

            let eligible_gateway_card_info_prime = match m_validation_type {
                Some(ref v_type) => eligible_gateway_card_infos
                    .into_iter()
                    .filter(|ci| {
                        ci.validationType.clone().map(|v| v.to_string())
                            == Some(v_type.to_string())
                    })
                    .collect::<Vec<_>>(),
                None => eligible_gateway_card_infos
                    .into_iter()
                    .filter(|ci| {
                        ci.authType
                            == m_auth_type.as_ref().map(|auth| auth.to_string())
                    })
                    .collect::<Vec<_>>(),
            };

            // debug!("gcis_without_merchant_validation: {:?}", gcis_without_merchant_validation);
            // debug!("gcis_after_merchant_validation: {:?}", gcis_after_merchant_validation);
            // debug!("eligible_gateway_card_info_prime: {:?}", eligible_gateway_card_info_prime);

            eligible_gateway_card_info_prime
        }
    } else {
        vec![]
    }
}


pub fn getGws(this: &mut DeciderFlow) -> Vec<ETG::Gateway> {  
    this.writer.functionalGateways.clone()
}

fn makeFirstLetterSmall(s: String) -> String {  
    let mut chars = s.chars();  
    if let Some(first) = chars.next() {  
        first.to_lowercase().chain(chars).collect()  
    } else {  
        s  
    }  
}

pub fn returnGwListWithLog(this: &mut DeciderFlow, fName: DeciderFilterName, doOrNot: bool) -> Vec<ETG::Gateway> {  
    // Get the current list of functional gateways
    let fgws = this.writer.functionalGateways.clone();
    
    // Get the transaction ID from the context
    let txn_id = this.get().dpTxnDetail.txnId.clone();
    
    // Log the filtered gateways
    // debug!(
    //     "GW_Filtering: Functional gateways after {} for {} : {:?}",  
    //     fName.to_string(),  
    //     ETTD::transaction_id_text(&txn_id),  
    //     fgws  
    // );
    
    // If tracking is enabled, add to the debug filter list
    if doOrNot {  
        this.writer.debugFilterList.push(DebugFilterEntry {  
            filterName: makeFirstLetterSmall(fName.to_string()),  
            gateways: fgws.clone(),  
        });  
    }
    
    // Return the list of gateways
    fgws  
}

pub fn ord_nub<T>(v: Vec<T>) -> Vec<T>
where
    T: std::cmp::Ord + std::clone::Clone,
{
    let mut v_mut = v;
    v_mut.sort();
    v_mut.dedup();
    v_mut
}

pub fn setGwsAndMgas(this: &mut DeciderFlow, filteredMgas: Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>) -> () {
    Utils::set_mgas(this, filteredMgas.clone());
    this.writer.functionalGateways = ord_nub(filteredMgas.iter().map(|mga| mga.gateway.clone()).collect());
}

