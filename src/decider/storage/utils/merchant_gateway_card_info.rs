use crate::storage::schema::gateway_card_info as g_dsl;
use crate::storage::schema::juspay_bank_code as j_dsl;

use crate::storage::schema::merchant_gateway_card_info as m_dsl;
use crate::storage::types::BitBool;
use crate::storage::types::{
    GatewayCardInfo as DBGatewayCardInfo, JuspayBankCode as DBJuspayBankCode,
    MerchantGatewayCardInfo as DBMerchantGatewayCardInfo,
};

use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::gateway_card_info::validation_type_to_text;
use crate::types::gateway_card_info::ValidationType;
use crate::types::merchant::id::merchant_pid_to_text;
use crate::types::merchant::merchant_account::MerchantAccount;
use crate::types::merchant::merchant_gateway_account::merchant_gw_acc_id_to_id;
use crate::types::merchant::merchant_gateway_account::MerchantGatewayAccount;
use crate::types::merchant_gateway_card_info::MerchantGatewayCardInfo;
use crate::types::payment::payment_method::PaymentMethodType;
use diesel::associations::HasTable;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;

pub async fn filter_gateways_for_payment_method_and_validation_type(
    app_state: &crate::app::TenantAppState,
    merchant_account: MerchantAccount,
    txn_card_info: TxnCardInfo,
    enabled_gateway_accounts: Vec<MerchantGatewayAccount>,
    given_validation_type: ValidationType,
    _: String,
) -> Vec<MerchantGatewayCardInfo> {
    if enabled_gateway_accounts.is_empty() {
        return vec![];
    }

    let enabled_gateway_accounts_ids: Vec<i64> = enabled_gateway_accounts
        .iter()
        .map(|acc| merchant_gw_acc_id_to_id(acc.id.clone()))
        .collect();

    let given_payment_method = txn_card_info.paymentMethod.clone();
    let given_payment_method_type = txn_card_info.paymentMethodType;

    // Step 1: Fetch Juspay Bank Codes
    let jpbc_records: Vec<DBJuspayBankCode> = match crate::generics::generic_find_all::<
        <DBJuspayBankCode as HasTable>::Table,
        _,
        DBJuspayBankCode,
    >(
        &app_state.db,
        j_dsl::bank_code.eq(given_payment_method.clone()),
    )
    .await
    {
        Ok(records) => records,
        Err(_) => return vec![],
    };

    let valid_jpbc_ids: Vec<i64> = jpbc_records.iter().map(|rec| rec.id).collect();
    if valid_jpbc_ids.is_empty() {
        return vec![];
    }

    // Step 2: Fetch Gateway Card Info
    let gci_records: Vec<DBGatewayCardInfo> = match crate::generics::generic_find_all::<
            <DBGatewayCardInfo as HasTable>::Table,
            _,
            DBGatewayCardInfo
        >(
            &app_state.db,
                     g_dsl::juspay_bank_code_id.eq_any(valid_jpbc_ids)
                .and(g_dsl::disabled.eq(Some(BitBool(false))))
                .and(g_dsl::validation_type.eq(Some(validation_type_to_text(given_validation_type))))
                .and(g_dsl::payment_method_type.eq(PaymentMethodType::to_text(&given_payment_method_type))),
        ).await {
            Ok(records) => records,
            Err(_) => return vec![],
        };

    let gci_ids: Vec<i64> = gci_records.iter().map(|rec| rec.id).collect();
    if gci_ids.is_empty() {
        return vec![];
    }

    // Step 3: Fetch Merchant Gateway Card Info
    let mgci_records: Vec<DBMerchantGatewayCardInfo> = match crate::generics::generic_find_all::<
        <DBMerchantGatewayCardInfo as HasTable>::Table,
        _,
        DBMerchantGatewayCardInfo,
    >(
        &app_state.db,
        m_dsl::gateway_card_info_id.eq_any(gci_ids)
            .and(m_dsl::merchant_account_id.eq(merchant_pid_to_text(merchant_account.id.clone())))
            .and(m_dsl::disabled.eq(BitBool(false)))
            .and(m_dsl::merchant_gateway_account_id.eq_any(enabled_gateway_accounts_ids)),
    )
    .await
    {
        Ok(records) => records,
        Err(_) => return vec![],
    };

    // Step 4: Convert to Domain Types
    mgci_records
        .into_iter()
        .filter_map(|db_record| MerchantGatewayCardInfo::try_from(db_record).ok())
        .collect()
}
