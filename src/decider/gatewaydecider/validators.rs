// use eulerhs::prelude::*;
// use control::category::Category;
// use data::aeson::encode_pretty::encodePretty;
// use types::currency::textToCurr;
// use types::money::fromDouble;
// use ghc::typelits::KnownSymbol;
// use optics::core::preview;
// use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, defaulting, liftEither, liftPure, mandated, nonEmptyText, nonNegative, parseField, project, secret};
// use data::byte_string::lazy as BSL;
// use data::int_map::strict as IM;
// use data::text as T;
// use data::text::encoding as TE;
// use gatewaydecider::types as T;
// use types::card as ETCa;
// use types::customer as ETC;
// use types::gateway as ETG;
// use types::mandate as ETMa;
// use types::merchant as ETM;
// use types::order as ETO;
// use types::order_address as ETOA;
// use types::order_metadata_v2 as ETOMV2;
// use types::payment as ETP;
// use types::txn_detail as ETTD;
// use type::reflection as TR;
// use types::account as RI;
// use types::locker as LI;
use super::types as T;
use crate::error;
use crate::types::card::card_type as Ca;
use crate::types::card::txn_card_info as ETCa;
use crate::types::country::country_iso::CountryISO2;
use crate::types::currency::Currency;
use crate::types::customer as ETC;
use crate::types::merchant::id::to_merchant_id;
use crate::types::merchant::merchant_gateway_account as ETMGA;
use crate::types::money::internal::Money;
use crate::types::order as ETO;
use crate::types::order::id as ETOID;
use crate::types::order::id::to_order_id;
use crate::types::order::udfs::UDFs;
use crate::types::order_metadata_v2 as ETOMV2;
use crate::types::source_object_id as SO;
use crate::types::transaction::id as ETTID;
use crate::types::txn_details::types as ETTD;
use masking::Secret;
use serde_json::Value as AValue;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::option::Option;
use std::string::String;
use std::vec::Vec;

// pub fn parseFromApiOrderReference(apiType: T::ApiOrderReference) -> Option<String>{
pub fn parse_from_api_order_reference(apiType: T::ApiOrderReference) -> Option<ETO::Order> {
    let udfs = parse_udfs(&apiType)?;

    Some(ETO::Order {
        id: apiType
            .id
            .and_then(|id_str| id_str.parse::<i64>().ok())
            .map(ETOID::to_order_prim_id)?,
        amount: apiType.amount.map(Money::from_double)?,
        currency: apiType
            .currency
            .as_deref()
            .map(Currency::text_to_curr)?
            .ok()?,
        date_created: apiType.dateCreated,
        merchant_id: apiType.merchantId.map(to_merchant_id)?,
        order_id: apiType.orderId.map(to_order_id)?,
        status: ETO::OrderStatus::from_text(apiType.status)?,
        customer_id: apiType.customerId.map(ETC::customer_id_text),
        description: apiType.description,
        udfs,
        preferred_gateway: apiType.preferredGateway,
        product_id: apiType.productId.map(ETOID::to_product_id),
        order_type: ETO::OrderType::from_text(apiType.orderType?)?,
        internal_metadata: apiType.internalMetadata,
        metadata: apiType.metadata,
    })
}

// pub fn parseFromApiOrderReference(apiType: T::ApiOrderReference) -> Parsed<ETO::Order> {
//     ETO::Order {
//         id: go(project("id").and_then(mandated).and_then(parseString::<_, i64>()).and_then(liftPure(ETO::OrderPrimId))),
//         amount: go(project("amount").and_then(mandated).and_then(liftPure(fromDouble))),
//         currency: go(project("currency").and_then(mandated).and_then(textToCurr)),
//         dateCreated: go(project("dateCreated")),
//         merchantId: go(project("merchantId").and_then(mandated).and_then(ETM::toMerchantId)),
//         orderId: go(project("orderId").and_then(mandated).and_then(ETO::toOrderId)),
//         status: go(project("status").and_then(ETO::textToOrderStatus)),
//         customerId: go(project("customerId").and_then(liftPure(|x| x.and_then(preview(ETC::customerIdText))))),
//         description: go(project("description")),
//         udfs: parseUDFs(),
//         preferredGateway: go(project("preferredGateway").and_then(around(ETG::textToGateway))),
//         productId: go(project("productId").and_then(around(ETO::toProductId))),
//         orderType: parseOrderType(),
//         internalMetadata: go(project("internalMetadata")),
//         metadata: go(project("metadata")),
//     }
// }

// fn go<'a, field, b>(step: Step<'a, field, T::ApiOrderReference, b>) -> Parsed<b>
// where
//     field: KnownSymbol,
// {
//     parseField(apiType, step)
impl FromIterator<(i32, String)> for UDFs {
    fn from_iter<T: IntoIterator<Item = (i32, String)>>(iter: T) -> Self {
        let mut udfs = Self(HashMap::new());
        for (key, value) in iter {
            udfs.0.insert(key, value); // Assuming UDFs has an insert method
        }
        udfs
    }
}

fn parse_udfs(apiType: &T::ApiOrderReference) -> Option<UDFs> {
    Some(UDFs::from_iter(
        udfLine(apiType)
            .into_iter()
            .enumerate()
            .filter_map(|(i, parsed)| parsed.map(|p| (i as i32 + 1, p))),
    ))
}

fn udfLine(api_type: &T::ApiOrderReference) -> Vec<Option<String>> {
    vec![
        api_type.udf1.clone(),
        api_type.udf2.clone(),
        api_type.udf3.clone(),
        api_type.udf4.clone(),
        api_type.udf5.clone(),
        api_type.udf6.clone(),
        api_type.udf7.clone(),
        api_type.udf8.clone(),
        api_type.udf9.clone(),
        api_type.udf10.clone(),
    ]
}

// fn udfLine() -> Vec<Parsed<Option<T::Text>>> {
//     vec![
//         go(project("udf1")),
//         go(project("udf2")),
//         go(project("udf3")),
//         go(project("udf4")),
//         go(project("udf5")),
//         go(project("udf6")),
//         go(project("udf7")),
//         go(project("udf8")),
//         go(project("udf9")),
//         go(project("udf10")),
//     ]
// }

// fn projectOT() -> Parsed<Option<ETO::OrderType>> {
//     go(project("orderType").and_then(around(ETO::textToOrderType)))
// }

// fn projectMF() -> Parsed<ETMa::MandateFeature> {
//     go(project("mandateFeature").and_then(around(ETMa::textToMandateFeature)).and_then(defaulting(ETMa::Disabled)))
// }

// fn determineOT(orderType: Option<ETO::OrderType>, mandateFeature: ETMa::MandateFeature) -> ETO::OrderType {
//     match orderType {
//         Some(ot) => ot,
//         None => match mandateFeature {
//             ETMa::Required => ETO::MandateRegister,
//             _ => ETO::OrderPayment,
//         },
//     }
// }

fn convert_metadata_to_string(metadata: Option<HashMap<String, AValue>>) -> Option<String> {
    metadata.map(|map| {
        map.into_iter()
            .map(|(key, value)| format!("{}: {}", key, value))
            .collect::<Vec<String>>()
            .join(", ")
    })
}

pub fn parse_from_api_order_metadata_v2(
    apiType: T::ApiOrderMetadataV2,
) -> Option<ETOMV2::OrderMetadataV2> {
    Some(ETOMV2::OrderMetadataV2 {
        id: apiType
            .id
            .and_then(|id_str| id_str.parse::<i64>().ok())
            .map(ETOMV2::to_order_metadata_v2_pid)?,
        date_created: apiType.dateCreated,
        last_updated: apiType.lastUpdated,
        metadata: convert_metadata_to_string(apiType.metadata),
        order_reference_id: apiType.orderReferenceId.parse::<i64>().ok()?,
        ip_address: apiType.ipAddress,
        partition_key: apiType.partitionKey,
    })
}

pub fn parse_from_api_txn_detail(apiType: T::ApiTxnDetail) -> Option<ETTD::TxnDetail> {
    Some(ETTD::TxnDetail {
        id: apiType
            .id
            .and_then(|id_str| id_str.parse::<i64>().ok())
            .map(ETTD::to_txn_detail_id)?,
        date_created: apiType.dateCreated?,
        order_id: ETOID::to_order_id(apiType.orderId),
        status: ETTD::TxnStatus::from_text(apiType.status)?,
        txn_id: ETTID::to_transaction_id(apiType.txnId),
        txn_type: Some(apiType.txnType),
        add_to_locker: Some(apiType.addToLocker.unwrap_or(false)),
        merchant_id: apiType.merchantId.map(to_merchant_id)?,
        gateway: apiType.gateway,
        express_checkout: Some(apiType.expressCheckout.unwrap_or(false)),
        is_emi: Some(apiType.isEmi.unwrap_or(false)),
        emi_bank: apiType.emiBank,
        emi_tenure: apiType.emiTenure,
        txn_uuid: apiType.txnUuid.unwrap_or_default(),
        merchant_gateway_account_id: apiType
            .merchantGatewayAccountId
            .map(ETMGA::to_merchant_gw_acc_id),
        net_amount: apiType.netAmount.map(Money::from_double),
        txn_amount: apiType.txnAmount.map(Money::from_double),
        txn_object_type: apiType
            .txnObjectType
            .and_then(ETTD::TxnObjectType::from_text),
        source_object: apiType.sourceObject,
        source_object_id: apiType.sourceObjectId.map(SO::to_source_object_id),
        currency: apiType
            .currency
            .as_deref()
            .map(Currency::text_to_curr)?
            .ok()?,
        country: apiType
            .country
            .as_deref()
            .map(CountryISO2::text_to_country)?
            .ok(),
        surcharge_amount: apiType.surchargeAmount.map(Money::from_double),
        tax_amount: apiType.taxAmount.map(Money::from_double),
        internal_metadata: apiType.internalMetadata.map(Secret::new),
        metadata: apiType.metadata.map(Secret::new),
        offer_deduction_amount: apiType.offerDeductionAmount.map(Money::from_double),
        internal_tracking_info: apiType.internalTrackingInfo,
        partition_key: apiType.partitionKey,
        txn_amount_breakup: apiType.txnAmountBreakup.as_deref().and_then(|breakup_str| {
            serde_json::from_str::<Vec<ETTD::TransactionCharge>>(breakup_str).ok()
        }),
    })
}

pub fn parse_from_api_txn_card_info(apiType: T::ApiTxnCardInfo) -> Option<ETCa::TxnCardInfo> {
    Some(ETCa::TxnCardInfo {
        id: apiType
            .id
            .and_then(|id_str| id_str.parse::<i64>().ok())
            .map(ETCa::to_txn_card_info_pid)?,
        // txnId: ETTID::to_transaction_id(apiType.txnId),
        card_isin: apiType.cardIsin,
        card_issuer_bank_name: apiType.cardIssuerBankName,
        card_switch_provider: apiType.cardSwitchProvider.map(Secret::new),
        card_type: apiType.cardType.as_deref().map(Ca::to_card_type)?.ok(),
        // cardLastFourDigits: apiType.cardLastFourDigits,
        name_on_card: apiType.nameOnCard.map(Secret::new),
        // cardFingerprint: apiType.cardFingerprint,
        // cardReferenceId: apiType.cardReferenceId,
        // txnDetailId: apiType.txnDetailId.and_then(|id_str| id_str.parse::<i64>().ok()).map(ETTD::to_txn_detail_id)?,
        date_created: apiType.dateCreated?,
        payment_method_type: apiType.paymentMethodType?,
        payment_method: apiType.paymentMethod?,
        // cardGlobalFingerprint: apiType.cardGlobalFingerprint,
        payment_source: apiType.paymentSource,
        auth_type: apiType
            .authType
            .as_deref()
            .map(ETCa::text_to_auth_type)?
            .ok(),
        partition_key: apiType.partitionKey,
    })
}

pub fn parse_api_decider_request(
    apiType: T::ApiDeciderRequest,
) -> Result<T::DomainDeciderRequestForApiCall, error::ApiError> {
    match parse_api_decider_request_o(apiType) {
        Some(domainDeciderRequest) => Ok(domainDeciderRequest),
        None => Err(error::ApiError::ParsingError(
            "Failed to parse ApiDeciderRequest",
        )),
    }
}

pub fn parse_api_decider_request_o(
    apiType: T::ApiDeciderRequest,
) -> Option<T::DomainDeciderRequestForApiCall> {
    Some(T::DomainDeciderRequestForApiCall {
        orderReference: parse_from_api_order_reference(apiType.orderReference)?,
        orderMetadata: parse_from_api_order_metadata_v2(apiType.orderMetadata)?,
        txnDetail: parse_from_api_txn_detail(apiType.txnDetail)?,
        txnCardInfo: parse_from_api_txn_card_info(apiType.txnCardInfo)?,
        card_token: apiType.card_token,
        txn_type: apiType.txn_type,
        should_create_mandate: apiType.should_create_mandate,
        enforce_gateway_list: apiType.enforce_gateway_list,
        priority_logic_script: apiType.priority_logic_script,
    })
}
