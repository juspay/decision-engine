use serde::{Deserialize, Serialize};

use crate::error::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum PaymentFlow {
    CARD_3DS,
    CARD_3DS2,
    CARD_DOTP,
    CARD_MOTO,
    CARD_NO_3DS,
    CARD_VIES,
    ZERO_AUTH,
    CARD_TOKENIZATION,
    CVVLESS,
    DIRECT_DEBIT,
    EMANDATE,
    EMI,
    INAPP_DEBIT,
    MANDATE,
    PARTIAL_CAPTURE,
    PARTIAL_VOID,
    PARTIAL_PAYMENT,
    PREAUTH,
    SDKLESS_INTENT,
    SPLIT_PAYMENT,
    SPLIT_SETTLEMENT,
    TOPUP,
    TPV,
    VISA_CHECKOUT,
    OUTAGE,
    SR_BASED_ROUTING,
    ELIMINATION_BASED_ROUTING,
    PL_BASED_ROUTING,
    MANDATE_WORKFLOW,
    ALTID,
    SURCHARGE,
    OFFER,
    CAPTCHA,
    PAYMENT_COLLECTION_LINK,
    AUTO_REFUND,
    PAYMENT_LINK,
    PAYMENT_FORM,
    RISK_CHECK,
    DYNAMIC_CURRENCY_CONVERSION,
    PART_PAYMENT,
    STANDALONE_AUTHENTICATION,
    STANDALONE_AUTHORIZATION,
    STANDALONE_CAPTURE,
    AUTHN_AUTHZ,
    AUTHZ_CAPTURE,
    REDIRECT_DEBIT,
    LINK_AND_DEBIT,
    NEW_CARD,
    INSTANT_REFUND,
    ASYNC,
    DOTP,
    MERCHANT_MANAGED_DEBIT,
    ADDRESS_VERIFICATION,
    FRICTIONLESS_3DS,
    TA_FILE,
    FIDO,
    REFUND,
    CTP,
    ONE_TIME_PAYMENT,
    REVERSE_PENNY_DROP,
    ON_DEMAND_SPLIT_SETTLEMENT,
    CREDIT_CARD_ON_UPI,
    DECIDER_FALLBACK_DOTP_TO_3DS,
    DECIDER_FALLBACK_NO_3DS_TO_3DS,
    PAYMENT_CHANNEL_FALLBACK_DOTP_TO_3DS,
    PG_FAILURE_FALLBACK_DOTP_TO_3DS,
    TOKENIZATION_CONSENT_FALLBACK_DOTP_TO_3DS,
    CUSTOMER_FALLBACK_DOTP_TO_3DS,
    AUTH_PROVIDER_FALLBACK_3DS2_TO_3DS,
    FRM_PREFERENCE_TO_NO_3DS,
    MERCHANT_FALLBACK_3DS2_TO_3DS,
    MERCHANT_FALLBACK_FIDO_TO_3DS,
    ORDER_PREFERENCE_FALLBACK_NO_3DS_TO_3DS,
    MERCHANT_PREFERENCE_FALLBACK_NO_3DS_TO_3DS,
    ORDER_PREFERENCE_TO_NO_3DS,
    MUTUAL_FUND,
    CROSS_BORDER_PAYMENT,
    APPLEPAY_TOKEN_DECRYPTION_FLOW,
    ONE_TIME_MANDATE,
    SINGLE_BLOCK_MULTIPLE_DEBIT,
    SILENT_RETRY,
    WALLET_TOPUP,
    NETWORK_TOKEN_CREATED,
    ISSUER_TOKEN_CREATED,
    LOCKER_TOKEN_CREATED,
    SODEXO_TOKEN_CREATED,
    NETWORK_TOKEN_USED,
    ISSUER_TOKEN_USED,
    LOCKER_TOKEN_USED,
    SODEXO_TOKEN_USED,
    PAYU_TOKEN_USED,
    MANDATE_REGISTER,
    MANDATE_REGISTER_DEBIT,
    MANDATE_PAYMENT,
    EMANDATE_REGISTER,
    EMANDATE_REGISTER_DEBIT,
    EMANDATE_PAYMENT,
    SI_HUB,
    TPV_EMANDATE,
    COLLECT,
    INTENT,
    INAPP,
    QR,
    PUSH_PAY,
    NO_COST_EMI,
    LOW_COST_EMI,
    STANDARD_EMI,
    STANDARD_EMI_SPLIT,
    INTERNAL_NO_COST_EMI,
    INTERNAL_LOW_COST_EMI,
    INTERNAL_NO_COST_EMI_SPLIT,
    INTERNAL_LOW_COST_EMI_SPLIT,
    DIRECT_BANK_EMI,
    PG_EMI,
    AUTO_DISBURSEMENT,
    AUTO_USER_REGISTRATION,
    BANK_INSTANT_REFUND,
    MANDATE_PREDEBIT_NOTIFICATION_DISABLEMENT,
    ORDER_AMOUNT_AS_SUBVENTION_AMOUNT,
    ORDER_ID_AS_RECON_ID,
    PASS_USER_TOKEN_TO_GATEWAY,
    S2S_FLOW,
    SPLIT_SETTLE_ONLY,
    SUBSCRIPTION_ONLY,
    TPV_ONLY,
    TXN_UUID_AS_TR,
    UPI_INTENT_REGISTRATION,
    V2_INTEGRATION,
    V2_LINK_AND_PAY,
    VPOS2,
    PAYMENT_PAGE,
    PP_QUICKPAY,
    PP_RETRY,
    INAPP_NEW_PAY,
    INAPP_REPEAT_PAY,
}

pub fn payment_flows_to_text(payment_flow: &PaymentFlow) -> String {
    match payment_flow {
        PaymentFlow::CARD_3DS => "CARD_3DS".to_string(),
        PaymentFlow::CARD_3DS2 => "CARD_3DS2".to_string(),
        PaymentFlow::FRICTIONLESS_3DS => "FRICTIONLESS_3DS".to_string(),
        PaymentFlow::CARD_DOTP => "CARD_DOTP".to_string(),
        PaymentFlow::CARD_MOTO => "CARD_MOTO".to_string(),
        PaymentFlow::CARD_NO_3DS => "CARD_NO_3DS".to_string(),
        PaymentFlow::CARD_VIES => "CARD_VIES".to_string(),
        PaymentFlow::ZERO_AUTH => "ZERO_AUTH".to_string(),
        PaymentFlow::CARD_TOKENIZATION => "CARD_TOKENIZATION".to_string(),
        PaymentFlow::CVVLESS => "CVVLESS".to_string(),
        PaymentFlow::DIRECT_DEBIT => "DIRECT_DEBIT".to_string(),
        PaymentFlow::EMANDATE => "EMANDATE".to_string(),
        PaymentFlow::EMI => "EMI".to_string(),
        PaymentFlow::INAPP_DEBIT => "INAPP_DEBIT".to_string(),
        PaymentFlow::MANDATE => "MANDATE".to_string(),
        PaymentFlow::PARTIAL_CAPTURE => "PARTIAL_CAPTURE".to_string(),
        PaymentFlow::PARTIAL_VOID => "PARTIAL_VOID".to_string(),
        PaymentFlow::PARTIAL_PAYMENT => "PARTIAL_PAYMENT".to_string(),
        PaymentFlow::PREAUTH => "PREAUTH".to_string(),
        PaymentFlow::SDKLESS_INTENT => "SDKLESS_INTENT".to_string(),
        PaymentFlow::SPLIT_PAYMENT => "SPLIT_PAYMENT".to_string(),
        PaymentFlow::SPLIT_SETTLEMENT => "SPLIT_SETTLEMENT".to_string(),
        PaymentFlow::WALLET_TOPUP => "WALLET_TOPUP".to_string(),
        PaymentFlow::TPV => "TPV".to_string(),
        PaymentFlow::VISA_CHECKOUT => "VISA_CHECKOUT".to_string(),
        PaymentFlow::AUTO_DISBURSEMENT => "AUTO_DISBURSEMENT".to_string(),
        PaymentFlow::AUTO_USER_REGISTRATION => "AUTO_USER_REGISTRATION".to_string(),
        PaymentFlow::BANK_INSTANT_REFUND => "BANK_INSTANT_REFUND".to_string(),
        PaymentFlow::MANDATE_PREDEBIT_NOTIFICATION_DISABLEMENT => {
            "MANDATE_PREDEBIT_NOTIFICATION_DISABLEMENT".to_string()
        }
        PaymentFlow::ORDER_AMOUNT_AS_SUBVENTION_AMOUNT => {
            "ORDER_AMOUNT_AS_SUBVENTION_AMOUNT".to_string()
        }
        PaymentFlow::ORDER_ID_AS_RECON_ID => "ORDER_ID_AS_RECON_ID".to_string(),
        PaymentFlow::PASS_USER_TOKEN_TO_GATEWAY => "PASS_USER_TOKEN_TO_GATEWAY".to_string(),
        PaymentFlow::S2S_FLOW => "S2S_FLOW".to_string(),
        PaymentFlow::SPLIT_SETTLE_ONLY => "SPLIT_SETTLE_ONLY".to_string(),
        PaymentFlow::SUBSCRIPTION_ONLY => "SUBSCRIPTION_ONLY".to_string(),
        PaymentFlow::TPV_ONLY => "TPV_ONLY".to_string(),
        PaymentFlow::TXN_UUID_AS_TR => "TXN_UUID_AS_TR".to_string(),
        PaymentFlow::UPI_INTENT_REGISTRATION => "UPI_INTENT_REGISTRATION".to_string(),
        PaymentFlow::V2_INTEGRATION => "V2_INTEGRATION".to_string(),
        PaymentFlow::V2_LINK_AND_PAY => "V2_LINK_AND_PAY".to_string(),
        PaymentFlow::VPOS2 => "VPOS2".to_string(),
        PaymentFlow::OUTAGE => "OUTAGE".to_string(),
        PaymentFlow::SR_BASED_ROUTING => "SR_BASED_ROUTING".to_string(),
        PaymentFlow::ELIMINATION_BASED_ROUTING => "ELIMINATION_BASED_ROUTING".to_string(),
        PaymentFlow::PL_BASED_ROUTING => "PL_BASED_ROUTING".to_string(),
        PaymentFlow::MANDATE_WORKFLOW => "MANDATE_WORKFLOW".to_string(),
        PaymentFlow::ALTID => "ALTID".to_string(),
        PaymentFlow::SURCHARGE => "SURCHARGE".to_string(),
        PaymentFlow::OFFER => "OFFER".to_string(),
        PaymentFlow::CAPTCHA => "CAPTCHA".to_string(),
        PaymentFlow::PAYMENT_COLLECTION_LINK => "PAYMENT_COLLECTION_LINK".to_string(),
        PaymentFlow::AUTO_REFUND => "AUTO_REFUND".to_string(),
        PaymentFlow::PAYMENT_LINK => "PAYMENT_LINK".to_string(),
        PaymentFlow::PAYMENT_FORM => "PAYMENT_FORM".to_string(),
        PaymentFlow::RISK_CHECK => "RISK_CHECK".to_string(),
        PaymentFlow::DYNAMIC_CURRENCY_CONVERSION => "DYNAMIC_CURRENCY_CONVERSION".to_string(),
        PaymentFlow::PART_PAYMENT => "PART_PAYMENT".to_string(),
        PaymentFlow::STANDALONE_AUTHENTICATION => "STANDALONE_AUTHENTICATION".to_string(),
        PaymentFlow::STANDALONE_AUTHORIZATION => "STANDALONE_AUTHORIZATION".to_string(),
        PaymentFlow::STANDALONE_CAPTURE => "STANDALONE_CAPTURE".to_string(),
        PaymentFlow::AUTHN_AUTHZ => "AUTHN_AUTHZ".to_string(),
        PaymentFlow::AUTHZ_CAPTURE => "AUTHZ_CAPTURE".to_string(),
        PaymentFlow::REDIRECT_DEBIT => "REDIRECT_DEBIT".to_string(),
        PaymentFlow::LINK_AND_DEBIT => "LINK_AND_DEBIT".to_string(),
        PaymentFlow::NEW_CARD => "NEW_CARD".to_string(),
        PaymentFlow::NETWORK_TOKEN_CREATED => "NETWORK_TOKEN_CREATED".to_string(),
        PaymentFlow::ISSUER_TOKEN_CREATED => "ISSUER_TOKEN_CREATED".to_string(),
        PaymentFlow::LOCKER_TOKEN_CREATED => "LOCKER_TOKEN_CREATED".to_string(),
        PaymentFlow::SODEXO_TOKEN_CREATED => "SODEXO_TOKEN_CREATED".to_string(),
        PaymentFlow::NETWORK_TOKEN_USED => "NETWORK_TOKEN_USED".to_string(),
        PaymentFlow::ISSUER_TOKEN_USED => "ISSUER_TOKEN_USED".to_string(),
        PaymentFlow::LOCKER_TOKEN_USED => "LOCKER_TOKEN_USED".to_string(),
        PaymentFlow::SODEXO_TOKEN_USED => "SODEXO_TOKEN_USED".to_string(),
        PaymentFlow::PAYU_TOKEN_USED => "PAYU_TOKEN_USED".to_string(),
        PaymentFlow::MANDATE_REGISTER => "MANDATE_REGISTER".to_string(),
        PaymentFlow::MANDATE_REGISTER_DEBIT => "MANDATE_REGISTER_DEBIT".to_string(),
        PaymentFlow::MANDATE_PAYMENT => "MANDATE_PAYMENT".to_string(),
        PaymentFlow::EMANDATE_REGISTER => "EMANDATE_REGISTER".to_string(),
        PaymentFlow::EMANDATE_REGISTER_DEBIT => "EMANDATE_REGISTER_DEBIT".to_string(),
        PaymentFlow::EMANDATE_PAYMENT => "EMANDATE_PAYMENT".to_string(),
        PaymentFlow::SI_HUB => "SI_HUB".to_string(),
        PaymentFlow::TPV_EMANDATE => "TPV_EMANDATE".to_string(),
        PaymentFlow::COLLECT => "COLLECT".to_string(),
        PaymentFlow::INTENT => "INTENT".to_string(),
        PaymentFlow::INAPP => "INAPP".to_string(),
        PaymentFlow::QR => "QR".to_string(),
        PaymentFlow::PUSH_PAY => "PUSH_PAY".to_string(),
        PaymentFlow::INSTANT_REFUND => "INSTANT_REFUND".to_string(),
        PaymentFlow::ASYNC => "ASYNC".to_string(),
        PaymentFlow::DOTP => "DOTP".to_string(),
        PaymentFlow::MERCHANT_MANAGED_DEBIT => "MERCHANT_MANAGED_DEBIT".to_string(),
        PaymentFlow::ADDRESS_VERIFICATION => "ADDRESS_VERIFICATION".to_string(),
        PaymentFlow::TA_FILE => "TA_FILE".to_string(),
        PaymentFlow::PAYMENT_PAGE => "PAYMENT_PAGE".to_string(),
        PaymentFlow::PP_QUICKPAY => "PP_QUICKPAY".to_string(),
        PaymentFlow::PP_RETRY => "PP_RETRY".to_string(),
        PaymentFlow::INAPP_NEW_PAY => "INAPP_NEW_PAY".to_string(),
        PaymentFlow::INAPP_REPEAT_PAY => "INAPP_REPEAT_PAY".to_string(),
        PaymentFlow::PG_EMI => "PG_EMI".to_string(),
        PaymentFlow::SILENT_RETRY => "SILENT_RETRY".to_string(),
        PaymentFlow::FIDO => "FIDO".to_string(),
        PaymentFlow::REFUND => "REFUND".to_string(),
        PaymentFlow::CTP => "CTP".to_string(),
        PaymentFlow::ONE_TIME_PAYMENT => "ONE_TIME_PAYMENT".to_string(),
        PaymentFlow::NO_COST_EMI => "NO_COST_EMI".to_string(),
        PaymentFlow::LOW_COST_EMI => "LOW_COST_EMI".to_string(),
        PaymentFlow::STANDARD_EMI => "STANDARD_EMI".to_string(),
        PaymentFlow::STANDARD_EMI_SPLIT => "STANDARD_EMI_SPLIT".to_string(),
        PaymentFlow::INTERNAL_NO_COST_EMI => "INTERNAL_NO_COST_EMI".to_string(),
        PaymentFlow::INTERNAL_LOW_COST_EMI => "INTERNAL_LOW_COST_EMI".to_string(),
        PaymentFlow::INTERNAL_NO_COST_EMI_SPLIT => "INTERNAL_NO_COST_EMI_SPLIT".to_string(),
        PaymentFlow::INTERNAL_LOW_COST_EMI_SPLIT => "INTERNAL_LOW_COST_EMI_SPLIT".to_string(),
        PaymentFlow::DIRECT_BANK_EMI => "DIRECT_BANK_EMI".to_string(),
        PaymentFlow::REVERSE_PENNY_DROP => "REVERSE_PENNY_DROP".to_string(),
        PaymentFlow::TOPUP => "TOPUP".to_string(),
        PaymentFlow::ON_DEMAND_SPLIT_SETTLEMENT => "ON_DEMAND_SPLIT_SETTLEMENT".to_string(),
        PaymentFlow::CREDIT_CARD_ON_UPI => "CREDIT_CARD_ON_UPI".to_string(),
        PaymentFlow::DECIDER_FALLBACK_DOTP_TO_3DS => "DECIDER_FALLBACK_DOTP_TO_3DS".to_string(),
        PaymentFlow::DECIDER_FALLBACK_NO_3DS_TO_3DS => "DECIDER_FALLBACK_NO_3DS_TO_3DS".to_string(),
        PaymentFlow::PAYMENT_CHANNEL_FALLBACK_DOTP_TO_3DS => {
            "PAYMENT_CHANNEL_FALLBACK_DOTP_TO_3DS".to_string()
        }
        PaymentFlow::PG_FAILURE_FALLBACK_DOTP_TO_3DS => {
            "PG_FAILURE_FALLBACK_DOTP_TO_3DS".to_string()
        }
        PaymentFlow::TOKENIZATION_CONSENT_FALLBACK_DOTP_TO_3DS => {
            "TOKENIZATION_CONSENT_FALLBACK_DOTP_TO_3DS".to_string()
        }
        PaymentFlow::CUSTOMER_FALLBACK_DOTP_TO_3DS => "CUSTOMER_FALLBACK_DOTP_TO_3DS".to_string(),
        PaymentFlow::AUTH_PROVIDER_FALLBACK_3DS2_TO_3DS => {
            "AUTH_PROVIDER_FALLBACK_3DS2_TO_3DS".to_string()
        }
        PaymentFlow::FRM_PREFERENCE_TO_NO_3DS => "FRM_PREFERENCE_TO_NO_3DS".to_string(),
        PaymentFlow::MERCHANT_FALLBACK_3DS2_TO_3DS => "MERCHANT_FALLBACK_3DS2_TO_3DS".to_string(),
        PaymentFlow::MERCHANT_FALLBACK_FIDO_TO_3DS => "MERCHANT_FALLBACK_FIDO_TO_3DS".to_string(),
        PaymentFlow::ORDER_PREFERENCE_FALLBACK_NO_3DS_TO_3DS => {
            "ORDER_PREFERENCE_FALLBACK_NO_3DS_TO_3DS".to_string()
        }
        PaymentFlow::MERCHANT_PREFERENCE_FALLBACK_NO_3DS_TO_3DS => {
            "MERCHANT_PREFERENCE_FALLBACK_NO_3DS_TO_3DS".to_string()
        }
        PaymentFlow::ORDER_PREFERENCE_TO_NO_3DS => "ORDER_PREFERENCE_TO_NO_3DS".to_string(),
        PaymentFlow::MUTUAL_FUND => "MUTUAL_FUND".to_string(),
        PaymentFlow::CROSS_BORDER_PAYMENT => "CROSS_BORDER_PAYMENT".to_string(),
        PaymentFlow::APPLEPAY_TOKEN_DECRYPTION_FLOW => "APPLEPAY_TOKEN_DECRYPTION_FLOW".to_string(),
        PaymentFlow::ONE_TIME_MANDATE => "ONE_TIME_MANDATE".to_string(),
        PaymentFlow::SINGLE_BLOCK_MULTIPLE_DEBIT => "SINGLE_BLOCK_MULTIPLE_DEBIT".to_string(),
    }
}

pub fn text_to_payment_flows(text: String) -> Result<PaymentFlow, ApiError> {
    match text.as_str() {
        "CARD_3DS" => Ok(PaymentFlow::CARD_3DS),
        "CARD_3DS2" => Ok(PaymentFlow::CARD_3DS2),
        "FRICTIONLESS_3DS" => Ok(PaymentFlow::FRICTIONLESS_3DS),
        "CARD_DOTP" => Ok(PaymentFlow::CARD_DOTP),
        "CARD_MOTO" => Ok(PaymentFlow::CARD_MOTO),
        "CARD_NO_3DS" => Ok(PaymentFlow::CARD_NO_3DS),
        "CARD_VIES" => Ok(PaymentFlow::CARD_VIES),
        "ZERO_AUTH" => Ok(PaymentFlow::ZERO_AUTH),
        "CARD_TOKENIZATION" => Ok(PaymentFlow::CARD_TOKENIZATION),
        "CVVLESS" => Ok(PaymentFlow::CVVLESS),
        "DIRECT_DEBIT" => Ok(PaymentFlow::DIRECT_DEBIT),
        "EMANDATE" => Ok(PaymentFlow::EMANDATE),
        "EMI" => Ok(PaymentFlow::EMI),
        "INAPP_DEBIT" => Ok(PaymentFlow::INAPP_DEBIT),
        "MANDATE" => Ok(PaymentFlow::MANDATE),
        "PARTIAL_CAPTURE" => Ok(PaymentFlow::PARTIAL_CAPTURE),
        "PARTIAL_VOID" => Ok(PaymentFlow::PARTIAL_VOID),
        "PARTIAL_PAYMENT" => Ok(PaymentFlow::PARTIAL_PAYMENT),
        "PREAUTH" => Ok(PaymentFlow::PREAUTH),
        "SDKLESS_INTENT" => Ok(PaymentFlow::SDKLESS_INTENT),
        "SPLIT_PAYMENT" => Ok(PaymentFlow::SPLIT_PAYMENT),
        "SPLIT_SETTLEMENT" => Ok(PaymentFlow::SPLIT_SETTLEMENT),
        "WALLET_TOPUP" => Ok(PaymentFlow::WALLET_TOPUP),
        "TPV" => Ok(PaymentFlow::TPV),
        "VISA_CHECKOUT" => Ok(PaymentFlow::VISA_CHECKOUT),
        "AUTO_DISBURSEMENT" => Ok(PaymentFlow::AUTO_DISBURSEMENT),
        "AUTO_USER_REGISTRATION" => Ok(PaymentFlow::AUTO_USER_REGISTRATION),
        "BANK_INSTANT_REFUND" => Ok(PaymentFlow::BANK_INSTANT_REFUND),
        "MANDATE_PREDEBIT_NOTIFICATION_DISABLEMENT" => {
            Ok(PaymentFlow::MANDATE_PREDEBIT_NOTIFICATION_DISABLEMENT)
        }
        "ORDER_AMOUNT_AS_SUBVENTION_AMOUNT" => Ok(PaymentFlow::ORDER_AMOUNT_AS_SUBVENTION_AMOUNT),
        "ORDER_ID_AS_RECON_ID" => Ok(PaymentFlow::ORDER_ID_AS_RECON_ID),
        "PASS_USER_TOKEN_TO_GATEWAY" => Ok(PaymentFlow::PASS_USER_TOKEN_TO_GATEWAY),
        "S2S_FLOW" => Ok(PaymentFlow::S2S_FLOW),
        "SPLIT_SETTLE_ONLY" => Ok(PaymentFlow::SPLIT_SETTLE_ONLY),
        "SUBSCRIPTION_ONLY" => Ok(PaymentFlow::SUBSCRIPTION_ONLY),
        "TPV_ONLY" => Ok(PaymentFlow::TPV_ONLY),
        "TXN_UUID_AS_TR" => Ok(PaymentFlow::TXN_UUID_AS_TR),
        "UPI_INTENT_REGISTRATION" => Ok(PaymentFlow::UPI_INTENT_REGISTRATION),
        "V2_INTEGRATION" => Ok(PaymentFlow::V2_INTEGRATION),
        "V2_LINK_AND_PAY" => Ok(PaymentFlow::V2_LINK_AND_PAY),
        "VPOS2" => Ok(PaymentFlow::VPOS2),
        "OUTAGE" => Ok(PaymentFlow::OUTAGE),
        "SR_BASED_ROUTING" => Ok(PaymentFlow::SR_BASED_ROUTING),
        "ELIMINATION_BASED_ROUTING" => Ok(PaymentFlow::ELIMINATION_BASED_ROUTING),
        "PL_BASED_ROUTING" => Ok(PaymentFlow::PL_BASED_ROUTING),
        "MANDATE_WORKFLOW" => Ok(PaymentFlow::MANDATE_WORKFLOW),
        "ALTID" => Ok(PaymentFlow::ALTID),
        "SURCHARGE" => Ok(PaymentFlow::SURCHARGE),
        "OFFER" => Ok(PaymentFlow::OFFER),
        "CAPTCHA" => Ok(PaymentFlow::CAPTCHA),
        "PAYMENT_COLLECTION_LINK" => Ok(PaymentFlow::PAYMENT_COLLECTION_LINK),
        "AUTO_REFUND" => Ok(PaymentFlow::AUTO_REFUND),
        "PAYMENT_LINK" => Ok(PaymentFlow::PAYMENT_LINK),
        "PAYMENT_FORM" => Ok(PaymentFlow::PAYMENT_FORM),
        "RISK_CHECK" => Ok(PaymentFlow::RISK_CHECK),
        "DYNAMIC_CURRENCY_CONVERSION" => Ok(PaymentFlow::DYNAMIC_CURRENCY_CONVERSION),
        "PART_PAYMENT" => Ok(PaymentFlow::PART_PAYMENT),
        "STANDALONE_AUTHENTICATION" => Ok(PaymentFlow::STANDALONE_AUTHENTICATION),
        "STANDALONE_AUTHORIZATION" => Ok(PaymentFlow::STANDALONE_AUTHORIZATION),
        "STANDALONE_CAPTURE" => Ok(PaymentFlow::STANDALONE_CAPTURE),
        "AUTHN_AUTHZ" => Ok(PaymentFlow::AUTHN_AUTHZ),
        "AUTHZ_CAPTURE" => Ok(PaymentFlow::AUTHZ_CAPTURE),
        "REDIRECT_DEBIT" => Ok(PaymentFlow::REDIRECT_DEBIT),
        "LINK_AND_DEBIT" => Ok(PaymentFlow::LINK_AND_DEBIT),
        "NEW_CARD" => Ok(PaymentFlow::NEW_CARD),
        "NETWORK_TOKEN_CREATED" => Ok(PaymentFlow::NETWORK_TOKEN_CREATED),
        "ISSUER_TOKEN_CREATED" => Ok(PaymentFlow::ISSUER_TOKEN_CREATED),
        "LOCKER_TOKEN_CREATED" => Ok(PaymentFlow::LOCKER_TOKEN_CREATED),
        "SODEXO_TOKEN_CREATED" => Ok(PaymentFlow::SODEXO_TOKEN_CREATED),
        "NETWORK_TOKEN_USED" => Ok(PaymentFlow::NETWORK_TOKEN_USED),
        "ISSUER_TOKEN_USED" => Ok(PaymentFlow::ISSUER_TOKEN_USED),
        "LOCKER_TOKEN_USED" => Ok(PaymentFlow::LOCKER_TOKEN_USED),
        "SODEXO_TOKEN_USED" => Ok(PaymentFlow::SODEXO_TOKEN_USED),
        "PAYU_TOKEN_USED" => Ok(PaymentFlow::PAYU_TOKEN_USED),
        "MANDATE_REGISTER" => Ok(PaymentFlow::MANDATE_REGISTER),
        "MANDATE_REGISTER_DEBIT" => Ok(PaymentFlow::MANDATE_REGISTER_DEBIT),
        "MANDATE_PAYMENT" => Ok(PaymentFlow::MANDATE_PAYMENT),
        "EMANDATE_REGISTER" => Ok(PaymentFlow::EMANDATE_REGISTER),
        "EMANDATE_REGISTER_DEBIT" => Ok(PaymentFlow::EMANDATE_REGISTER_DEBIT),
        "EMANDATE_PAYMENT" => Ok(PaymentFlow::EMANDATE_PAYMENT),
        "SI_HUB" => Ok(PaymentFlow::SI_HUB),
        "TPV_EMANDATE" => Ok(PaymentFlow::TPV_EMANDATE),
        "COLLECT" => Ok(PaymentFlow::COLLECT),
        "INTENT" => Ok(PaymentFlow::INTENT),
        "INAPP" => Ok(PaymentFlow::INAPP),
        "QR" => Ok(PaymentFlow::QR),
        "PUSH_PAY" => Ok(PaymentFlow::PUSH_PAY),
        "INSTANT_REFUND" => Ok(PaymentFlow::INSTANT_REFUND),
        "ASYNC" => Ok(PaymentFlow::ASYNC),
        "DOTP" => Ok(PaymentFlow::DOTP),
        "MERCHANT_MANAGED_DEBIT" => Ok(PaymentFlow::MERCHANT_MANAGED_DEBIT),
        "ADDRESS_VERIFICATION" => Ok(PaymentFlow::ADDRESS_VERIFICATION),
        "TA_FILE" => Ok(PaymentFlow::TA_FILE),
        "PAYMENT_PAGE" => Ok(PaymentFlow::PAYMENT_PAGE),
        "PP_QUICKPAY" => Ok(PaymentFlow::PP_QUICKPAY),
        "PP_RETRY" => Ok(PaymentFlow::PP_RETRY),
        "INAPP_NEW_PAY" => Ok(PaymentFlow::INAPP_NEW_PAY),
        "INAPP_REPEAT_PAY" => Ok(PaymentFlow::INAPP_REPEAT_PAY),
        "PG_EMI" => Ok(PaymentFlow::PG_EMI),
        "SILENT_RETRY" => Ok(PaymentFlow::SILENT_RETRY),
        "FIDO" => Ok(PaymentFlow::FIDO),
        "REFUND" => Ok(PaymentFlow::REFUND),
        "CTP" => Ok(PaymentFlow::CTP),
        "ONE_TIME_PAYMENT" => Ok(PaymentFlow::ONE_TIME_PAYMENT),
        "NO_COST_EMI" => Ok(PaymentFlow::NO_COST_EMI),
        "LOW_COST_EMI" => Ok(PaymentFlow::LOW_COST_EMI),
        "STANDARD_EMI" => Ok(PaymentFlow::STANDARD_EMI),
        "STANDARD_EMI_SPLIT" => Ok(PaymentFlow::STANDARD_EMI_SPLIT),
        "INTERNAL_NO_COST_EMI" => Ok(PaymentFlow::INTERNAL_NO_COST_EMI),
        "INTERNAL_LOW_COST_EMI" => Ok(PaymentFlow::INTERNAL_LOW_COST_EMI),
        "INTERNAL_NO_COST_EMI_SPLIT" => Ok(PaymentFlow::INTERNAL_NO_COST_EMI_SPLIT),
        "INTERNAL_LOW_COST_EMI_SPLIT" => Ok(PaymentFlow::INTERNAL_LOW_COST_EMI_SPLIT),
        "DIRECT_BANK_EMI" => Ok(PaymentFlow::DIRECT_BANK_EMI),
        "REVERSE_PENNY_DROP" => Ok(PaymentFlow::REVERSE_PENNY_DROP),
        "TOPUP" => Ok(PaymentFlow::TOPUP),
        "ON_DEMAND_SPLIT_SETTLEMENT" => Ok(PaymentFlow::ON_DEMAND_SPLIT_SETTLEMENT),
        "CREDIT_CARD_ON_UPI" => Ok(PaymentFlow::CREDIT_CARD_ON_UPI),
        "DECIDER_FALLBACK_DOTP_TO_3DS" => Ok(PaymentFlow::DECIDER_FALLBACK_DOTP_TO_3DS),
        "DECIDER_FALLBACK_NO_3DS_TO_3DS" => Ok(PaymentFlow::DECIDER_FALLBACK_NO_3DS_TO_3DS),
        "PAYMENT_CHANNEL_FALLBACK_DOTP_TO_3DS" => {
            Ok(PaymentFlow::PAYMENT_CHANNEL_FALLBACK_DOTP_TO_3DS)
        }
        "PG_FAILURE_FALLBACK_DOTP_TO_3DS" => Ok(PaymentFlow::PG_FAILURE_FALLBACK_DOTP_TO_3DS),
        "TOKENIZATION_CONSENT_FALLBACK_DOTP_TO_3DS" => {
            Ok(PaymentFlow::TOKENIZATION_CONSENT_FALLBACK_DOTP_TO_3DS)
        }
        "CUSTOMER_FALLBACK_DOTP_TO_3DS" => Ok(PaymentFlow::CUSTOMER_FALLBACK_DOTP_TO_3DS),
        "AUTH_PROVIDER_FALLBACK_3DS2_TO_3DS" => Ok(PaymentFlow::AUTH_PROVIDER_FALLBACK_3DS2_TO_3DS),
        "FRM_PREFERENCE_TO_NO_3DS" => Ok(PaymentFlow::FRM_PREFERENCE_TO_NO_3DS),
        "MERCHANT_FALLBACK_3DS2_TO_3DS" => Ok(PaymentFlow::MERCHANT_FALLBACK_3DS2_TO_3DS),
        "MERCHANT_FALLBACK_FIDO_TO_3DS" => Ok(PaymentFlow::MERCHANT_FALLBACK_FIDO_TO_3DS),
        "ORDER_PREFERENCE_FALLBACK_NO_3DS_TO_3DS" => {
            Ok(PaymentFlow::ORDER_PREFERENCE_FALLBACK_NO_3DS_TO_3DS)
        }
        "MERCHANT_PREFERENCE_FALLBACK_NO_3DS_TO_3DS" => {
            Ok(PaymentFlow::MERCHANT_PREFERENCE_FALLBACK_NO_3DS_TO_3DS)
        }
        "ORDER_PREFERENCE_TO_NO_3DS" => Ok(PaymentFlow::ORDER_PREFERENCE_TO_NO_3DS),
        "MUTUAL_FUND" => Ok(PaymentFlow::MUTUAL_FUND),
        "CROSS_BORDER_PAYMENT" => Ok(PaymentFlow::CROSS_BORDER_PAYMENT),
        "APPLEPAY_TOKEN_DECRYPTION_FLOW" => Ok(PaymentFlow::APPLEPAY_TOKEN_DECRYPTION_FLOW),
        "ONE_TIME_MANDATE" => Ok(PaymentFlow::ONE_TIME_MANDATE),
        "SINGLE_BLOCK_MULTIPLE_DEBIT" => Ok(PaymentFlow::SINGLE_BLOCK_MULTIPLE_DEBIT),
        _ => Err(ApiError::ParsingError("Invalid Payment Flow")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum Purpose {
    AFFORDABILITY,
    COMPLIANT,
    INTERNAL_CONFIG,
    SR_IMPROVEMENT,
    UEX_IMPROVEMENT,
    PAYMENT,
    SECURITY,
    OPTIMIZATION,
    NO_CODE_PAYMENT,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum Category {
    PREMIUM,
    STANDARD,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum ControlLevel {
    GATEWAY,
    MERCHANT,
    MERCHANT_GATEWAY,
    TENANT,
    TENANT_GATEWAY,
    TRACKING_ONLY,
    MERCHANT_GATEWAY_ACCOUNT,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum FlowStatus {
    BETA,
    LIVE,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum MicroPaymentFlowType {
    ARRAY,
    BOOLEAN,
    DOUBLE,
    OBJECT,
    STRING,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum UiAccessMode {
    READ_ONLY,
    HIDDEN,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddressVerification {
    #[serde(rename = "collectAvsInfo")]
    pub collectAvsInfo: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FlowLevel {
  PaymentFlow,
  GatewayPaymentFlow,
  GatewayPaymentMethodFlow,
  MerchantGatewayPaymentMethodFlow,
}  

pub fn text_to_flow_level(text: String) -> Result<FlowLevel, ApiError> {
    match text.as_str() {
        "PAYMENT_FLOW" => Ok(FlowLevel::PaymentFlow),
        "GATEWAY_PAYMENT_FLOW" => Ok(FlowLevel::GatewayPaymentFlow),
        "GATEWAY_PAYMENT_METHOD_FLOW" => Ok(FlowLevel::GatewayPaymentMethodFlow),
        "MERCHANT_GATEWAY_PAYMENT_METHOD_FLOW" => Ok(FlowLevel::MerchantGatewayPaymentMethodFlow),
        _ => Err(ApiError::ParsingError("Invalid FlowLevel")),
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]


pub struct FlowLevelId {
    pub flowLevelId: String,
}

pub fn to_flow_level_id(id: String) -> FlowLevelId {
    FlowLevelId {
        flowLevelId: id,
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MicroPaymentFlowName {
  RegisterMaxAmount,
  SupportedFrequencies,
  BankAccountDetailsSupportMode,
}

pub fn text_to_micro_payment_flow_name(text: String) -> Result<MicroPaymentFlowName, ApiError> {
    match text.as_str() {
        "REGISTER_MAX_AMOUNT" => Ok(MicroPaymentFlowName::RegisterMaxAmount),
        "SUPPORTED_FREQUENCIES" => Ok(MicroPaymentFlowName::SupportedFrequencies),
        "BANK_ACCOUNT_DETAILS_SUPPORT_MODE" => Ok(MicroPaymentFlowName::BankAccountDetailsSupportMode),
        _ => Err(ApiError::ParsingError("Invalid MicroPaymentFlowName")),
    }
}

pub fn text_to_micro_payment_flow_type(text: String) -> Result<MicroPaymentFlowType, ApiError> {
    match text.as_str() {
        "ARRAY" => Ok(MicroPaymentFlowType::ARRAY),
        "BOOLEAN" => Ok(MicroPaymentFlowType::BOOLEAN),
        "DOUBLE" => Ok(MicroPaymentFlowType::DOUBLE),
        "OBJECT" => Ok(MicroPaymentFlowType::OBJECT),
        "STRING" => Ok(MicroPaymentFlowType::STRING),
        _ => Err(ApiError::ParsingError("Invalid MicroPaymentFlowType")),
    }
}

pub fn micro_payment_flow_name_to_text(mpfn: &MicroPaymentFlowName) -> String {
    match mpfn {
        MicroPaymentFlowName::RegisterMaxAmount => "REGISTER_MAX_AMOUNT".to_string(),
        MicroPaymentFlowName::SupportedFrequencies => "SUPPORTED_FREQUENCIES".to_string(),
        MicroPaymentFlowName::BankAccountDetailsSupportMode => "BANK_ACCOUNT_DETAILS_SUPPORT_MODE".to_string(),
    }
}

pub fn flow_level_to_text(mpfn: &FlowLevel) -> String {
    match mpfn {
        FlowLevel::PaymentFlow => "PAYMENT_FLOW".to_string(),
        FlowLevel::GatewayPaymentFlow => "GATEWAY_PAYMENT_FLOW".to_string(),
        FlowLevel::GatewayPaymentMethodFlow => "GATEWAY_PAYMENT_METHOD_FLOW".to_string(),
        FlowLevel::MerchantGatewayPaymentMethodFlow => "MERCHANT_GATEWAY_PAYMENT_METHOD_FLOW".to_string(),
    }
}