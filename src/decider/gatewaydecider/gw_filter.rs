use crate::decider::gatewaydecider::constants::CASH_ONLY_GATEWAYS;
use crate::decider::gatewaydecider::types::*;
use crate::decider::gatewaydecider::utils as Utils;
use crate::decider::storage::utils::gateway_card_info as ETGCIS;
use crate::merchant_config_util::isPaymentFlowEnabledWithHierarchyCheck;
use crate::redis::feature::{isFeatureEnabled, isFeatureEnabledByDimension};
use crate::redis::types::ServiceConfigKey;
use crate::types::bank_code::find_bank_code;
use crate::types::card::card_type as ETCA;
use crate::types::card::txn_card_info::{self as ETTCa, auth_type_to_text};
use crate::types::card::vault_provider::VaultProvider;
use crate::types::gateway as ETG;
use crate::types::gateway as GT;
use crate::types::gateway_bank_emi_support::GatewayBankEmiSupport;
use crate::types::gateway_bank_emi_support_v2::GatewayBankEmiSupportV2;
use crate::types::gateway_card_info as ETGCI;
use crate::types::gateway_card_info::GatewayCardInfo;
use crate::types::merchant as ETM;
use crate::types::merchant::merchant_account::*;
use crate::types::merchant::merchant_gateway_account as ETMA;
use crate::types::merchant_config::merchant_config as MerchantConfig;
use crate::types::merchant_gateway_card_info as ETMGCI;
use crate::types::payment_flow::PaymentFlow as PF;
use crate::types::tenant::tenant_config::ModuleName as TC;
use crate::types::txn_details::types::TxnDetail;
use crate::types::txn_offer_detail::{self as ETOD};
use std::vec::Vec;

use crate::decider::storage::utils::gateway_bank_emi_support as SGBES;
use crate::decider::storage::utils::merchant_gateway_account as SETMA;
use crate::decider::storage::utils::txn_card_info as SUTC;
use crate::types::card::txn_card_info::AuthType;
use crate::types::gateway_payment_method_flow::{
    self as GPMF, to_gateway_payment_method_flow_id, GatewayPaymentMethodFlowId,
};
use crate::types::merchant_gateway_account_sub_info::{self as ETMGASI, SubIdType, SubInfoType};
use crate::types::merchant_gateway_payment_method_flow as MGPMF;
use crate::types::order::Order;
use crate::types::payment::payment_method::{self as ETP};
use crate::types::payment_flow::PaymentFlow;
use std::collections::{HashMap, HashSet};
// use crate::types::metadata::Meta;
// use crate::types::pl_ref_id_map::PLRefIdMap;
use crate::decider::gatewaydecider::constants as C;
use crate::decider::storage::utils::merchant_gateway_card_info as SETMCI;
use crate::logger;
use crate::redis::cache::findByNameFromRedis;
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::currency::Currency;
use crate::types::feature as ETF;
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;
use crate::types::payment::payment_method_type_const::*;
use crate::types::transaction::id::transaction_id_to_text;
use crate::types::txn_details::types::TxnObjectType;
use masking::PeekInterface;
use serde_json;
use serde_json::Value as AValue;

pub fn ord_nub<T>(v: Vec<T>) -> Vec<T>
where
    T: std::cmp::Ord + std::clone::Clone,
{
    let mut v_mut = v;
    v_mut.sort();
    v_mut.dedup();
    v_mut
}

pub fn getGws(this: &mut DeciderFlow) -> Vec<String> {
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

pub fn returnGwListWithLog(
    this: &mut DeciderFlow,
    fName: DeciderFilterName,
    doOrNot: bool,
) -> Vec<String> {
    // Get the current list of functional gateways
    let fgws = this.writer.functionalGateways.clone();

    // Get the transaction ID from the context
    let txn_id = this.get().dpTxnDetail.txnId.clone();

    // Log the filtered gateways
    logger::debug!(
        action = "GW_Filtering",
        tag = "GW_Filtering",
        "Functional gateways after {:?} for {:?} : {:?}",
        fName.to_string(),
        txn_id,
        fgws
    );

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

/// Filters out None values from a vector of Options and unwraps the Some values
pub fn catMaybes<T: Clone>(options: &[Option<T>]) -> Vec<T> {
    options.iter().filter_map(|opt| opt.clone()).collect()
}

pub fn intersect<T: Eq + std::hash::Hash>(a: &[T], b: &[T]) -> Vec<T>
where
    T: Clone,
{
    let set_a: HashSet<_> = a.iter().collect();
    let set_b: HashSet<_> = b.iter().collect();
    set_a.intersection(&set_b).cloned().cloned().collect()
}

pub fn setGwsAndMgas(
    this: &mut DeciderFlow,
    filteredMgas: Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,
) {
    Utils::set_mgas(this, filteredMgas.clone());
    this.writer.functionalGateways =
        ord_nub(filteredMgas.iter().map(|mga| mga.gateway.clone()).collect());
}

/// Sets the functional gateways in the DeciderFlow and updates related merchant gateway accounts
pub fn setGws(this: &mut DeciderFlow, gws: Vec<String>) {
    // Get the merchant gateway accounts
    let m_mgas = Utils::get_mgas(this);

    // Filter merchant gateway accounts based on gateway list
    if let Some(mgas) = m_mgas {
        let filtered_mgas = mgas
            .into_iter()
            .filter(|val| gws.contains(&val.gateway))
            .collect();
        Utils::set_mgas(this, filtered_mgas);
    }

    // Update the functional gateways in the DeciderFlow
    this.writer.functionalGateways = gws;
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

pub async fn newGwFilters(
    this: &mut DeciderFlow<'_>,
) -> Result<
    (
        GatewayList,
        Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,
    ),
    ErrorResponse,
> {
    let _ = getFunctionalGateways(this).await;
    let gws = filterFunctionalGateways(this).await;
    if gws.is_empty() {
        let txnId = this.get().dpTxnDetail.txnId.clone();
        let merchantId = this.get().dpTxnDetail.merchantId.clone();
        logger::warn!(
            tag = "GW_Filtering",
            action = "GW_Filtering",
            "There are no functional gateways for {:?} for merchant: {:?}",
            txnId,
            merchantId
        );
        Utils::log_gateway_decider_approach(
            this,
            None,
            None,
            vec![],
            GatewayDeciderApproach::NONE,
            None,
            vec![],
            None,
        )
        .await;
        Ok((vec![], vec![]))
    } else {
        let _ = filterGatewaysForBrand(this).await;
        filterGatewaysForAuthType(this).await?;
        filterGatewaysForValidationType(this).await?;
        let _ = filterGatewaysForEmi(this).await;
        let _ = filterGatewaysForTxnOfferDetails(this).await;
        let _ = filterGatewaysForPaymentMethod(this).await;
        let _ = filterGatewaysForTokenProvider(this).await;
        let _ = filterGatewaysForWallet(this).await;
        let _ = filterGatewaysForNbOnly(this).await;
        let _ = filterGatewaysForConsumerFinance(this).await;
        let _ = filterGatewaysForUpi(this).await;
        let _ = filterGatewaysForTxnType(this).await;
        let _ = filterGatewaysForTxnDetailType(this).await;
        let _ = filterGatewaysForReward(this).await;
        let _ = filterGatewaysForCash(this).await;
        let _ = filterFunctionalGatewaysForSplitSettlement(this).await;
        let _ = filterFunctionalGatewaysForMerchantRequiredFlow(this).await;
        let _ = filterFunctionalGatewaysForOTMFlow(this).await;
        let _ = filterGatewaysForMGASelectionIntegrity(this).await;
        let funcGateways =
            returnGwListWithLog(this, DeciderFilterName::FinalFunctionalGateways, false);
        let allMgas = if Utils::get_is_merchant_enabled_for_dynamic_mga_selection(this).await {
            Utils::get_mgas(this)
        } else {
            None
        };
        Ok((funcGateways, allMgas.unwrap_or_default()))
    }
}

pub async fn getFunctionalGateways(this: &mut DeciderFlow<'_>) -> GatewayList {
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let oref = this.get().dpOrder.clone();
    let macc = this.get().dpMerchantAccount.clone();
    let txn_id = this.get().dpTxnDetail.txnId.clone();
    let txn_detail = this.get().dpTxnDetail.clone();
    let is_edcc_applied = this.get().dpEDCCApplied;
    let enforce_gateway_list = this.get().dpEnforceGatewayList.clone();

    logger::info!(
        tag = "enableGatewayReferenceIdBasedRouting",
        action = "enableGatewayReferenceIdBasedRouting",
        "enableGatewayReferenceIdBasedRouting is enable or not for txn_id : {:?}, enableGatewayReferenceIdBasedRouting: {:?}",
        txn_id,
        macc.enableGatewayReferenceIdBasedRouting
    );

    let (meta, pl_ref_id_map) = Utils::get_order_metadata_and_pl_ref_id_map(
        this,
        macc.enableGatewayReferenceIdBasedRouting,
        &oref,
    );

    let proceed_with_all_mgas = Utils::is_enabled_for_all_mgas(this);
    let enabled_gateway_accounts = if proceed_with_all_mgas {
        SETMA::get_all_enabled_mgas_by_merchant_id(macc.merchantId).await
    } else {
        logger::debug!(
            "metadata in getFunctionalGateways for txnId : {:?}, metadata: {:?}",
            txn_id,
            meta
        );
        let possible_ref_ids_of_merchant =
            Utils::get_all_possible_ref_ids(meta.clone(), oref.clone(), pl_ref_id_map.clone());
        SETMA::get_enabled_mgas_by_merchant_id_and_ref_id(
            this,
            macc.merchantId,
            possible_ref_ids_of_merchant,
        )
        .await
    };

    let payment_flow_list = Utils::get_payment_flow_list_from_txn_detail(&txn_detail);
    Utils::set_payment_flow_list(this, payment_flow_list);

    let mgas_ = match (
        txn_detail.isEmi || Utils::is_reccuring_payment_transaction(&txn_detail),
        &enforce_gateway_list,
    ) {
        (false, _) => enabled_gateway_accounts.clone(),
        (_, None) => enabled_gateway_accounts.clone(),
        (_, Some(en_gateway_list)) if en_gateway_list.is_empty() => {
            enabled_gateway_accounts.clone()
        }
        (_, Some(en_gateway_list)) => enabled_gateway_accounts
            .into_iter()
            .filter(|mga| en_gateway_list.contains(&mga.gateway))
            .collect(),
    };

    let mgas__ = if proceed_with_all_mgas {
        mgas_
    } else {
        mgas_
            .into_iter()
            .filter(|mga| {
                let gw_ref_id = Utils::get_gateway_reference_id(
                    meta.clone(),
                    &mga.gateway,
                    oref.clone(),
                    pl_ref_id_map.clone(),
                );
                mga.referenceId == gw_ref_id
            })
            .collect()
    };

    validateAndSetDynamicMGAFlag(this, proceed_with_all_mgas, &mgas__);

    let edcc_mgas = if txn_detail.currency != oref.currency && is_edcc_applied == Some(true) {
        let edcc_supported_gateways: Vec<String> =
            findByNameFromRedis(C::EDCC_SUPPORTED_GATEWAYS.get_key())
                .await
                .unwrap_or_else(Vec::new);

        mgas__
            .into_iter()
            .filter(|mga| {
                edcc_supported_gateways.contains(&mga.gateway)
                    && Utils::check_if_enabled_in_mga(
                        mga,
                        "DYNAMIC_CURRENCY_CONVERSION",
                        "isEdccSupported",
                    )
            })
            .collect()
    } else {
        mgas__
    };

    let mgas = if proceed_with_all_mgas {
        edcc_mgas
    } else {
        filterMGAsByEnforcedPaymentFlows(this, edcc_mgas)
    };

    if mgas.is_empty() {
        setGwsAndMgas(this, vec![]);
        returnGwListWithLog(this, DeciderFilterName::GetFunctionalGateways, true)
    } else {
        let currency_filtered_mgas = mgas
            .into_iter()
            .filter(|mga| currencyFilter(txn_detail.currency.clone(), mga))
            .collect::<Vec<_>>();

        let rpd_filter_mgas = if Utils::is_reverse_penny_drop_txn(&txn_detail) {
            currency_filtered_mgas
                .clone()
                .into_iter()
                .filter(Utils::check_for_reverse_penny_drop_in_mga)
                .collect()
        } else {
            currency_filtered_mgas.clone()
        };
        setGwsAndMgas(this, currency_filtered_mgas.clone());
        returnGwListWithLog(
            this,
            DeciderFilterName::FilterFunctionalGatewaysForCurrency,
            true,
        );

        let filtered_mgas = if proceed_with_all_mgas {
            rpd_filter_mgas
        } else {
            let mga_eligible_seamless_gateways =
                findByNameFromRedis(C::MGA_ELIGIBLE_SEAMLESS_GATEWAYS.get_key())
                    .await
                    .unwrap_or_else(std::vec::Vec::new);
            rpd_filter_mgas
                .into_iter()
                .filter(|mga| {
                    isMgaEligible(
                        mga,
                        &txn_card_info,
                        txn_detail.txnObjectType.clone(),
                        &mga_eligible_seamless_gateways,
                        &txn_detail,
                    )
                })
                .collect()
        };

        setGwsAndMgas(this, filtered_mgas);
        returnGwListWithLog(
            this,
            DeciderFilterName::FilterFunctionalGatewaysForReversePennyDrop,
            true,
        )
    }
}

fn validateAndSetDynamicMGAFlag(
    this: &mut DeciderFlow,
    proceed_with_all_mgas: bool,
    mgas: &Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,
) {
    let gwts: Vec<_> = mgas.iter().map(|mga| &mga.gateway).collect();
    if !proceed_with_all_mgas
        && gwts.len()
            != gwts
                .iter()
                .cloned()
                .collect::<std::collections::HashSet<_>>()
                .len()
    {
        Utils::set_is_merchant_enabled_for_dynamic_mga_selection(this, true);
    }
}

pub fn filterMGAsByEnforcedPaymentFlows(
    this: &mut DeciderFlow,
    initial_mgas: Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {
    // Extract unique gateways from the merchant gateway accounts
    let gateways: Vec<String> = initial_mgas
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
        &oref,
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

/// Evaluates which merchant gateway accounts support the required payment flows
/// for a specific gateway
fn evaluatePaymentFlowEnforcement(
    meta: HashMap<String, String>,
    oref: Order,
    pl_ref_id_map: HashMap<String, String>,
    txn_payment_flows: &Vec<String>,
    gateway: &String,
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
                .cloned()
                .unwrap_or_default()
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

fn areEqualArrays<T: Eq + Ord>(mut xs: Vec<T>, mut ys: Vec<T>) -> bool {
    xs.sort();
    ys.sort();
    xs == ys
}

/// Filters merchant gateway accounts based on currency support
pub fn currencyFilter(order_curr: Currency, mga: &MerchantGatewayAccount) -> bool {
    // Use if-let pattern for cleaner Option handling
    if let Some(currencies) = getTrueString(mga.supportedCurrencies.clone()) {
        canAcceptCurrency(&currencies, order_curr)
    } else {
        order_curr == Currency::INR
    }
}

fn getTrueString(val: Option<String>) -> Option<String> {
    val.filter(|text| !text.to_string().is_empty())
}

/// Helper function to check if a currency is in the supported list
pub fn canAcceptCurrency(supported_mga_currencies: &str, currency: Currency) -> bool {
    // Parse the supported currencies text to a list of Currency objects
    let curr_list: Vec<Currency> =
        match serde_json::from_str::<Vec<String>>(supported_mga_currencies) {
            Ok(strings) => strings
                .into_iter()
                .filter_map(|curr| Currency::text_to_curr(&curr).ok())
                .collect(),
            Err(_) => Vec::new(),
        };
    // Currency is supported if it's in the list or if list is empty and currency is INR
    curr_list.contains(&currency) || (curr_list.is_empty() && currency == Currency::INR)
}

pub fn isMgaEligible(
    mga: &MerchantGatewayAccount,
    txnCI: &TxnCardInfo,
    mTxnObjType: TxnObjectType,
    mgaEligibleSeamlessGateways: &[String],
    txn_detail: &TxnDetail,
) -> bool {
    let payment_flow_list = Utils::get_payment_flow_list_from_txn_detail(&txn_detail);
    let is_otm_flow = payment_flow_list.contains(&"ONE_TIME_MANDATE".to_string());
    validateMga(
        mga,
        txnCI,
        mTxnObjType,
        mgaEligibleSeamlessGateways,
        is_otm_flow,
    )
}

fn validateMga(
    mga: &MerchantGatewayAccount,
    txnCI: &TxnCardInfo,
    mTxnObjType: TxnObjectType,
    mgaEligibleSeamlessGateways: &[String],
    is_otm_flow: bool,
) -> bool {
    if mgaEligibleSeamlessGateways.contains(&mga.gateway) && isCardOrNbTxn(txnCI) {
        Utils::is_seamless(mga)
    } else if isMandateRegister(mTxnObjType.clone()) {
        Utils::is_subscription(mga)
    } else if (isEmandateRegister(mTxnObjType) && !is_otm_flow) {
        Utils::is_emandate_enabled(mga)
    } else {
        !Utils::is_only_subscription(mga)
    }
}

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

pub async fn filterFunctionalGateways(this: &mut DeciderFlow<'_>) -> GatewayList {
    let txnDetail = this.get().dpTxnDetail.clone();
    let txnCardInfo = this.get().dpTxnCardInfo.clone();
    let mAcc = this.get().dpMerchantAccount.clone();
    let mInternalMeta: Option<InternalMetadata> = txnDetail
        .internalMetadata
        .as_ref()
        .and_then(|meta| serde_json::from_str(meta).ok());

    Utils::set_internal_meta_data(this, mInternalMeta.clone());

    // CVV Less Gateway Validations
    if Utils::is_card_transaction(&txnCardInfo) {
        if let Some(true) = mInternalMeta.as_ref().and_then(|meta| meta.isCvvLessTxn) {
            if txnCardInfo.authType == Some(AuthType::MOTO) {
                let st = getGws(this);
                let authTypeRestrictedGateways =
                    findByNameFromRedis::<HashMap<AuthType, Vec<String>>>(
                        C::AUTH_TYPE_RESTRICTED_GATEWAYS.get_key(),
                    )
                    .await
                    .unwrap_or_else(HashMap::new);
                let motoSupportedGateways: Vec<String> = txnCardInfo
                    .authType
                    .as_ref()
                    .and_then(|auth_type| authTypeRestrictedGateways.get(auth_type))
                    .cloned()
                    .unwrap_or_default();
                let filtered_gateways: Vec<String> = st
                    .into_iter()
                    .filter(|gw| motoSupportedGateways.contains(gw))
                    .collect();
                logger::info!(
                    tag = "filterFunctionalGateways",
                    action = "filterFunctionalGateways",
                    "Functional gateways after filtering for MOTO cvvLessTxns support for txn_id: {:?}",
                    txnDetail.txnId
                );
                setGws(this, filtered_gateways);
            } else if Utils::is_token_repeat_txn(mInternalMeta.clone()) {
                let brand = txnCardInfo
                    .cardSwitchProvider
                    .as_ref()
                    .map(|provider| provider.peek().to_string())
                    .unwrap_or_else(|| "DEFAULT".to_string());
                let isMerchantEnabledForCvvLessV2Flow = isFeatureEnabled(
                    C::cvvLessV2Flow.get_key(),
                    mAcc.merchantId.0,
                    "kv_redis".to_string(),
                )
                .await;
                if isMerchantEnabledForCvvLessV2Flow {
                    let configResp = isPaymentFlowEnabledWithHierarchyCheck(
                        mAcc.id.clone(),
                        mAcc.tenantAccountId,
                        TC::MERCHANT_CONFIG,
                        PF::CVVLESS,
                        crate::types::country::country_iso::text_db_to_country_iso(
                            mAcc.country.as_deref().unwrap_or_default(),
                        )
                        .ok(),
                    )
                    .await;
                    let functionalGateways = if !configResp {
                        logger::error!(
                            tag = "CVVLESS-ERROR",
                            action = "CVVLESS-ERROR",
                            "CVVLESS_FLOW_DISABLED for txn_id: {:?}",
                            txnDetail.txnId
                        );
                        Vec::new()
                    } else {
                        let st = getGws(this);
                        let mgaList = Utils::get_mgas(this).unwrap_or_default();
                        let isBrandSupportsCvvlessTR =
                            is_brand_supports_cvvless(&txnCardInfo, &brand).await;
                        if isBrandSupportsCvvlessTR {
                            let mPmEntryDB = ETP::get_by_name(brand).await;
                            if let Some(cardPaymentMethod) = mPmEntryDB {
                                let uniqueGwLs: Vec<String> = st.into_iter().collect();
                                let allGPMfEntries =
                                    GPMF::find_all_gpmf_by_gateway_payment_flow_payment_method(
                                        uniqueGwLs.clone(),
                                        cardPaymentMethod.id,
                                        PaymentFlow::CVVLESS,
                                    )
                                    .await;
                                let gmpfGws: Vec<String> = allGPMfEntries
                                    .iter()
                                    .map(|gpmf| gpmf.gateway.clone())
                                    .collect();
                                let filteredMga: Vec<MerchantGatewayAccount> = mgaList
                                    .into_iter()
                                    .filter(|mga| gmpfGws.contains(&mga.gateway))
                                    .collect();
                                let mgaIds: Vec<i64> = filteredMga
                                    .iter()
                                    .map(|mga| mga.id.merchantGwAccId)
                                    .collect();
                                let gpmfIds: Vec<GatewayPaymentMethodFlowId> =
                                    allGPMfEntries.iter().map(|gpmf| gpmf.id.clone()).collect();
                                let mgpmfEntries =
                                    MGPMF::get_all_mgpmf_by_mga_id_and_gpmf_ids(mgaIds, gpmfIds)
                                        .await;
                                let filteredMgaList: Vec<i64> = mgpmfEntries
                                    .iter()
                                    .map(|mgpmf| mgpmf.merchantGatewayAccountId)
                                    .collect();
                                let finalFilteredMga: Vec<MerchantGatewayAccount> = filteredMga
                                    .into_iter()
                                    .filter(|mga| filteredMgaList.contains(&mga.id.merchantGwAccId))
                                    .collect();
                                Utils::set_mgas(this, finalFilteredMga.clone());
                                finalFilteredMga
                                    .into_iter()
                                    .map(|mga| mga.gateway)
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        }
                    };
                    logger::info!(
                        tag = "filterFunctionalGateways",
                        action = "filterFunctionalGateways",
                        "Functional gateways after filtering for token repeat cvvLessTxns support for txn_id: {:?}",
                        txnDetail.txnId
                    );
                    setGws(this, functionalGateways);
                } else {
                    let isBrandSupportsCvvlessTR =
                        is_brand_supports_cvvless(&txnCardInfo, &brand).await;
                    let functionalGateways = if isBrandSupportsCvvlessTR {
                        let mTokenRepeatCvvlessSupportedGateways =
                            Utils::get_token_supported_gateways(
                                txnDetail.clone(),
                                txnCardInfo.clone(),
                                "CVV_LESS".to_string(),
                                mInternalMeta.clone(),
                            )
                            .await;
                        // let filteredGatewayFromMerchantConfig = Utils::filtered_gateways_merchant_config(
                        //     mTokenRepeatCvvlessSupportedGateways.clone(),
                        //     PF::CVVLESS,
                        //     &mAcc,
                        //     &brand,
                        // ).await?;
                        let filteredGatewayFromMerchantConfig =
                            mTokenRepeatCvvlessSupportedGateways.clone();
                        let tokenRepeatCvvlessSupportedGateways =
                            filteredGatewayFromMerchantConfig.unwrap_or_default();
                        let st = getGws(this);
                        st.into_iter()
                            .filter(|gw| tokenRepeatCvvlessSupportedGateways.contains(gw))
                            .collect()
                    } else {
                        Vec::new()
                    };
                    logger::info!(
                        tag = "filterFunctionalGateways",
                        action = "filterFunctionalGateways",
                        "Functional gateways after filtering for token repeat cvvLessTxns support for txn_id: {:?}",
                        txnDetail.txnId
                    );
                    setGws(this, functionalGateways)
                }
            } else {
                let cardBrandToCvvLessTxnSupportedGateways: HashMap<String, Vec<String>> =
                    findByNameFromRedis(C::CARD_BRAND_TO_CVVLESS_TXN_SUPPORTED_GATEWAYS.get_key())
                        .await
                        .unwrap_or_default();
                let cvvLessTxnSupportedCommonGateways: Vec<String> =
                    findByNameFromRedis(C::CVVLESS_TXN_SUPPORTED_COMMON_GATEWAYS.get_key())
                        .await
                        .unwrap_or_default();
                let cvvLessTxnSupportedGateways = cvvLessTxnSupportedCommonGateways
                    .into_iter()
                    .chain(
                        cardBrandToCvvLessTxnSupportedGateways
                            .get(&txnCardInfo.paymentMethod)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if !cvvLessTxnSupportedGateways.is_empty() {
                    let st = getGws(this);
                    let filtered_gateways: Vec<String> = st
                        .into_iter()
                        .filter(|gw| cvvLessTxnSupportedGateways.contains(gw))
                        .collect();
                    logger::info!(
                        tag = "filterFunctionalGateways",
                        action = "filterFunctionalGateways",
                        "Functional gateways after filtering for cvvLessTxns for txn_id: {:?}",
                        txnDetail.txnId
                    );
                    setGws(this, filtered_gateways);
                }
            }
        }
    }

    // Card token based repeat transaction gateway filter
    if Utils::is_card_transaction(&txnCardInfo) && Utils::is_token_repeat_txn(mInternalMeta.clone())
    {
        if let Some(secAuthType) = txnCardInfo.authType.clone() {
            if secAuthType == AuthType::OTP {
                let mTokenRepeatOtpSupportedGateways = Utils::get_token_supported_gateways(
                    txnDetail.clone(),
                    txnCardInfo.clone(),
                    "OTP".to_string(),
                    mInternalMeta.clone(),
                )
                .await;
                let st = getGws(this);
                let tokenRepeatOtpSupportedGateways =
                    mTokenRepeatOtpSupportedGateways.unwrap_or_default();
                let filtered_gateways: Vec<String> = st
                    .into_iter()
                    .filter(|gw| tokenRepeatOtpSupportedGateways.contains(gw))
                    .collect();
                setGws(this, filtered_gateways);
            }
        }
        let mTokenRepeatSupportedGateways = Utils::get_token_supported_gateways(
            txnDetail.clone(),
            txnCardInfo.clone(),
            "CARD".to_string(),
            mInternalMeta,
        )
        .await;
        let st = getGws(this);
        let tokenRepeatSupportedGateways = mTokenRepeatSupportedGateways.unwrap_or_default();
        let filtered_gateways: Vec<String> = if tokenRepeatSupportedGateways.is_empty() {
            st
        } else {
            st.into_iter()
                .filter(|gw| tokenRepeatSupportedGateways.contains(gw))
                .collect()
        };
        setGws(this, filtered_gateways);
    }

    // Amex BTA Card based gateway filter
    if Utils::is_card_transaction(&txnCardInfo) && txnCardInfo.authType == Some(AuthType::MOTO) {
        let paymentFlowList = Utils::get_payment_flow_list_from_txn_detail(&txnDetail);
        let st = getGws(this);
        if paymentFlowList.contains(&"TA_FILE".to_string()) {
            let taOfflineEnabledGateways: Vec<String> =
                findByNameFromRedis::<Vec<String>>(C::TA_OFFLINE_ENABLED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
            let filtered_gateways: Vec<String> = st
                .into_iter()
                .filter(|gw| taOfflineEnabledGateways.contains(gw))
                .collect();
            setGws(this, filtered_gateways);
        }
    }

    let st = getGws(this);
    logger::info!(
        tag = "filterFunctionalGateways",
        action = "filterFunctionalGateways",
        "Functional gateways before filtering for MerchantContainer for txn_id: {:?}",
        txnDetail.txnId
    );
    let merchantContainerSupportedGateways: Vec<String> =
        findByNameFromRedis(C::MERCHANT_CONTAINER_SUPPORTED_GATEWAYS.get_key())
            .await
            .unwrap_or_default();
    let filtered_gateways: Vec<String> = if txnCardInfo.paymentMethodType == MERCHANT_CONTAINER {
        st.into_iter()
            .filter(|gw| merchantContainerSupportedGateways.contains(gw))
            .collect()
    } else {
        st.into_iter()
            .filter(|gw| !merchantContainerSupportedGateways.contains(gw))
            .collect()
    };
    setGws(this, filtered_gateways);
    returnGwListWithLog(this, DeciderFilterName::FilterFunctionalGateways, true)
}

async fn is_brand_supports_cvvless(txnCardInfo: &TxnCardInfo, brand: &str) -> bool {
    if brand == "RUPAY" {
        check_cvv_less_support_rupay(txnCardInfo).await
    } else {
        true
    }
}

async fn check_cvv_less_support_rupay(txnCardInfo: &TxnCardInfo) -> bool {
    let bankCode = Utils::fetch_juspay_bank_code(txnCardInfo);
    let mCardType = txnCardInfo.card_type.as_ref().map(ETCA::card_type_to_text);
    if let Some(bCode) = bankCode {
        let feature_key =
            C::getTokenRepeatCvvLessBankCodeKey(txnCardInfo.cardSwitchProvider.clone()).get_key();
        let dimension = format!(
            "{}::{}",
            bCode,
            mCardType.unwrap_or_default().to_uppercase()
        );
        isFeatureEnabledByDimension(feature_key, dimension).await
    } else {
        false
    }
}

/// Filters functional gateways based on card brand
/// Only keeps gateways that support the detected card brand
pub async fn filterGatewaysForBrand(this: &mut DeciderFlow<'_>) -> Vec<String> {
    // Get the current list of functional gateways
    let st = getGws(this);

    // Get the card brand from the transaction
    let card_brand = Utils::get_card_brand(this).await;

    // Filter gateways by card brand
    let new_st = filterByCardBrand(this, &st, card_brand.as_deref()).await;

    // Update the functional gateways in state
    setGws(this, new_st);

    // Log the results and return the filtered gateway list
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForBrand,
        true,
    )
}

// Filters gateways based on the card brand
/// - AMEX cards: only use AMEX-supported gateways
/// - SODEXO cards: only use SODEXO-only or SODEXO-also gateways
/// - Others: filter out AMEX-not-supported and SODEXO-only gateways
/// - Unknown brand: filter out AMEX-not-supported gateways
pub async fn filterByCardBrand(
    this: &mut DeciderFlow<'_>,
    st: &[String],
    card_brand: Option<&str>,
) -> Vec<String> {
    let amex_supported_gateways: HashSet<String> =
        findByNameFromRedis(C::AMEX_SUPPORTED_GATEWAYS.get_key())
            .await
            .unwrap_or_else(Vec::new)
            .into_iter()
            .collect();

    let amex_not_supported_gateways: HashSet<String> =
        findByNameFromRedis(C::AMEX_NOT_SUPPORTED_GATEWAYS.get_key())
            .await
            .unwrap_or_else(Vec::new)
            .into_iter()
            .collect();

    let sodexo_only_gateways: HashSet<String> =
        findByNameFromRedis(C::SODEXO_ONLY_GATEWAYS.get_key())
            .await
            .unwrap_or_else(Vec::new)
            .into_iter()
            .collect();

    let sodexo_also_gateways: HashSet<String> =
        findByNameFromRedis(C::SODEXO_ALSO_GATEWAYS.get_key())
            .await
            .unwrap_or_else(Vec::new)
            .into_iter()
            .collect();

    match card_brand {
        Some("AMEX") => st
            .iter()
            .filter(|gw| amex_supported_gateways.contains(*gw))
            .cloned()
            .collect(),

        Some("SODEXO") => st
            .iter()
            .filter(|gw| sodexo_only_gateways.contains(*gw) || sodexo_also_gateways.contains(*gw))
            .cloned()
            .collect(),

        Some(brand) if brand != "SODEXO" => st
            .iter()
            .filter(|gw| !amex_not_supported_gateways.contains(*gw))
            .filter(|gw| !sodexo_only_gateways.contains(*gw))
            .cloned()
            .collect(),

        _ => st
            .iter()
            .filter(|gw| !amex_not_supported_gateways.contains(*gw))
            .cloned()
            .collect(),
    }
}

/// Filters gateways based on authentication type and gateway eligibility
pub async fn filterGatewaysForAuthType(
    this: &mut DeciderFlow<'_>,
) -> Result<Vec<String>, ErrorResponse> {
    // Get the current list of gateways and relevant transaction data
    let st = getGws(this);
    let mga_list = Utils::get_mgas(this).unwrap_or_default();
    let txn_detail = this.get().dpTxnDetail.clone();
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let macc = this.get().dpMerchantAccount.clone();
    let dynamic_mga_enabled = Utils::get_is_merchant_enabled_for_dynamic_mga_selection(this).await;
    // Only proceed with filtering if card ISIN is available
    if let Some(ref card_isin) = txn_card_info.card_isin {
        // Filter for OTP authentication type
        if txn_card_info
            .authType
            .as_ref()
            .map(|at| *at == AuthType::OTP)
            .unwrap_or(false)
        {
            setGwsAndMgas(
                this,
                mga_list
                    .clone()
                    .into_iter()
                    .filter(|mga| {
                        Utils::check_if_enabled_in_mga(mga, "CARD_DOTP", "cardDirectOtpEnabled")
                            && st.contains(&mga.gateway)
                    })
                    .collect(),
            );
        }

        // Filter for MOTO authentication type
        if txn_card_info
            .authType
            .as_ref()
            .map(|at| *at == AuthType::MOTO)
            .unwrap_or(false)
        {
            setGwsAndMgas(
                this,
                mga_list
                    .clone()
                    .into_iter()
                    .filter(|mga| {
                        Utils::check_if_enabled_in_mga(mga, "CARD_MOTO", "cardMotoEnabled")
                            && st.contains(&mga.gateway)
                    })
                    .collect(),
            );
        }

        // Filter for NO_THREE_DS authentication type
        if txn_card_info
            .authType
            .as_ref()
            .map(|at| *at == AuthType::NO_THREE_DS)
            .unwrap_or(false)
        {
            setGwsAndMgas(
                this,
                mga_list
                    .clone()
                    .into_iter()
                    .filter(|mga| {
                        Utils::check_if_no_ds_enabled_in_mga(mga, "CARD_NO_3DS", "cardNo3DsEnabled")
                            && st.contains(&mga.gateway)
                    })
                    .collect(),
            );
        }

        // Filter for VIES authentication type
        if txn_card_info
            .authType
            .as_ref()
            .map(|at| *at == AuthType::VIES)
            .unwrap_or(false)
        {
            setGwsAndMgas(
                this,
                mga_list
                    .clone()
                    .into_iter()
                    .filter(|mga| SETMA::is_vies_enabled(mga) && st.contains(&mga.gateway))
                    .collect(),
            );
        }

        // Check if BIN eligibility check is disabled via feature flag
        let mb_feature = ETF::get_feature_enabled(
            "DISABLE_DECIDER_BIN_ELIGIBILITY_CHECK",
            &macc.merchantId,
            true,
        )
        .await;

        logger::debug!(
            action = "filterFunctionalGatewaysForAuthType",
            "BIN eligibility check feature flag: {:?}",
            mb_feature
        );

        // Apply additional filtering if BIN eligibility check is not disabled
        if mb_feature.is_none() {
            let stt = getGws(this);

            // Get gateway restrictions and capabilities from Redis
            let atm_pin_card_info_restricted_gateways =
                findByNameFromRedis(C::ATM_PIN_CARD_INFO_RESTRICTED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_else(Vec::new)
                    .into_iter()
                    .collect::<HashSet<_>>();

            let otp_card_info_restricted_gateways =
                findByNameFromRedis(C::OTP_CARD_INFO_RESTRICTED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_else(Vec::new)
                    .into_iter()
                    .collect::<HashSet<_>>();

            let otp_card_info_supported_gateways =
                findByNameFromRedis(C::OTP_CARD_INFO_SUPPORTED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_else(Vec::new)
                    .into_iter()
                    .collect::<HashSet<_>>();

            let moto_card_info_supported_gateways =
                findByNameFromRedis(C::MOTO_CARD_INFO_SUPPORTED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_else(Vec::new)
                    .into_iter()
                    .collect::<HashSet<_>>();

            let auth_type_restricted_gateways =
                findByNameFromRedis(C::AUTH_TYPE_RESTRICTED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_else(Vec::new)
                    .into_iter()
                    .collect::<HashMap<AuthType, Vec<String>>>();

            // Partition gateways based on whether they need card info check
            let (card_info_check_needed_gateways, card_info_check_not_needed_gateways): (
                Vec<_>,
                Vec<_>,
            ) = stt.into_iter().partition(|gateway| {
                isGatewayCardInfoCheckNeeded(
                    &txn_card_info,
                    &atm_pin_card_info_restricted_gateways,
                    &otp_card_info_supported_gateways,
                    &moto_card_info_supported_gateways,
                    gateway,
                )
            });

            // Filter gateways that don't need card info check but still need auth type compatibility
            let auth_type_supported_gws: Vec<_> = card_info_check_not_needed_gateways
                .into_iter()
                .filter(|gw| {
                    isAuthTypeSupportedGateway(
                        &txn_card_info,
                        &atm_pin_card_info_restricted_gateways,
                        &otp_card_info_restricted_gateways,
                        &auth_type_restricted_gateways,
                        gw,
                    )
                })
                .collect();

            // Check auth type support for gateways that need card info check
            let gci_validated_gws = isAuthTypeSupported(
                this,
                macc,
                card_isin.to_string(),
                card_info_check_needed_gateways,
                txn_card_info.authType,
            )
            .await?;

            logger::debug!(
                tag = "filterFunctionalGatewaysForAuthType",
                action = "filterFunctionalGatewaysForAuthType",
                "Functional gateways after filtering after DISABLE_DECIDER_BIN_ELIGIBILITY_CHECK check: {:?}: {:?}",
                txn_detail.txnId,
                gci_validated_gws
                    .iter()
                    .chain(auth_type_supported_gws.iter())
                    .collect::<Vec<_>>()
            );

            // Update functional gateways with combined filtered lists
            setGws(
                this,
                gci_validated_gws
                    .into_iter()
                    .chain(auth_type_supported_gws.into_iter())
                    .collect(),
            );
        }
    }

    // Return the final gateway list with logging
    Ok(returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForAuthType,
        true,
    ))
}

/// Determines if a gateway needs specific card info checks based on auth type
/// Returns true if the gateway should go through additional card info validation
fn isGatewayCardInfoCheckNeeded(
    txn_card_info: &TxnCardInfo,
    atm_pin_card_info_restricted_gateways: &HashSet<String>,
    otp_card_info_supported_gateways: &HashSet<String>,
    moto_card_info_supported_gateways: &HashSet<String>,
    gateway: &String,
) -> bool {
    // Check for ATM PIN auth type and if the gateway is in the restricted list
    txn_card_info
        .authType
        .as_ref()
        .map(|at| *at == AuthType::ATMPIN)
        .unwrap_or(false)
        && atm_pin_card_info_restricted_gateways.contains(gateway)
        ||
        // Check for OTP auth type and if the gateway supports it
        txn_card_info
            .authType
            .as_ref()
            .map(|at| *at == AuthType::OTP)
            .unwrap_or(false)
            && otp_card_info_supported_gateways.contains(gateway)
        ||
        // Check for MOTO auth type and if the gateway supports it
        txn_card_info
            .authType
            .as_ref()
            .map(|at| *at == AuthType::MOTO)
            .unwrap_or(false)
            && moto_card_info_supported_gateways.contains(gateway)
}

fn isAuthTypeSupportedGateway(
    txn_card_info: &TxnCardInfo,
    atm_pin_card_info_restricted_gateways: &HashSet<String>,
    otp_card_info_restricted_gateways: &HashSet<String>,
    auth_type_restricted_gateways: &HashMap<AuthType, Vec<String>>,
    gateway: &String,
) -> bool {
    // First try to get gateways for this auth type from the restricted map
    txn_card_info
        .authType
        .as_ref()
        .and_then(|auth_type| auth_type_restricted_gateways.get(auth_type))
        .map(|gws| gws.contains(gateway))
        .unwrap_or_else(|| {
            // Check if auth type is VIES (special case that's always allowed)
            (txn_card_info
                .authType
                .as_ref()
                .map(|at| *at == AuthType::VIES)
                .unwrap_or(false))
                || !(txn_card_info
                    .authType
                    .as_ref()
                    .map(|auth_type| {
                        *auth_type != AuthType::ATMPIN
                            && atm_pin_card_info_restricted_gateways.contains(gateway)
                    })
                    .unwrap_or(false))
                    && !(txn_card_info
                        .authType
                        .as_ref()
                        .map(|auth_type| {
                            *auth_type != AuthType::OTP
                                && otp_card_info_restricted_gateways.contains(gateway)
                        })
                        .unwrap_or(false))
        })
}

pub async fn isAuthTypeSupported(
    this: &mut DeciderFlow<'_>,
    ma: ETM::merchant_account::MerchantAccount,
    cardbin: String,
    gws: Vec<String>,
    mauth: Option<AuthType>,
) -> Result<Vec<String>, ErrorResponse> {
    // Get the bin list for the provided card bin
    let bin_list = Utils::get_bin_list(Some(cardbin));

    // let mauth: Option<AuthType> = mauthS.map(|auth| auth.peek().clone());

    // Filter gateway card info based on merchant account, bin, and auth type
    let enabled_gcis = filterGatewaysCardInfo(this, ma, bin_list, gws, mauth, None).await?;

    // Extract just the gateways from the gateway card info objects
    Ok(enabled_gcis
        .into_iter()
        .filter_map(|gci| gci.gateway)
        .collect())
}

/// Filters gateways for One-Time Mandate payment flow
/// Identifies eligible gateways that support OTM based on bank codes and payment method compatibility
pub async fn filterFunctionalGatewaysForOTMFlow(this: &mut DeciderFlow<'_>) -> Vec<String> {
    // Get current functional gateways
    let st = getGws(this);

    // Get transaction data from context
    let txn_detail = this.get().dpTxnDetail.clone();
    let macc = this.get().dpMerchantAccount.clone();
    let order_reference = this.get().dpOrder.clone();
    let txn_card_info = this.get().dpTxnCardInfo.clone();

    // Get merchant gateway accounts
    let m_mgas = Utils::get_mgas(this);

    // Check if this is a One-Time Mandate flow
    let payment_flow_list = Utils::get_payment_flow_list_from_txn_detail(&txn_detail);
    let is_otm_flow = payment_flow_list.contains(&"ONE_TIME_MANDATE".to_string());
    let internal_tracking_info = txn_detail.internalTrackingInfo.clone();

    if is_otm_flow {
        // Get order metadata and ref IDs
        let (metadata, pl_ref_id_map) = Utils::get_order_metadata_and_pl_ref_id_map(
            this,
            macc.enableGatewayReferenceIdBasedRouting,
            &order_reference,
        );

        // Get all possible merchant reference IDs
        let possible_ref_ids_of_merchant =
            Utils::get_all_possible_ref_ids(metadata, order_reference, pl_ref_id_map);

        // Get merchant gateway accounts that support OTM
        let mgas = SETMA::get_enabled_mgas_by_merchant_id_and_ref_id(
            this,
            macc.merchantId,
            possible_ref_ids_of_merchant,
        )
        .await
        .into_iter()
        .filter(Utils::is_otm_enabled)
        .collect::<Vec<_>>();

        // Filter MGAs to only include those with gateways in our allowed list
        let eligible_mga_post_filtering = mgas
            .into_iter()
            .filter(|mga| st.contains(&mga.gateway))
            .collect::<Vec<_>>();

        // Extract just the gateway list from filtered MGAs
        let gw_list = eligible_mga_post_filtering
            .iter()
            .map(|x| x.gateway.clone())
            .collect::<Vec<_>>();

        // If we have a valid bank code, do additional filtering based on payment method flow
        if let Some(jbc) = find_bank_code(txn_card_info.paymentMethod).await {
            // Find all gateway payment method flows for OTM with this bank code
            let all_gpmf_entries = GPMF::find_all_gpmf_by_country_code_gw_pf_id_pmt_jbcid_db(
                crate::types::country::country_iso::CountryISO::IND,
                gw_list,
                PaymentFlow::ONE_TIME_MANDATE,
                txn_card_info.paymentMethodType,
                jbc.id,
            )
            .await
            .unwrap_or_default();

            // Extract MGA IDs and GPMF IDs for further filtering
            let mga_ids = eligible_mga_post_filtering
                .iter()
                .map(|mga| mga.id.merchantGwAccId)
                .collect::<Vec<_>>();

            let gpmf_ids: Vec<GatewayPaymentMethodFlowId> = all_gpmf_entries
                .iter()
                .map(|entry| to_gateway_payment_method_flow_id(entry.id.clone()))
                .collect();

            // Get merchant gateway payment method flows that match both MGA and GPMF
            let mgpmf_entries =
                MGPMF::get_all_mgpmf_by_mga_id_and_gpmf_ids(mga_ids, gpmf_ids).await;

            // Extract merchant gateway account IDs that have matching payment method flows
            let mgpmf_mga_id_entries = mgpmf_entries
                .iter()
                .map(|entry| entry.merchantGatewayAccountId)
                .collect::<Vec<_>>();

            // Final filtering of MGAs to only those with matching payment flows
            let eligible_mga_post_filtering_otm = eligible_mga_post_filtering
                .into_iter()
                .filter(|mga| mgpmf_mga_id_entries.contains(&mga.id.merchantGwAccId))
                .collect::<Vec<_>>();

            // Extract final gateway list from filtered MGAs
            let gw_list_post_otm_filtering = eligible_mga_post_filtering_otm
                .iter()
                .map(|x| x.gateway.clone())
                .collect::<Vec<_>>();

            // Update state with final MGA and gateway lists
            Utils::set_mgas(this, eligible_mga_post_filtering_otm);
            setGws(this, gw_list_post_otm_filtering);
        } else {
            // If no bank code found, keep original gateway list
            setGws(this, st);
        }
    } else {
        // If not OTM flow, keep original gateway list
        setGws(this, st);
    }

    // Return gateway list with logging
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForOTM,
        true,
    )
}

/// Filters gateways based on transaction validation type (Card Mandate, TPV, E-Mandate)
pub async fn filterGatewaysForValidationType(
    this: &mut DeciderFlow<'_>,
) -> Result<Vec<String>, ErrorResponse> {
    // Get current gateways and transaction details
    let st = getGws(this);
    let txn_detail = this.get().dpTxnDetail.clone();
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let macc = this.get().dpMerchantAccount.clone();
    let order_reference = this.get().dpOrder.clone();

    // Get order metadata and reference IDs
    let (metadata, pl_ref_id_map) = Utils::get_order_metadata_and_pl_ref_id_map(
        this,
        macc.enableGatewayReferenceIdBasedRouting,
        &order_reference,
    );
    let possible_ref_ids_of_merchant = Utils::get_all_possible_ref_ids(
        metadata.clone(),
        order_reference.clone(),
        pl_ref_id_map.clone(),
    );

    // Handle Card Mandate transactions
    if Utils::is_mandate_transaction(&txn_detail) && Utils::is_card_transaction(&txn_card_info) {
        // Get excluded gateways from Redis
        let uniqueGwLs: Vec<String> = st.clone().into_iter().collect();
        let brand = txn_card_info
            .cardSwitchProvider
            .as_ref()
            .map(|provider| provider.peek().to_string())
            .unwrap_or_else(|| "DEFAULT".to_string());
        let mPmEntryDB = ETP::get_by_name(brand).await;

        let updatedSt = if let Some(cardPaymentMethod) = mPmEntryDB {
            let uniqueGwLs: Vec<String> = st.into_iter().collect();
            let allGPMfEntries = GPMF::find_all_gpmf_by_gateway_payment_flow_payment_method(
                uniqueGwLs.clone(),
                cardPaymentMethod.id,
                PaymentFlow::CVVLESS,
            )
            .await;
            let mgaList = Utils::get_mgas(this).unwrap_or_default();
            let gmpfGws: Vec<String> = allGPMfEntries
                .iter()
                .map(|gpmf| gpmf.gateway.clone())
                .collect();
            let filteredMga: Vec<MerchantGatewayAccount> = mgaList
                .into_iter()
                .filter(|mga| gmpfGws.contains(&mga.gateway))
                .collect();
            let mgaIds: Vec<i64> = filteredMga
                .iter()
                .map(|mga| mga.id.merchantGwAccId)
                .collect();
            let gpmfIds: Vec<GatewayPaymentMethodFlowId> =
                allGPMfEntries.iter().map(|gpmf| gpmf.id.clone()).collect();
            let mgpmfEntries = MGPMF::get_all_mgpmf_by_mga_id_and_gpmf_ids(mgaIds, gpmfIds).await;
            let filteredMgaList: Vec<i64> = mgpmfEntries
                .iter()
                .map(|mgpmf| mgpmf.merchantGatewayAccountId)
                .collect();
            let finalFilteredMga: Vec<MerchantGatewayAccount> = filteredMga
                .into_iter()
                .filter(|mga| filteredMgaList.contains(&mga.id.merchantGwAccId))
                .collect();
            Utils::set_mgas(this, finalFilteredMga.clone());
            finalFilteredMga
                .into_iter()
                .map(|mga| mga.gateway)
                .collect()
        } else {
            Vec::new()
        };

        let card_mandate_bin_filter_excluded_gateways =
            findByNameFromRedis(C::CARD_MANDATE_BIN_FILTER_EXCLUDED_GATEWAYS.get_key())
                .await
                .unwrap_or_else(Vec::new);
        let bin_wise_filter_excluded_gateways =
            intersect(&card_mandate_bin_filter_excluded_gateways, &updatedSt);
        let bin_list = Utils::get_bin_list(txn_card_info.card_isin.clone());

        // Filter gateways based on card info
        let m_new_gateways_ = filterGatewaysCardInfo(
            this,
            macc.clone(),
            bin_list,
            updatedSt,
            txn_card_info.authType.clone(),
            Some(ETGCI::ValidationType::CardMandate),
        )
        .await?;

        let m_new_gateways = m_new_gateways_
            .into_iter()
            .map(|g| g.gateway)
            .collect::<Vec<_>>();

        logger::debug!(
            tag = "filterFunctionalGateways",
            action = "filterFunctionalGateways",
            "Functional gateways after filtering after filterGatewaysCardInfo for txn_id {:?}: {:?}",
            txn_detail.txnId, m_new_gateways
        );

        // Clean gateway list and combine with excluded list
        let new_gateways = ord_nub(catMaybes(&m_new_gateways));
        let m_internal_meta = Utils::get_internal_meta_data(this);
        let new_gws = [&new_gateways[..], &bin_wise_filter_excluded_gateways[..]].concat();
        setGws(this, new_gws.clone());

        // Handle token repeat transactions
        if Utils::is_token_repeat_txn(m_internal_meta.clone()) {
            let m_token_repeat_mandate_supported_gateways = Utils::get_token_supported_gateways(
                txn_detail.clone(),
                txn_card_info.clone(),
                "MANDATE".to_string(),
                m_internal_meta.clone(),
            )
            .await;

            let gws = match m_token_repeat_mandate_supported_gateways {
                Some(token_repeat_mandate_supported_gateways) => new_gws
                    .iter()
                    .filter(|x| token_repeat_mandate_supported_gateways.contains(x))
                    .cloned()
                    .collect::<Vec<_>>(),
                None => new_gws.clone(),
            };

            let final_gws = if gws.is_empty() { new_gws.clone() } else { gws };

            logger::debug!(
                tag = "filterFunctionalGateways",
                action = "filterFunctionalGateways",
                "Functional gateways after filtering for token repeat Mandate support for txn_id {:?}: {:?}",
                txn_detail.txnId, final_gws
            );

            setGws(this, final_gws);
        }

        // Handle non-express checkout, non-token repeat transactions
        if !txn_detail.expressCheckout && !Utils::is_token_repeat_txn(m_internal_meta) {
            let m_mandate_guest_checkout_supported_gateways: Option<Vec<String>> =
                findByNameFromRedis(
                    C::getmandateGuestCheckoutKey(txn_card_info.cardSwitchProvider).get_key(),
                )
                .await;

            let gws = match m_mandate_guest_checkout_supported_gateways {
                Some(mandate_guest_checkout_supported_gateways) => new_gws
                    .iter()
                    .filter(|x| mandate_guest_checkout_supported_gateways.contains(x))
                    .cloned()
                    .collect::<Vec<_>>(),
                None => new_gws.clone(),
            };

            let final_gws = if gws.is_empty() { new_gws.clone() } else { gws };

            logger::debug!(
                tag = "filterFunctionalGateways",
                action = "filterFunctionalGateways",
                "Functional gateways after filtering for Mandate Guest Checkout support for txn_id {:?}: {:?}",
                txn_detail.txnId, final_gws
            );

            setGws(this, final_gws);
        }
    }
    // Handle TPV or E-Mandate transactions
    else if Utils::is_tpv_transaction(&txn_detail)
        || (Utils::is_emandate_transaction(&txn_detail)
            && Utils::is_emandate_supported_payment_method(&txn_card_info))
    {
        // Check if we should skip processing for one-time mandate flow
        let payment_flow_list = Utils::get_payment_flow_list_from_txn_detail(&txn_detail);
        let is_otm_flow = payment_flow_list.contains(&"ONE_TIME_MANDATE".to_string());

        if is_otm_flow {
            logger::info!(
                "Skipping processing for OTM flow for txn_id {:?}",
                txn_detail.txnId.clone()
            );
        } else
        // Filter enabled gateway accounts
        {
            // Determine validation type and get enabled MGAs
            let (validation_type, e_mgas) = if Utils::is_emandate_transaction(&txn_detail) {
                let e_mgas = SETMA::get_emandate_enabled_mga(
                    this,
                    macc.merchantId.clone(),
                    possible_ref_ids_of_merchant,
                )
                .await;

                let v_type = if Utils::is_tpv_mandate_transaction(&txn_detail) {
                    ETGCI::ValidationType::TpvEmandate
                } else {
                    ETGCI::ValidationType::Emandate
                };

                (v_type, e_mgas)
            } else {
                let e_mgas = ETMA::getEnabledMgasByMerchantIdAndRefId(
                    macc.merchantId.0.clone(),
                    possible_ref_ids_of_merchant
                        .into_iter()
                        .map(|s| s.mga_reference_id)
                        .collect(),
                )
                .await;

                (ETGCI::ValidationType::Tpv, e_mgas)
            };
            if matches!(
                validation_type,
                ETGCI::ValidationType::TpvEmandate | ETGCI::ValidationType::Emandate
            ) && is_otm_flow
            {
                return Ok(returnGwListWithLog(
                    this,
                    DeciderFilterName::FilterFunctionalGatewaysForValidationType,
                    true,
                ));
            }
            let enabled_gateway_accounts = e_mgas
                .into_iter()
                .filter(|mga| {
                    predicate(
                        this,
                        mga.clone(),
                        mga.gateway.clone(),
                        metadata.clone(),
                        order_reference.clone(),
                        pl_ref_id_map.clone(),
                    )
                })
                .collect::<Vec<_>>();

            let amount = Utils::effective_amount_with_txn_amount(txn_detail.clone()).await;

            // Filter gateways for payment method and validation type
            let merchant_gateway_card_infos =
                SETMCI::filter_gateways_for_payment_method_and_validation_type(
                    this.state(),
                    macc,
                    txn_card_info.clone(),
                    enabled_gateway_accounts.clone(),
                    validation_type,
                    transaction_id_to_text(txn_detail.txnId.clone()),
                )
                .await;

            // Apply maximum register amount filter
            let merchant_gateway_card_infos =
                Utils::filter_gateway_card_info_for_max_register_amount(
                    txn_detail.clone(),
                    txn_card_info,
                    merchant_gateway_card_infos,
                    amount,
                );

            // Extract gateway card info IDs
            let gci_ids = merchant_gateway_card_infos
                .iter()
                .map(|gci| gci.gatewayCardInfoId.clone())
                .collect::<Vec<_>>();

            // Get gateways by IDs
            let nst = ETGCI::get_all_by_mgci_ids(gci_ids)
                .await
                .into_iter()
                .map(|g| g.gateway)
                .collect::<Vec<_>>();

            logger::debug!(
                "nst for filterGatewaysForValidationType for txn_id {:?}: {:?}",
                txn_detail.txnId.clone(),
                nst
            );

            let new_st = catMaybes(&nst);

            // Update gateway list and MGAs
            setGwsAndMgas(
                this,
                enabled_gateway_accounts
                    .into_iter()
                    .filter(|mga| {
                        new_st.contains(&mga.gateway)
                            && merchant_gateway_card_infos
                                .iter()
                                .map(|gci| gci.merchantGatewayAccountId.clone())
                                .collect::<Vec<_>>()
                                .contains(&Some(mga.id.clone()))
                    })
                    .collect(),
            );
        }
    }
    // Handle other transaction types
    else {
        let tpv_only_supported_gateways =
            findByNameFromRedis(C::TPV_ONLY_SUPPORTED_GATEWAYS.get_key())
                .await
                .unwrap_or_else(Vec::new);

        if !tpv_only_supported_gateways.is_empty()
            && !intersect(&tpv_only_supported_gateways, &st).is_empty()
        {
            // Group MGAs by gateway
            // let group_into_map = |proj: fn(&ETM::merchant_gateway_account::MerchantGatewayAccount) -> ETG::Gateway| async move {
            //     SETMA::get_tpv_only_gateway_accounts(this, possible_ref_ids_of_merchant.clone())
            //         .await
            //         .into_iter()
            //         .fold(HashMap::new(), |mut acc, mga| {
            //             acc.entry(proj(&mga))
            //                 .or_insert_with(Vec::new)
            //                 .push(mga);
            //             acc
            //         })
            // };

            let tpv_only_mgas = group_into_map(this, possible_ref_ids_of_merchant.clone(), |mga| {
                mga.gateway.clone()
            })
            .await;
            let all_mgas = group_into_map(this, possible_ref_ids_of_merchant.clone(), |mga| {
                mga.gateway.clone()
            })
            .await;

            // Find gateways to be removed
            let gateways_to_be_removed = tpv_only_mgas
                .iter()
                .filter(|(k, v)| all_mgas.get(*k).is_some_and(|a| a.len() == v.len()))
                .map(|(k, _)| k.clone())
                .collect::<Vec<_>>();

            // Filter out gateways and update
            setGws(
                this,
                st.iter()
                    .filter(|g| !gateways_to_be_removed.contains(g))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
        }
    }

    // Log and return the final gateway list
    Ok(returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForValidationType,
        true,
    ))
}

/// Groups merchant gateway accounts by a gateway key
/// Returns a HashMap where keys are gateways and values are vectors of MGAs with that gateway
pub async fn group_into_map<F>(
    this: &mut DeciderFlow<'_>,
    possible_ref_ids_of_merchant: Vec<ETMA::MgaReferenceId>,
    proj: F,
) -> HashMap<String, Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>>
where
    F: Fn(&ETM::merchant_gateway_account::MerchantGatewayAccount) -> String,
{
    SETMA::get_tpv_only_gateway_accounts(this, possible_ref_ids_of_merchant)
        .await
        .into_iter()
        .fold(HashMap::new(), |mut acc, mga| {
            acc.entry(proj(&mga)).or_default().push(mga);
            acc
        })
}

/// Determines if a merchant gateway account matches the provided gateway reference ID
/// Used for gateway reference ID based routing
pub fn predicate(
    this: &mut DeciderFlow,
    mga: ETM::merchant_gateway_account::MerchantGatewayAccount,
    gw: String,
    metadata: HashMap<String, String>,
    order_ref: Order,
    pl_ref_id_map: HashMap<String, String>,
) -> bool {
    // Get gateway reference ID from the metadata, gateway, order, and reference ID map
    let gw_ref_id = Utils::get_gateway_reference_id(metadata, &gw, order_ref, pl_ref_id_map);

    // Check if merchant gateway account's reference ID matches
    mga.referenceId == gw_ref_id
}
pub async fn filterGatewaysCardInfo(
    this: &mut DeciderFlow<'_>,
    merchant_account: MerchantAccount,
    card_bins: Vec<Option<String>>,
    enabled_gateways: Vec<String>,
    m_auth_type: Option<AuthType>,
    m_validation_type: Option<ETGCI::ValidationType>,
) -> Result<Vec<GatewayCardInfo>, ErrorResponse> {
    let appState = this.state().clone();
    if !enabled_gateways.is_empty()
        && card_bins.iter().all(|bin| bin.is_some())
        && (m_auth_type.is_some() || m_validation_type.is_some())
    {
        if m_validation_type
            .clone()
            .map(|vt| vt == ETGCI::ValidationType::CardMandate)
            .unwrap_or(false)
        {
            let merchant_wise_mandate_supported_gateway: Vec<String> =
                Utils::get_merchant_wise_mandate_bin_eligible_gateways(
                    &merchant_account.clone(),
                    &enabled_gateways,
                )
                .await;
            let merchant_wise_mandate_supported_gateway_prime: Vec<Option<String>> =
                merchant_wise_mandate_supported_gateway
                    .iter()
                    .map(|g| Some(g.clone()))
                    .collect();

            let merchant_wise_eligible_gateway_card_info = if !enabled_gateways.is_empty() {
                let supported_gci = ETGCIS::getSupportedGatewayCardInfoForBins(
                    &appState,
                    merchant_account,
                    card_bins.clone(),
                )
                .await?;

                supported_gci
                    .into_iter()
                    .filter(|ci| {
                        ci.gateway.is_some()
                            && merchant_wise_mandate_supported_gateway_prime.contains(&ci.gateway)
                            && ci.validationType == Some(ETGCI::ValidationType::CardMandate)
                            && ci
                                .authType
                                .clone()
                                .unwrap_or_else(|| "THREE_DS".to_string())
                                == auth_type_to_text(
                                    &m_auth_type.clone().unwrap_or(AuthType::THREE_DS),
                                )
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![]
            };

            let filtered_gateways: Vec<String> = enabled_gateways
                .iter()
                .filter(|g| !merchant_wise_mandate_supported_gateway.contains(g))
                .cloned()
                .collect();

            let eligible_gateway_card_info_prime =
                ETGCI::get_enabled_gateway_card_info_for_gateways(card_bins, filtered_gateways)
                    .await
                    .into_iter()
                    .filter(|ci| {
                        (ci.validationType == Some(ETGCI::ValidationType::CardMandate)
                            && ci
                                .authType
                                .clone()
                                .unwrap_or_else(|| "THREE_DS".to_string())
                                == auth_type_to_text(
                                    &m_auth_type.clone().unwrap_or(AuthType::THREE_DS),
                                ))
                    })
                    .collect::<Vec<GatewayCardInfo>>();

            Ok(eligible_gateway_card_info_prime
                .into_iter()
                .chain(merchant_wise_eligible_gateway_card_info.into_iter())
                .collect::<Vec<_>>())
        } else {
            let mut merchant_validation_required_gws = Vec::new();
            let mut gci_validation_gws = Vec::new();

            for gw in enabled_gateways.iter().cloned() {
                if Utils::is_merchant_wise_auth_type_check_needed(
                    &merchant_account,
                    m_auth_type.as_ref(),
                    m_validation_type.as_ref(),
                    &gw,
                )
                .await
                {
                    merchant_validation_required_gws.push(gw);
                } else {
                    gci_validation_gws.push(gw);
                }
            }

            let gcis =
                ETGCI::get_enabled_gateway_card_info_for_gateways(card_bins, enabled_gateways)
                    .await;

            let gcis_without_merchant_validation = gcis
                .iter()
                .filter(|gci| {
                    gci_validation_gws.contains(&gci.gateway.clone().unwrap_or("none".to_string()))
                        && !merchant_validation_required_gws
                            .contains(&gci.gateway.clone().unwrap_or("none".to_string()))
                })
                .cloned()
                .collect::<Vec<_>>();

            let gcis_with_merchant_validation = gcis
                .iter()
                .filter(|gci| {
                    merchant_validation_required_gws
                        .contains(&gci.gateway.clone().unwrap_or("none".to_string()))
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
                ETMGCI::find_all_mgcis_by_macc_and_gci_p_id(merchant_account.id, gci_ids.clone())
                    .await
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
                .iter()
                .cloned()
                .chain(gcis_after_merchant_validation.iter().cloned())
                .collect::<Vec<_>>();

            let eligible_gateway_card_info_prime = match m_validation_type {
                Some(ref v_type) => eligible_gateway_card_infos
                    .clone()
                    .into_iter()
                    .filter(|ci| {
                        ci.validationType.clone().map(|v| v.to_string()) == Some(v_type.to_string())
                    })
                    .collect::<Vec<_>>(),
                None => eligible_gateway_card_infos
                    .clone()
                    .into_iter()
                    .filter(|ci| ci.authType == m_auth_type.as_ref().map(|auth| auth.to_string()))
                    .collect::<Vec<_>>(),
            };

            logger::info!(
                tag = "filterGatewaysCardInfo",
                action = "filterGatewaysCardInfo",
                "merchant_validation_required_gws - {:?}, gci_validation_gws - {:?}, gcis - {:?}, gcis_without_merchant_validation - {:?}, gcis_with_merchant_validation - {:?}, gci_ids - {:?}, mgcis_enabled_gcis - {:?},mgcis_enabled_gci_ids - {:?}, gcis_after_merchant_validation - {:?}, eligible_gateway_card_infos - {:?}",
                merchant_validation_required_gws,
                gci_validation_gws,
                gcis,
                gcis_without_merchant_validation,
                gcis_with_merchant_validation,
                gci_ids,
                mgcis_enabled_gcis,
                mgcis_enabled_gci_ids,
                gcis_after_merchant_validation,
                eligible_gateway_card_infos,
            );

            logger::debug!(
                "gcisWithoutMerchantValidation {:?}",
                gcis_without_merchant_validation.clone()
            );
            logger::debug!(
                "gcisWithMerchantValidation {:?}",
                gcis_after_merchant_validation.clone()
            );
            logger::debug!(
                "eligibleGatewayCardInfo {:?}",
                eligible_gateway_card_info_prime
            );

            Ok(eligible_gateway_card_info_prime)
        }
    } else {
        Ok(vec![])
    }
}

/// Filters gateways based on transaction offer details
/// If transaction has offer details, applies gateway rules based on offer specifics
pub async fn filterGatewaysForTxnOfferDetails(this: &mut DeciderFlow<'_>) -> Vec<String> {
    // Get current functional gateways
    let functional_gateways = getGws(this);

    // Get transaction offer details and transaction detail from context
    let txn_offer_details = this.get().dpTxnOfferDetails.clone();
    let txn_detail = this.get().dpTxnDetail.clone();

    match txn_offer_details {
        Some(txn_offer_details) => {
            // Apply gateway rules for each transaction offer detail
            let mut filtered_gws = functional_gateways.clone();
            for txn_offer_detail in txn_offer_details.iter() {
                filtered_gws =
                    filterByGatewayRule(this, &txn_detail, filtered_gws, txn_offer_detail).await;
            }

            // Update functional gateways if the list has changed
            if functional_gateways.len() != filtered_gws.len() {
                setGws(this, filtered_gws.clone());
            }

            // Return gateway list with logging
            returnGwListWithLog(
                this,
                DeciderFilterName::FilterFunctionalGatewaysForTxnOfferDetails,
                true,
            )
        }
        // If no offer details, just return the current gateway list with logging
        None => returnGwListWithLog(
            this,
            DeciderFilterName::FilterFunctionalGatewaysForTxnOfferDetails,
            true,
        ),
    }
}
/// Filters gateway list based on gateway routing rules defined in transaction offer details
/// If force_routing is enabled, only keeps gateways that appear in both the input list and the offer routing rules
pub async fn filterByGatewayRule(
    this: &mut DeciderFlow<'_>,
    txn_detail: &TxnDetail,
    gw_list_acc: Vec<String>,
    txn_offer_detail: &ETOD::TxnOfferDetail,
) -> Vec<String> {
    match &txn_offer_detail.gatewayInfo {
        // If no gateway info in offer, return original list
        None => gw_list_acc,

        // If gateway info exists, try to parse it
        Some(txt) => {
            match serde_json::from_str::<GatewayRule>(txt) {
                // If rule has force_routing enabled, apply intersection filter
                Ok(gateway_rule) if gateway_rule.force_routing.unwrap_or(false) => {
                    // Convert gateway names from rule to gateway objects
                    let txn_offer_details_gws: Vec<String> = gateway_rule
                        .gateway_info
                        .iter()
                        .map(|info| info.name.clone())
                        .collect();

                    // Create a HashSet from the offer gateways for efficient intersection
                    let offer_gw_set: HashSet<String> = txn_offer_details_gws.into_iter().collect();

                    // Return only gateways that appear in both lists
                    gw_list_acc
                        .into_iter()
                        .filter(|gw| offer_gw_set.contains(gw))
                        .collect()
                }

                // If parsing fails, log the error and return original list
                Err(err) => {
                    // Commented out log as requested
                    // debug!(
                    //     "For txn with id = {}, offerId = {}, parsing result is {:?}",
                    //     txn_detail.txnId,
                    //     txn_offer_detail.offerId,
                    //     err
                    // );
                    logger::debug!(
                        tag = "GatewayRuleParsingError",
                        action = "GatewayRuleParsingError",
                        "For txn with id = {:?}, offerId = {}, parsing result is {:?}",
                        txn_detail.txnId,
                        txn_offer_detail.offerId,
                        err
                    );

                    gw_list_acc
                }

                // If force_routing is not enabled, return original list
                _ => gw_list_acc,
            }
        }
    }
}
pub async fn filterGatewaysForEmi(this: &mut DeciderFlow<'_>) -> GatewayList {
    let functional_gateways = getGws(this);
    let merchant_acc = this.get().dpMerchantAccount.clone();
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let txn_detail = this.get().dpTxnDetail.clone();

    logger::debug!(
        tag = "GW_Filtering",
        action = "GW_Filtering",
        "For txn with id = {:?} isEmi = {:?}",
        txn_detail.txnId,
        txn_detail.isEmi
    );

    if txn_detail.isEmi {
        let is_mandate_txn = Utils::is_mandate_transaction(&txn_detail);
        let si_on_emi_card_supported_gateways: HashSet<String> =
            findByNameFromRedis::<HashSet<String>>(C::SI_ON_EMI_CARD_SUPPORTED_GATEWAYS.get_key())
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();

        let st = if is_mandate_txn {
            functional_gateways
                .into_iter()
                .filter(|gw| si_on_emi_card_supported_gateways.contains(gw))
                .collect()
        } else {
            functional_gateways
        };

        let st_filtered = if is_mandate_txn {
            let card_brand = Utils::get_card_brand(this).await;
            let si_on_emi_disabled_card_brand_gateway_mapping: HashMap<String, Vec<String>> =
                findByNameFromRedis::<HashMap<String, Vec<String>>>(
                    C::SI_ON_EMI_DISABLED_CARD_BRAND_GATEWAY_MAPPING.get_key(),
                )
                .await
                .unwrap_or_default();

            let card_brand_str = card_brand.as_deref().unwrap_or("");
            let disabled_gws = si_on_emi_disabled_card_brand_gateway_mapping
                .get(card_brand_str)
                .cloned()
                .unwrap_or_default();

            logger::debug!(
                "For txn with id = {:?}, Filtering out gateways: {:?} for card brand: {:?}",
                txn_detail.txnId,
                disabled_gws,
                card_brand.unwrap_or_else(|| "UNKNOWN".to_string())
            );

            st.into_iter()
                .filter(|gw| !disabled_gws.contains(gw))
                .collect()
        } else {
            st
        };

        let gws = if Utils::check_no_or_low_cost_emi(&txn_card_info) {
            let no_or_low_cost_emi_supported_gateways: HashSet<String> =
                findByNameFromRedis::<HashSet<String>>(
                    C::NO_OR_LOW_COST_EMI_SUPPORTED_GATEWAYS.get_key(),
                )
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();
            st_filtered
                .into_iter()
                .filter(|gw| no_or_low_cost_emi_supported_gateways.contains(gw))
                .collect()
        } else {
            st_filtered
        };

        let juspay_bank_code = Utils::get_juspay_bank_code_from_internal_metadata(&txn_detail);
        let gws = if Utils::is_card_transaction(&txn_card_info) && !gws.is_empty() {
            if Utils::check_if_bin_is_eligible_for_emi(
                txn_card_info.card_isin.clone(),
                juspay_bank_code,
                txn_card_info
                    .card_type
                    .as_ref()
                    .map(ETCA::card_type_to_text),
            )
            .await
            {
                gws
            } else {
                vec![]
            }
        } else {
            gws
        };

        let m_internal_meta = Utils::get_internal_meta_data(this);
        let scope_ = if Utils::is_card_transaction(&txn_card_info)
            && Utils::is_network_token_repeat_txn(m_internal_meta.clone())
        {
            "NETWORK_TOKEN"
        } else if Utils::is_card_transaction(&txn_card_info)
            && Utils::is_issuer_token_repeat_txn(m_internal_meta.clone())
        {
            "ISSUER_TOKEN"
        } else if Utils::is_card_transaction(&txn_card_info)
            && Utils::is_alt_id_based_txn(m_internal_meta)
        {
            "ALT_ID"
        } else if txn_detail
            .emiBank
            .as_deref()
            .is_some_and(|bank| bank.ends_with("_CLEMI"))
        {
            "CARDLESS"
        } else {
            "CARD"
        };

        let gws = if !gws.is_empty() {
            logger::debug!(
                tag = "filterGatewaysForEmi",
                action = "filterGatewaysForEmi",
                "filterGatewaysForEmi gateway list before getGatewayBankEmiSupport for txn_id :{:?}, where gateway is : {:?}",
                txn_detail.txnId,
                gws.clone()
            );

            let gbes_v2_flag = isFeatureEnabled(
                C::gbesV2Enabled.get_key(),
                merchant_acc.merchantId.0,
                "kv_redis".to_string(),
            )
            .await;
            if gbes_v2_flag {
                let gbes_v2_list_ = SGBES::getGatewayBankEmiSupportV2(
                    txn_detail.emiBank.clone(),
                    gws.clone(),
                    scope_.to_string(),
                    txn_detail.emiTenure,
                )
                .await;

                let gbes_v2_list = if scope_ == "ALT_ID" {
                    let emi_bank = txn_detail.emiBank.clone();
                    let mut gbesV2List = Vec::new();
                    for gbes in gbes_v2_list_.clone() {
                        let is_enabled = if let Some(emi_bank) = emi_bank.clone() {
                            isFeatureEnabledByDimension(
                                C::altIdEnabledGatewayEmiBank.get_key(),
                                format!("{}::{}", gbes.gateway, emi_bank),
                            )
                            .await
                        } else {
                            false
                        };
                        if is_enabled {
                            gbesV2List.push(gbes);
                        }
                    }
                    gbesV2List.clone()
                } else {
                    gbes_v2_list_.clone()
                };

                if gbes_v2_list.is_empty() {
                    logger::info!(
                        tag = "GBESV2 Entry Not Found",
                        action = "GBESV2 Entry Not Found",
                        "GBESV2 Entry Not Found For emiBank - {:?}, gateways - {:?}, scope_ - {:?}, tenure - {:?}",
                        txn_detail.emiBank,
                        gws.clone(),
                        scope_,
                        txn_detail.emiTenure
                    );
                }
                let gbes_v2_filtered: Vec<GatewayBankEmiSupportV2> = gbes_v2_list
                    .into_iter()
                    .filter(|gbs| {
                        let mb_metadata: Option<GBESV2Metadata> = gbs
                            .metadata
                            .as_ref()
                            .and_then(|metadata| serde_json::from_str(metadata).ok());

                        match mb_metadata.and_then(|meta| meta.supported_networks) {
                            Some(supported_networks) => supported_networks
                                .iter()
                                .any(|network| txn_card_info.paymentMethod == network.to_string()),
                            None => true,
                        }
                    })
                    .collect();

                extractGatewaysV2(gbes_v2_filtered)
            } else {
                let gbes_list_ = SGBES::getGatewayBankEmiSupport(
                    txn_detail.emiBank.clone(),
                    gws.clone(),
                    scope_.to_string(),
                )
                .await;

                let gbes_list = if scope_ == "ALT_ID" {
                    let emi_bank = txn_detail.emiBank.clone();
                    let mut gbesList = Vec::new();
                    for gbes in gbes_list_.clone() {
                        let is_enabled = if let Some(emi_bank) = emi_bank.clone() {
                            isFeatureEnabledByDimension(
                                C::altIdEnabledGatewayEmiBank.get_key(),
                                format!("{}::{}", gbes.gateway, emi_bank),
                            )
                            .await
                        } else {
                            false
                        };
                        if is_enabled {
                            gbesList.push(gbes);
                        }
                    }
                    gbesList.clone()
                } else {
                    gbes_list_.clone()
                };
                extractGateways(gbes_list)
            }
        } else {
            gws
        };

        setGws(this, gws);
    } else if Utils::is_card_transaction(&txn_card_info) {
        let card_emi_explicit_gateways: HashSet<String> =
            findByNameFromRedis::<HashSet<String>>(C::CARD_EMI_EXPLICIT_GATEWAYS.get_key())
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();
        setGws(
            this,
            functional_gateways
                .into_iter()
                .filter(|gw| !card_emi_explicit_gateways.contains(gw))
                .collect(),
        );
    }

    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForEmi,
        true,
    )
}
fn extractGateways(gbes: Vec<GatewayBankEmiSupport>) -> GatewayList {
    gbes.into_iter().map(|gb| gb.gateway).collect()
}

fn extractGatewaysV2(gbes_v2: Vec<GatewayBankEmiSupportV2>) -> GatewayList {
    gbes_v2.into_iter().map(|gb| gb.gateway).collect()
}

pub async fn filterGatewaysForPaymentMethod(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let txn = this.get().dpTxnDetail.clone();
    let merchant_acc = this.get().dpMerchantAccount.clone();
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let oref = this.get().dpOrder.clone();

    let is_dynamic_mga_enabled =
        Utils::get_is_merchant_enabled_for_dynamic_mga_selection(this).await;
    let (metadata, pl_ref_id_map) = Utils::get_order_metadata_and_pl_ref_id_map(
        this,
        merchant_acc.enableGatewayReferenceIdBasedRouting,
        &oref,
    );

    let proceed_with_all_mgas = Utils::is_enabled_for_all_mgas(this);
    let mgas = if proceed_with_all_mgas {
        SETMA::get_all_enabled_mgas_by_merchant_id(merchant_acc.merchantId.clone()).await
    } else {
        let possible_ref_ids_of_merchant =
            Utils::get_all_possible_ref_ids(metadata, oref.clone(), pl_ref_id_map);
        SETMA::get_enabled_mgas_by_merchant_id_and_ref_id(
            this,
            merchant_acc.merchantId.clone(),
            possible_ref_ids_of_merchant,
        )
        .await
    };

    let eligible_mgas: Vec<_> = mgas
        .into_iter()
        .filter(|mga| st.contains(&mga.gateway))
        .collect();

    if st.is_empty()
        || Utils::is_emandate_transaction(&txn)
        || Utils::is_tpv_transaction(&txn)
        || Utils::is_emandate_payment_transaction(&txn)
    {
        logger::debug!(
            tag = "filterGatewaysForPaymentMethod",
            action = "filterGatewaysForPaymentMethod",
            "For txn: {:?}, Skipped",
            txn.txnId
        );
    } else if Utils::is_card_transaction(&txn_card_info) {
        let m_payment_method = Utils::get_card_brand(this).await;
        let maybe_payment_method = if txn_card_info.paymentMethod.is_empty() {
            m_payment_method
        } else {
            Some(txn_card_info.paymentMethod.clone())
        };

        if let Some(payment_method) = maybe_payment_method {
            let (rem, rem_mgas) = getGatewaysAcceptingPaymentMethod(
                &oref,
                &merchant_acc,
                &eligible_mgas,
                &st,
                &payment_method,
                proceed_with_all_mgas,
                is_dynamic_mga_enabled,
            )
            .await;

            logger::debug!(
                tag = "filterGatewaysForPaymentMethod",
                action = "filterGatewaysForPaymentMethod",
                "For txn: {:?}, Remaining gateways after getGatewaysAcceptingPaymentMethod: {:?}",
                txn.txnId,
                rem
            );

            setGwsAndMgas(this, rem_mgas);
        }
    } else {
        logger::debug!(
            tag = "filterGatewaysForPaymentMethod",
            action = "filterGatewaysForPaymentMethod",
            "For txn: {:?}, Not card transaction",
            txn.txnId
        );

        let pm = getPaymentMethodForNonCardTransaction(&txn_card_info);
        let v2_integration_not_supported_gateways: Vec<String> =
            findByNameFromRedis::<Vec<String>>(C::V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS.get_key())
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();

        // let v2_integration_not_supported_gateways_hashset: HashSet<Gateway> = v2_integration_not_supported_gateways.iter().cloned().collect();

        let upi_intent_not_supported_gateways: Vec<String> =
            findByNameFromRedis::<Vec<String>>(C::UPI_INTENT_NOT_SUPPORTED_GATEWAYS.get_key())
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();
        // let upi_intent_not_supported_gateways_hashset: HashSet<Gateway> = upi_intent_not_supported_gateways.iter().cloned().collect();

        let (st, filtered_mgas) = if pm == "UPI_PAY" || pm == "UPI_QR" {
            if !st.is_empty() && !is_disjoint(&v2_integration_not_supported_gateways, &st) {
                filterGatewaysForUpiPayBasedOnSupportedFlow(
                    this,
                    st,
                    eligible_mgas,
                    v2_integration_not_supported_gateways,
                    upi_intent_not_supported_gateways,
                )
            } else {
                (st, eligible_mgas)
            }
        } else {
            (st, eligible_mgas)
        };

        let (_, rem_mgas) = getGatewaysAcceptingPaymentMethod(
            &oref,
            &merchant_acc,
            &filtered_mgas,
            &st,
            &pm,
            proceed_with_all_mgas,
            is_dynamic_mga_enabled,
        )
        .await;

        setGwsAndMgas(this, rem_mgas);
    }
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForPaymentMethod,
        true,
    )
}

fn is_disjoint(gateways1: &Vec<String>, gateways2: &Vec<String>) -> bool {
    // Convert gateway names to sets for efficient comparison
    let set1: HashSet<String> = gateways1.iter().cloned().collect();
    let set2: HashSet<String> = gateways2.iter().cloned().collect();

    set1.is_disjoint(&set2)
}

async fn getGatewaysAcceptingPaymentMethod(
    oref: &Order,
    merchant_acc: &MerchantAccount,
    eligible_mgas: &[MerchantGatewayAccount],
    gateways: &GatewayList,
    payment_method: &str,
    proceed_with_all_mgas: bool,
    is_dynamic_mga_enabled: bool,
) -> (GatewayList, Vec<MerchantGatewayAccount>) {
    let filtered_mgas: Vec<_> = eligible_mgas
        .iter()
        .filter(|mga| {
            canAcceptPaymentMethod(mga, payment_method) && gateways.contains(&mga.gateway)
        })
        .cloned()
        .collect();

    let gateways: GatewayList = filtered_mgas
        .iter()
        .map(|mga| mga.gateway.clone())
        .collect();

    (gateways, filtered_mgas)
}

fn getPaymentMethodForNonCardTransaction(txn_card_info: &TxnCardInfo) -> String {
    if matches!(
        txn_card_info.paymentMethodType.as_str(),
        CONSUMER_FINANCE | UPI | REWARD | CASH
    ) {
        txn_card_info.paymentMethod.clone()
    } else {
        txn_card_info.cardIssuerBankName.clone().unwrap_or_default()
    }
}

fn canAcceptPaymentMethod(mga: &MerchantGatewayAccount, pm: &str) -> bool {
    if let Some(payment_methods_str) = &mga.paymentMethods {
        if let Some(payment_methods) =
            Utils::get_value::<AValue>("paymentMethods", payment_methods_str.as_str())
        {
            if let Some(arr) = payment_methods.as_array() {
                arr.iter().filter_map(|x| x.as_str()).any(|x| x == pm)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}

pub async fn filterGatewaysForTokenProvider(this: &mut DeciderFlow<'_>) -> GatewayList {
    let st = getGws(this);
    let vault = this.get().dpVaultProvider.clone();
    let txn_id = this.get().dpTxnDetail.txnId.clone();

    logger::debug!(
        tag = "filterGatewaysForTokenProvider",
        action = "filterGatewaysForTokenProvider",
        "Vault provider for txn {:?} = {:?}",
        txn_id,
        vault
    );
    match vault {
        None => returnGwListWithLog(
            this,
            DeciderFilterName::FilterFunctionalGatewaysForTokenProvider,
            false,
        ),
        Some(VaultProvider::Juspay) => returnGwListWithLog(
            this,
            DeciderFilterName::FilterFunctionalGatewaysForTokenProvider,
            true,
        ),
        Some(v) => {
            let token_provider_gateway_mapping =
                findByNameFromRedis::<HashMap<VaultProvider, String>>(
                    C::TOKEN_PROVIDER_GATEWAY_MAPPING.get_key(),
                )
                .await
                .unwrap_or_default();
            let new_st = st
                .into_iter()
                .filter(|gateway| {
                    token_provider_gateway_mapping
                        .iter()
                        .any(|mapping| mapping.0 == &v && mapping.1 == gateway)
                })
                .collect();
            setGws(this, new_st);
            returnGwListWithLog(
                this,
                DeciderFilterName::FilterFunctionalGatewaysForTokenProvider,
                true,
            )
        }
    }
}

pub async fn filterGatewaysForWallet(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let upi_only_gateways: HashSet<String> =
        findByNameFromRedis::<HashSet<String>>(C::UPI_ONLY_GATEWAYS.get_key())
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();
    let wallet_only_gateways: HashSet<String> =
        findByNameFromRedis::<HashSet<String>>(C::WALLET_ONLY_GATEWAYS.get_key())
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let wallet_also_gateways: HashSet<String> =
        findByNameFromRedis::<HashSet<String>>(C::WALLET_ALSO_GATEWAYS.get_key())
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let new_st = match txn_card_info.card_type {
        Some(ETCA::CardType::Wallet) => st
            .into_iter()
            .filter(|gateway| {
                wallet_only_gateways.contains(gateway)
                    || wallet_also_gateways.contains(gateway)
                    || (SUTC::is_google_pay_txn(txn_card_info.clone())
                        && upi_only_gateways.contains(gateway))
            })
            .collect::<Vec<_>>(),
        _ => st
            .into_iter()
            .filter(|gateway| !wallet_only_gateways.contains(gateway))
            .collect::<Vec<_>>(),
    };

    setGws(this, new_st);
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForWallet,
        true,
    )
}

pub async fn filterGatewaysForNbOnly(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    if txn_card_info.card_type != Some(ETCA::CardType::Nb) {
        let nb_only_gateways: Vec<String> =
            findByNameFromRedis::<Vec<String>>(C::NB_ONLY_GATEWAYS.get_key())
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();

        let nb_only_gateways_hashset = nb_only_gateways.into_iter().collect::<HashSet<_>>();
        let new_st = st
            .into_iter()
            .filter(|gateway| !nb_only_gateways_hashset.contains(gateway))
            .collect::<Vec<_>>();
        setGws(this, new_st);
    }

    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForNbOnly,
        true,
    )
}

pub async fn filterFunctionalGatewaysForMerchantRequiredFlow(
    this: &mut DeciderFlow<'_>,
) -> GatewayList {
    let st = getGws(this);
    let txn_detail = this.get().dpTxnDetail.clone();
    let payment_flow_list = Utils::get_payment_flow_list_from_txn_detail(&txn_detail);

    let is_mf_order = payment_flow_list.contains(&"MUTUAL_FUND".to_string());
    let is_cb_order = payment_flow_list.contains(&"CROSS_BORDER_PAYMENT".to_string());
    let is_sbmd = payment_flow_list.contains(&"SINGLE_BLOCK_MULTIPLE_DEBIT".to_string());

    let mf_filtered_gw = filter_gateways_for_flow(
        is_mf_order,
        C::MUTUAL_FUND_FLOW_SUPPORTED_GATEWAYS.get_key(),
        st,
    )
    .await;
    let mf_and_cb_filtered_gw = filter_gateways_for_flow(
        is_cb_order,
        C::CROSS_BORDER_FLOW_SUPPORTED_GATEWAYS.get_key(),
        mf_filtered_gw,
    )
    .await;
    let filtered_gw = filter_gateways_for_flow(
        is_sbmd,
        C::SBMD_SUPPORTED_GATEWAYS.get_key(),
        mf_and_cb_filtered_gw,
    )
    .await;

    setGws(this, filtered_gw);
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForMerchantRequiredFlow,
        true,
    )
}

pub async fn filter_gateways_for_flow(
    condition: bool,
    redis_key: String,
    gateways: Vec<String>,
) -> Vec<String> {
    if condition {
        let supported_gateways: Vec<String> = findByNameFromRedis::<Vec<String>>(redis_key)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();
        gateways
            .into_iter()
            .filter(|gateway| supported_gateways.contains(gateway))
            .collect()
    } else {
        gateways
    }
}

pub async fn filterGatewaysForMGASelectionIntegrity(this: &mut DeciderFlow<'_>) -> GatewayList {
    let is_dynamic_mga_enabled =
        Utils::get_is_merchant_enabled_for_dynamic_mga_selection(this).await;

    if !is_dynamic_mga_enabled {
        return returnGwListWithLog(
            this,
            DeciderFilterName::FilterGatewaysForMGASelectionIntegrity,
            true,
        );
    }

    filterForEMITenureSpecificMGAs(this);
    let mgas = Utils::get_mgas(this).unwrap_or_default();
    let st = getGws(this);
    let txn_detail = this.get().dpTxnDetail.clone();

    let filtered_mgas = mgas
        .into_iter()
        .filter(|mga| st.contains(&mga.gateway))
        .collect::<Vec<_>>();

    let gwts = validate_only_one_mga(filtered_mgas, txn_detail, st);
    setGws(this, gwts);
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterGatewaysForMGASelectionIntegrity,
        true,
    )
}

pub fn validate_only_one_mga(
    mgas: Vec<MerchantGatewayAccount>,
    txn_detail: TxnDetail,
    st: Vec<String>,
) -> GatewayList {
    st.into_iter()
        .filter_map(|gwt| {
            let count = mgas.iter().filter(|mga| mga.gateway == gwt).count();
            if count == 1 {
                Some(gwt)
            } else {
                logger::error!(
                    tag = "INVALID_MGA_CONFIGURATION",
                    action = "INVALID_MGA_CONFIGURATION",
                    "txn_id: {:?}, gwt: {}",
                    txn_detail.txnId,
                    gwt
                );
                None
            }
        })
        .collect()
}

/// Filters gateways for EMI tenure-specific merchant gateway accounts
/// Keeps only gateways that support the specific EMI tenure requested in the transaction
pub fn filterForEMITenureSpecificMGAs(this: &mut DeciderFlow) -> Vec<String> {
    // Get transaction details from context
    let txn_detail = this.get().dpTxnDetail.clone();

    // Only filter if transaction is EMI
    if txn_detail.isEmi {
        // Get current functional gateways
        let st = getGws(this);

        // Get merchant gateway accounts
        let mgas = Utils::get_mgas(this).unwrap_or_default();

        // Filter MGAs based on EMI tenure support
        let filtered_mgas = mgas
            .into_iter()
            .filter(|gw_account| {
                // First check if gateway is in our functional list
                if st.contains(&gw_account.gateway) {
                    // Check if gateway needs tenure-specific credentials
                    if C::gatewaysWithTenureBasedCreds
                        .map(|str| str.to_string())
                        .contains(&gw_account.gateway.to_string())
                    {
                        // Extract account details and parse as EMI account details
                        let acc_details = gw_account.account_details.peek();
                        match serde_json::from_str::<EMIAccountDetails>(acc_details) {
                            Ok(emi_details) => {
                                // Check if EMI details match transaction EMI requirements
                                get_emi(emi_details.isEmi) == txn_detail.isEmi
                                    && get_tenure(emi_details.emiTenure)
                                        == txn_detail.emiTenure.unwrap_or(0)
                            }
                            _ => true, // If parsing fails, keep the gateway
                        }
                    } else {
                        true // Not a tenure-based gateway, keep it
                    }
                } else {
                    false // Gateway not in functional list, filter it out
                }
            })
            .collect();

        // Update the DeciderFlow with filtered MGAs
        setGwsAndMgas(this, filtered_mgas);
    }

    // Return the gateway list with logging
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterGatewaysForEMITenureSpecficGatewayCreds,
        true,
    )
}

/// Determines if an optional AValue represents a true value
/// Returns true if the value is a boolean true or a string "true" (case-insensitive)
fn get_emi(is_emi: Option<AValue>) -> bool {
    is_emi
        .map(|value| match value {
            AValue::Bool(b) => b,
            AValue::String(s) => s.to_lowercase() == "true",
            _ => false,
        })
        .unwrap_or(false)
}

/// Gets the EMI tenure value with a default of 0
fn get_tenure(tenure: Option<i32>) -> i32 {
    tenure.unwrap_or(0)
}

pub async fn filterGatewaysForConsumerFinance(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let consumer_finance_only_gateways: Vec<String> =
        findByNameFromRedis::<Vec<String>>(C::CONSUMER_FINANCE_ONLY_GATEWAYS.get_key())
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let consumer_finance_only_gateways_hashset = consumer_finance_only_gateways
        .into_iter()
        .collect::<HashSet<_>>();

    if txn_card_info.paymentMethodType == CONSUMER_FINANCE {
        let consumer_finance_also_gateways: Vec<String> =
            findByNameFromRedis::<Vec<String>>(C::CONSUMER_FINANCE_ALSO_GATEWAYS.get_key())
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();
        let consumer_finance_also_gateways_hashset = consumer_finance_also_gateways
            .into_iter()
            .collect::<HashSet<_>>();
        let consumer_finance_support_gateways = consumer_finance_only_gateways_hashset
            .union(&consumer_finance_also_gateways_hashset)
            .cloned()
            .collect::<HashSet<_>>();
        setGws(
            this,
            st.into_iter()
                .filter(|gw| consumer_finance_support_gateways.contains(gw))
                .collect(),
        );
    } else {
        setGws(
            this,
            st.into_iter()
                .filter(|gw| !consumer_finance_only_gateways_hashset.contains(gw))
                .collect(),
        );
    }

    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForConsumerFinance,
        true,
    )
}

pub async fn filterGatewaysForUpi(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let txn_card_info = this.get().dpTxnCardInfo.clone();
    let txn_detail = this.get().dpTxnDetail.clone();
    let upi_only_gateways: Vec<String> =
        findByNameFromRedis::<Vec<String>>(C::UPI_ONLY_GATEWAYS.get_key())
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    //Convert upi_only_gateways to <HashSet<_>>
    let upi_only_gateways_hashset = upi_only_gateways.into_iter().collect::<HashSet<_>>();

    if txn_card_info.paymentMethodType == UPI {
        let upi_also_gateway: Vec<String> =
            findByNameFromRedis::<Vec<String>>(C::UPI_ALSO_GATEWAYS.get_key())
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();
        let upi_also_gateway_hashset = upi_also_gateway.into_iter().collect::<HashSet<_>>();

        let upi_support_gateways = upi_only_gateways_hashset
            .union(&upi_also_gateway_hashset)
            .cloned()
            .collect::<HashSet<_>>();
        setGws(
            this,
            st.into_iter()
                .filter(|gateway| upi_support_gateways.contains(gateway))
                .collect(),
        );
    } else if !SUTC::is_google_pay_txn(txn_card_info) {
        setGws(
            this,
            st.into_iter()
                .filter(|gateway| !upi_only_gateways_hashset.contains(gateway))
                .collect(),
        );
    } else {
        // Do nothing
    }

    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForUpi,
        true,
    )
}

pub async fn filterGatewaysForTxnType(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let m_txn_type = this.get().dpTxnType.clone();
    let txn_card_info = this.get().dpTxnCardInfo.clone();

    let maybe_txn_type = Utils::get_true_string(m_txn_type);
    match maybe_txn_type {
        None => (),
        Some(txn_type) => {
            let mgas = Utils::get_mgas(this).unwrap_or_default();
            let (st, curr_mgas) =
                if txn_card_info.paymentMethodType == UPI && txn_card_info.paymentMethod == UPI {
                    let functional_mgas: Vec<_> = mgas
                        .iter()
                        .filter(|mga| {
                            st.contains(&mga.gateway)
                                && Utils::is_txn_type_enabled(
                                    mga.supportedTxnType.as_deref(),
                                    UPI,
                                    &txn_type,
                                )
                        })
                        .cloned()
                        .collect();
                    (
                        functional_mgas
                            .iter()
                            .map(|mga| mga.gateway.clone())
                            .collect(),
                        functional_mgas,
                    )
                } else {
                    (st.clone(), mgas)
                };

            let v2_integration_not_supported_gateways: Vec<String> =
                findByNameFromRedis(C::V2_INTEGRATION_NOT_SUPPORTED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_default();
            let upi_intent_not_supported_gateways: Vec<String> =
                findByNameFromRedis(C::UPI_INTENT_NOT_SUPPORTED_GATEWAYS.get_key())
                    .await
                    .unwrap_or_default();
            let (_, filtered_mgas) = if ["UPI_PAY", "UPI_QR"].contains(&txn_type.as_str())
                // && intersect(&st, &(v2_integration_not_supported_gateways.clone() + upi_intent_not_supported_gateways.clone())).is_empty()
                && {
                    let mut combined_gateways = v2_integration_not_supported_gateways.clone();
                    combined_gateways.extend(upi_intent_not_supported_gateways.clone());
                    !intersect(&st, &combined_gateways).is_empty()
                } {
                filterGatewaysForUpiPayBasedOnSupportedFlow(
                    this,
                    st,
                    curr_mgas,
                    v2_integration_not_supported_gateways,
                    upi_intent_not_supported_gateways,
                )
            } else {
                (st.clone(), curr_mgas)
            };

            let txn_type_gateway_mapping = findByNameFromRedis::<HashMap<String, Vec<String>>>(
                C::TXN_TYPE_GATEWAY_MAPPING.get_key(),
            )
            .await
            .unwrap_or_default();
            setGwsAndMgas(
                this,
                filtered_mgas
                    .iter()
                    .filter(|mga| {
                        getTxnTypeSupportedGateways(&txn_type, &txn_type_gateway_mapping)
                            .contains(&mga.gateway)
                    })
                    .cloned()
                    .collect(),
            );
        }
    }
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForTxnType,
        true,
    )
}

fn getTxnTypeSupportedGateways(
    txn_type: &str,
    txn_type_gateway_mapping: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    txn_type_gateway_mapping
        .iter()
        .find(|mapping| mapping.0 == txn_type)
        .map(|mapping| mapping.1.clone())
        .unwrap_or_else(std::vec::Vec::new)
}

/// Filters gateways and merchant gateway accounts based on UPI payment flow support
/// Checks both V2 integration and UPI intent capabilities
pub fn filterGatewaysForUpiPayBasedOnSupportedFlow(
    this: &mut DeciderFlow,
    gws: Vec<String>,
    mgas: Vec<MerchantGatewayAccount>,
    v2_integration_not_supported_gateways: Vec<String>,
    upi_intent_not_supported_gateways: Vec<String>,
) -> (Vec<String>, Vec<MerchantGatewayAccount>) {
    // First filter: Check V2 integration support
    let upd_mgas: Vec<MerchantGatewayAccount> = mgas
        .into_iter()
        .filter(|mga| {
            // If gateway is not in the "not supported" list, it's supported
            if !v2_integration_not_supported_gateways.contains(&mga.gateway) {
                return true;
            }

            // Otherwise, check if V2 integration is enabled for this MGA
            let value = Utils::is_payment_flow_enabled_in_mga(mga, "V2_INTEGRATION")
                .map(|enabled| {
                    if enabled {
                        "true".to_string()
                    } else {
                        "0".to_string()
                    }
                })
                .or_else(|| Utils::get_value("shouldUseV2Integration", mga.account_details.peek()));

            // MGA passes filter if value is "true" or "1"
            value == Some("true".to_string()) || value == Some("1".to_string())
        })
        .collect();

    // Second filter: Check UPI intent support
    let upd_mgas: Vec<MerchantGatewayAccount> = upd_mgas
        .into_iter()
        .filter(|mga| {
            // If gateway is not in the "not supported" list, it's supported
            if !upi_intent_not_supported_gateways.contains(&mga.gateway) {
                return true;
            }

            // Otherwise, check if UPI intent is enabled for this MGA
            let value = Utils::get_value("isUpiIntentEnabled", mga.account_details.peek());
            value == Some("true".to_string())
        })
        .collect();

    // Extract just the gateways from the filtered MGAs
    let gateways = upd_mgas.iter().map(|mga| mga.gateway.clone()).collect();

    // Return both the gateway list and the filtered MGA list
    (gateways, upd_mgas)
}

pub async fn filterGatewaysForTxnDetailType(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let m_txn_type = this.get().dpTxnDetail.txnType.clone();
    let txn_type: &str = m_txn_type.as_str();
    let txn_detail_type_restricted_gateways =
        findByNameFromRedis(C::TXN_DETAIL_TYPE_RESTRICTED_GATEWAYS.get_key())
            .await
            .unwrap_or_default();
    let filter_gws = if txn_type == "ZERO_AUTH" {
        st.iter()
            .filter(|gw| {
                get_zero_auth_supported_gateways(txn_type, &txn_detail_type_restricted_gateways)
                    .contains(gw)
            })
            .cloned()
            .collect()
    } else {
        st
    };
    setGws(this, filter_gws);
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForTxnDetailType,
        true,
    )
}

fn get_zero_auth_supported_gateways(
    txn_type: &str,
    txn_detail_type_restricted_gateways: &Vec<(String, Vec<String>)>,
) -> Vec<String> {
    txn_detail_type_restricted_gateways
        .iter()
        .find(|mapping| mapping.0 == txn_type)
        .map_or_else(Vec::new, |mapping| mapping.1.clone())
}

pub async fn filterGatewaysForReward(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let payment_method_type = this.get().dpTxnCardInfo.paymentMethodType.clone();
    let card_type = this.get().dpTxnCardInfo.card_type.clone();
    let reward_also_gateways: HashSet<String> =
        findByNameFromRedis(C::REWARD_ALSO_GATEWAYS.get_key())
            .await
            .unwrap_or_else(Vec::new)
            .into_iter()
            .collect();
    let reward_only_gateways: HashSet<String> =
        findByNameFromRedis(C::REWARD_ONLY_GATEWAYS.get_key())
            .await
            .unwrap_or_else(Vec::new)
            .into_iter()
            .collect();
    let filtered_gws = if card_type == Some(ETCA::CardType::Reward) || payment_method_type == REWARD
    {
        st.into_iter()
            .filter(|gw| reward_also_gateways.contains(gw) || reward_only_gateways.contains(gw))
            .collect()
    } else {
        st.into_iter()
            .filter(|gw| !reward_only_gateways.contains(gw))
            .collect()
    };
    setGws(this, filtered_gws);
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForReward,
        true,
    )
}

pub async fn filterGatewaysForCash(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let st = getGws(this);
    let payment_method_type = this.get().dpTxnCardInfo.paymentMethodType.clone();
    if payment_method_type != CASH {
        let cash_only_gateways: Vec<String> = findByNameFromRedis(C::CASH_ONLY_GATEWAYS.get_key())
            .await
            .unwrap_or_else(Vec::new)
            .into_iter()
            .collect();
        let filtered_gws = st
            .into_iter()
            .filter(|gw| !cash_only_gateways.contains(gw))
            .collect();
        setGws(this, filtered_gws);
    }
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForCash,
        true,
    )
}

pub async fn filterFunctionalGatewaysForSplitSettlement(this: &mut DeciderFlow<'_>) -> Vec<String> {
    let oref = this.get().dpOrder.clone();
    let txn_id = this.get().dpTxnDetail.clone();
    let e_split_settlement_details = Utils::get_split_settlement_details(this).await;
    let macc = this.get().dpMerchantAccount.clone();
    logger::debug!(
        tag = "enableGatewayReferenceIdBasedRouting in splitsettlement",
        action = "enableGatewayReferenceIdBasedRouting in splitsettlement",
        "enableGatewayReferenceIdBasedRouting: for txnId {:?} is {:?}",
        txn_id,
        macc.enableGatewayReferenceIdBasedRouting
    );
    let (metadata, pl_ref_id_map) = Utils::get_order_metadata_and_pl_ref_id_map(
        this,
        macc.enableGatewayReferenceIdBasedRouting,
        &oref,
    );
    //await response for possible_ref_ids_of_merchant
    let possible_ref_ids_of_merchant =
        Utils::get_all_possible_ref_ids(metadata.clone(), oref.clone(), pl_ref_id_map.clone());

    let possible_ref_ids_of_merchant_string = possible_ref_ids_of_merchant
        .iter()
        .map(|x| x.mga_reference_id.clone())
        .collect::<Vec<String>>();

    match e_split_settlement_details {
        Ok(split_settlement_details) => {
            let given_sub_mids: Vec<_> = split_settlement_details
                .vendor
                .split
                .iter()
                .map(|v| v.sub_mid.clone())
                .collect();
            let given_sub_mids_size = given_sub_mids.len();
            if given_sub_mids.is_empty() {
                logger::debug!(
                    tag = "SplitSettlement",
                    action = "SplitSettlement",
                    "Empty givenSubMids - skipping SplitSettlement filter"
                )
            } else {
                let st = getGws(this);
                let enabled_gateway_accounts = ETMA::getEnabledMgasByMerchantIdAndRefId(
                    macc.merchantId.0.clone(),
                    possible_ref_ids_of_merchant_string.clone(),
                )
                .await;
                // s::get_enabled_mgas_by_merchant_id_and_ref_id(
                //     macc.merchant_id.clone(),
                //     possible_ref_ids_of_merchant.clone(),
                // );
                let filtered_gateway_accounts: Vec<_> = enabled_gateway_accounts
                    .into_iter()
                    .filter_map(|gwacc| {
                        let gw_ref_id = Utils::get_gateway_reference_id(
                            metadata.clone(),
                            &gwacc.gateway.clone(),
                            oref.clone(),
                            pl_ref_id_map.clone(),
                        );
                        if gwacc.referenceId == gw_ref_id && st.contains(&gwacc.gateway) {
                            Some(gwacc)
                        } else {
                            None
                        }
                    })
                    .collect();
                let a: Vec<ETMA::MerchantGwAccId> = filtered_gateway_accounts
                    .iter()
                    .map(|mga| mga.id.clone())
                    .collect();
                let merchant_gateway_account_sub_infos =
                    ETMGASI::find_all_mgasi_by_maga_ids(&a).await;
                let merchant_gateway_account_list_map: std::collections::HashMap<_, _> =
                    filtered_gateway_accounts.into_iter().fold(
                        std::collections::HashMap::new(),
                        |mut acc, mga| {
                            let sub_infos: Vec<_> = merchant_gateway_account_sub_infos
                                .iter()
                                .filter(|mgasi| {
                                    mgasi.merchantGatewayAccountId == mga.id
                                        && mgasi.subIdType == SubIdType::VENDOR
                                        && mgasi.subInfoType == SubInfoType::SPLIT_SETTLEMENT
                                        && !mgasi.disabled
                                })
                                .map(|mgasi| mgasi.juspaySubAccountId.clone())
                                .collect();
                            //Create a hasmap of mga.id and sub_infos
                            acc.insert(mga.id.clone(), sub_infos);

                            // acc.insert(mga.id.clone(), sub_infos);
                            // acc.insert((mga.id.clone(), mga.gateway.clone()), sub_infos);
                            acc
                        },
                    );
                let map_keys: Vec<_> = merchant_gateway_account_list_map
                    .iter()
                    .filter(|(_, v)| intersect(&given_sub_mids, v).len() == given_sub_mids_size)
                    .map(|(k, _)| k.clone())
                    .collect();
                let mga_ids = ord_nub(map_keys.iter().map(|k| k.merchantGwAccId).collect());
                let all_mgas = Utils::get_mgas(this).unwrap_or_default();
                setGwsAndMgas(
                    this,
                    all_mgas
                        .into_iter()
                        .filter(|mga| mga_ids.contains(&mga.id.merchantGwAccId))
                        .collect(),
                );
            }
        }
        Err(msg) => {
            logger::debug!(
                tag = "SplitSettlement",
                action = "SplitSettlement",
                "Skipping SplitSettlement filter : {}",
                msg
            );
            let st = getGws(this);
            let split_settlement_supported_gateways: Option<Vec<String>> =
                findByNameFromRedis(C::SPLIT_SETTLEMENT_SUPPORTED_GATEWAYS.get_key()).await;
            if !intersect(
                &split_settlement_supported_gateways.unwrap_or_default(),
                &st,
            )
            .is_empty()
            {
                let mgas = SETMA::get_split_settlement_only_gateway_accounts(
                    this,
                    possible_ref_ids_of_merchant.clone(),
                )
                .await;
                let all_mgas = Utils::get_mgas(this).unwrap_or_default();
                let filtered_mgas = all_mgas
                    .into_iter()
                    .filter(|mga| !mgas.iter().any(|mga_filter| mga_filter.id == mga.id))
                    .collect();
                setGwsAndMgas(this, filtered_mgas);
            }
        }
    }
    returnGwListWithLog(
        this,
        DeciderFilterName::FilterFunctionalGatewaysForSplitSettlement,
        true,
    )
}
