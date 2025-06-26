use crate::decider::gatewaydecider::types::*;

// use eulerhs::prelude::*;
// use crate::decider::storage::utils::gatewaydecider::types::*;
// use gatewaydecider::types::*;
// use juspay::extra::secret::make_secret;
use crate::decider::gatewaydecider::utils::{get_mgas, is_emandate_enabled, set_mgas};
use crate::logger;
use crate::types::gateway as ETG;
use crate::types::merchant as ETM;
use crate::types::merchant::id::MerchantId;
use crate::types::merchant::merchant_gateway_account::MgaReferenceId;
// use eulerhs::language as l;
use crate::decider::gatewaydecider::utils::check_if_enabled_in_mga;

// TODO: why should we pass MerchantId here, can we just get it from mAcc?
pub async fn get_enabled_mgas_by_merchant_id_and_ref_id(
    this: &mut DeciderFlow<'_>,
    mid: MerchantId,
    ref_ids: Vec<MgaReferenceId>,
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {
    crate::logger::info!(
        "MGAS: Length : {:?}",
        this.writer.mgas.as_ref().map_or(611, |mgas| mgas.len())
    );
    let cm_mgas = get_mgas(this);

    //Get vector<string> for ref_ids
    let ref_ids_strings: Vec<String> = ref_ids
        .iter()
        .map(|x| x.mga_reference_id.clone())
        .collect::<Vec<String>>();

    match cm_mgas {
        Some(c_mgas) => c_mgas,
        None => {
            let d_mgas = ETM::merchant_gateway_account::getEnabledMgasByMerchantIdAndRefId(
                mid.0,
                ref_ids_strings,
            )
            .await;

            crate::logger::info!(
                "length of mgas for txnId in main function: {}",
                d_mgas.len()
            );
            let filtered_mgas = filter_morpheus(d_mgas);
            set_mgas(this, filtered_mgas.clone());
            filtered_mgas
        }
    }
}

pub async fn get_emandate_enabled_mga(
    this: &mut DeciderFlow<'_>,
    mid: MerchantId,
    ref_ids: Vec<MgaReferenceId>,
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {
    let enabled_mgas = get_enabled_mgas_by_merchant_id_and_ref_id(this, mid, ref_ids).await;
    enabled_mgas
        .into_iter()
        .filter(is_emandate_enabled)
        .collect()
}

pub async fn get_tpv_only_gateway_accounts(
    this: &mut DeciderFlow<'_>,
    ref_ids: Vec<MgaReferenceId>,
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {
    let m_acc = this.get().dpMerchantAccount.clone();
    let ref_ids_strings: Vec<String> = ref_ids
        .iter()
        .map(|x| x.mga_reference_id.to_string())
        .collect::<Vec<String>>();
    let mgas = ETM::merchant_gateway_account::getEnabledMgasByMerchantIdAndRefId(
        m_acc.merchantId.0,
        ref_ids_strings,
    )
    .await;
    let filtered_mgas = filter_morpheus(mgas);
    filtered_mgas
        .into_iter()
        .filter(is_tpv_only_gateway)
        .collect()
}

fn filter_morpheus(
    mgas: Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {
    mgas.into_iter()
        .filter(|mga| mga.gateway != "MORPHEUS".to_string())
        .collect()
}

pub fn is_only_one_paylater(
    mgas: &Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>,
) -> bool {
    match mgas.as_slice() {
        [] => false,
        [mga] => mga.gateway == "PAYLATER".to_string(),
        _ => false,
    }
}

pub fn is_tpv_only_gateway(mga: &ETM::merchant_gateway_account::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "TPV_ONLY", "tpvOnly")
}

pub fn is_vies_enabled(mga: &ETM::merchant_gateway_account::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "CARD_VIES", "viesEnabled")
}

pub async fn get_split_settlement_only_gateway_accounts(
    this: &mut DeciderFlow<'_>,
    ref_ids: Vec<MgaReferenceId>,
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {
    let m_acc = this.get().dpMerchantAccount.clone();
    let ref_ids_strings: Vec<String> = ref_ids
        .iter()
        .map(|x| x.mga_reference_id.to_string())
        .collect::<Vec<String>>();
    let mgas = ETM::merchant_gateway_account::getEnabledMgasByMerchantIdAndRefId(
        m_acc.merchantId.0,
        ref_ids_strings,
    )
    .await;
    let filtered_mgas = filter_morpheus(mgas);
    filtered_mgas
        .into_iter()
        .filter(is_split_settlement_only_gateway)
        .collect()
}

pub fn is_split_settlement_only_gateway(
    mga: &ETM::merchant_gateway_account::MerchantGatewayAccount,
) -> bool {
    check_if_enabled_in_mga(mga, "SPLIT_SETTLE_ONLY", "splitSettlementOnly")
}

pub async fn get_all_enabled_mgas_by_merchant_id(
    mid: MerchantId,
) -> Vec<ETM::merchant_gateway_account::MerchantGatewayAccount> {
    let mgas = ETM::merchant_gateway_account::getEnabledMgasByMerchantId(mid.0).await;
    filter_morpheus(mgas)
}
