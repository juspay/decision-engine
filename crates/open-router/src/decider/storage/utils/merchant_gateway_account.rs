

// use eulerhs::prelude::*;
use gatewaydecider::types::*;
// use juspay::extra::secret::make_secret;
use gatewaydecider::utils::{get_mgas, is_emandate_enabled, set_mgas, check_if_enabled_in_mga};
use crate::types::merchant::id::MerchantId;
use crate::types::merchant::merchant_gateway_account::MgaReferenceId;
use crate::types::merchant as etm;
use crate::types::gateway as etg;
// use eulerhs::language as l;

// TODO: why should we pass MerchantId here, can we just get it from mAcc?
pub fn get_enabled_mgas_by_merchant_id_and_ref_id(
    mid: MerchantId,
    ref_ids: Vec<MgaReferenceId>,
) -> Vec<etm::merchant_gateway_account::MerchantGatewayAccount> {
    let cm_mgas = get_mgas();
    let m_acc = asks(|ctx| ctx.dp_merchant_account.clone());
    match cm_mgas {
        Some(c_mgas) => Ok(c_mgas),
        None => {
            let d_mgas = etm::get_enabled_mgas_by_merchant_id_and_ref_id(mid, ref_ids)?;
            l::log_debug_t("length of mgas for txnId in main function: ", &format!("{}", d_mgas.len()));
            let filtered_mgas = filter_morpheus(d_mgas);
            set_mgas(filtered_mgas.clone());
            Ok(filtered_mgas)
        }
    }
}

pub fn get_emandate_enabled_mga(
    mid: etm::MerchantId,
    ref_ids: Vec<etm::MgaReferenceId>,
) -> DeciderFlow<Vec<etm::MerchantGatewayAccount>> {
    let enabled_mgas = get_enabled_mgas_by_merchant_id_and_ref_id(mid, ref_ids)?;
    Ok(enabled_mgas.into_iter().filter(is_emandate_enabled).collect())
}

pub fn get_tpv_only_gateway_accounts(
    ref_ids: Vec<etm::MgaReferenceId>,
) -> DeciderFlow<Vec<etm::MerchantGatewayAccount>> {
    let m_acc = asks(|ctx| ctx.dp_merchant_account.clone());
    let mgas = get_enabled_mgas_by_merchant_id_and_ref_id(m_acc.merchant_id, ref_ids)?;
    let filtered_mgas = filter_morpheus(mgas);
    Ok(filtered_mgas.into_iter().filter(is_tpv_only_gateway).collect())
}

fn filter_morpheus(mgas: Vec<etm::MerchantGatewayAccount>) -> Vec<etm::MerchantGatewayAccount> {
    mgas.into_iter().filter(|mga| mga.gateway != etg::MORPHEUS).collect()
}

pub fn is_only_one_paylater(mgas: Vec<etm::MerchantGatewayAccount>) -> bool {
    match mgas.as_slice() {
        [] => false,
        [mga] => mga.gateway == etg::PAYLATER,
        _ => false,
    }
}

pub fn is_tpv_only_gateway(mga: &etm::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "TPV_ONLY", "tpvOnly")
}

pub fn is_vies_enabled(mga: &etm::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "CARD_VIES", "viesEnabled")
}

pub fn get_split_settlement_only_gateway_accounts(
    ref_ids: Vec<etm::MgaReferenceId>,
) -> DeciderFlow<Vec<etm::MerchantGatewayAccount>> {
    let m_acc = asks(|ctx| ctx.dp_merchant_account.clone());
    let mgas = get_enabled_mgas_by_merchant_id_and_ref_id(m_acc.merchant_id, ref_ids)?;
    let filtered_mgas = filter_morpheus(mgas);
    Ok(filtered_mgas.into_iter().filter(is_split_settlement_only_gateway).collect())
}

pub fn is_split_settlement_only_gateway(mga: &etm::MerchantGatewayAccount) -> bool {
    check_if_enabled_in_mga(mga, "SPLIT_SETTLE_ONLY", "splitSettlementOnly")
}

pub fn get_all_enabled_mgas_by_merchant_id(
    mid: etm::MerchantId,
) -> DeciderFlow<Vec<etm::MerchantGatewayAccount>> {
    let mgas = etm::get_enabled_mgas_by_merchant_id(mid)?;
    Ok(filter_morpheus(mgas))
}