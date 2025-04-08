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
use crate::types::currency::Currency;
use crate::types::customer as ETC;
use crate::types::gateway as ETG;
use crate::types::merchant::id::to_merchant_id;
use crate::types::merchant::merchant_gateway_account as ETMGA;
use crate::types::money::internal::Money;
use crate::types::order as ETO;
use crate::types::order::id as ETOID;
use crate::types::order::id::to_order_id;
use crate::types::order::udfs::UDFs;
use crate::types::order_metadata_v2 as ETOMV2;
use crate::types::payment::payment_method as ETP;
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
pub fn parseFromApiOrderReference(apiType: T::ApiOrderReference) -> Option<ETO::Order> {
    let udfs = parseUDFs(&apiType)?;

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
        dateCreated: apiType.dateCreated,
        merchantId: apiType.merchantId.map(to_merchant_id)?,
        orderId: apiType.orderId.map(to_order_id)?,
        status: ETO::OrderStatus::from_text(apiType.status)?,
        customerId: apiType.customerId.map(ETC::customer_id_text),
        description: apiType.description,
        udfs,
        preferredGateway: apiType
            .preferredGateway
            .as_deref()
            .map(ETG::text_to_gateway)?
            .ok(),
        productId: apiType.productId.map(ETOID::to_product_id),
        orderType: ETO::OrderType::from_text(apiType.orderType?)?,
        internalMetadata: apiType.internalMetadata,
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

fn parseUDFs(apiType: &T::ApiOrderReference) -> Option<UDFs> {
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

pub fn parseFromApiOrderMetadataV2(
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

pub fn parseFromApiTxnDetail(apiType: T::ApiTxnDetail) -> Option<ETTD::TxnDetail> {
    Some(ETTD::TxnDetail {
        id: apiType
            .id
            .and_then(|id_str| id_str.parse::<i64>().ok())
            .map(ETTD::to_txn_detail_id)?,
        dateCreated: apiType.dateCreated?,
        orderId: ETOID::to_order_id(apiType.orderId),
        status: ETTD::TxnStatus::from_text(apiType.status)?,
        txnId: ETTID::to_transaction_id(apiType.txnId),
        txnType: apiType.txnType,
        addToLocker: apiType.addToLocker.unwrap_or(false),
        merchantId: apiType.merchantId.map(to_merchant_id)?,
        gateway: apiType.gateway.as_deref().map(ETG::text_to_gateway)?.ok(),
        expressCheckout: apiType.expressCheckout.unwrap_or(false),
        isEmi: apiType.isEmi.unwrap_or(false),
        emiBank: apiType.emiBank,
        emiTenure: apiType.emiTenure,
        txnUuid: apiType.txnUuid?,
        merchantGatewayAccountId: apiType
            .merchantGatewayAccountId
            .map(ETMGA::to_merchant_gw_acc_id),
        netAmount: apiType.netAmount.map(Money::from_double)?,
        txnAmount: apiType.txnAmount.map(Money::from_double)?,
        txnObjectType: apiType
            .txnObjectType
            .and_then(ETTD::TxnObjectType::from_text)?,
        sourceObject: apiType.sourceObject,
        sourceObjectId: apiType.sourceObjectId.map(SO::to_source_object_id),
        currency: apiType
            .currency
            .as_deref()
            .map(Currency::text_to_curr)?
            .ok()?,
        surchargeAmount: apiType.surchargeAmount.map(Money::from_double),
        taxAmount: apiType.taxAmount.map(Money::from_double),
        internalMetadata: apiType.internalMetadata,
        metadata: apiType.metadata,
        offerDeductionAmount: apiType.offerDeductionAmount.map(Money::from_double),
        internalTrackingInfo: apiType.internalTrackingInfo,
        partitionKey: apiType.partitionKey,
        txnAmountBreakup: apiType.txnAmountBreakup.as_deref().and_then(|breakup_str| {
            serde_json::from_str::<Vec<ETTD::TransactionCharge>>(breakup_str).ok()
        }),
    })
}

pub fn parseFromApiTxnCardInfo(apiType: T::ApiTxnCardInfo) -> Option<ETCa::TxnCardInfo> {
    Some(ETCa::TxnCardInfo {
        id: apiType
            .id
            .and_then(|id_str| id_str.parse::<i64>().ok())
            .map(ETCa::to_txn_card_info_pid)?,
        // txnId: ETTID::to_transaction_id(apiType.txnId),
        card_isin: apiType.cardIsin,
        cardIssuerBankName: apiType.cardIssuerBankName,
        cardSwitchProvider: apiType.cardSwitchProvider.map(Secret::new),
        card_type: apiType.cardType.as_deref().map(Ca::to_card_type)?.ok(),
        // cardLastFourDigits: apiType.cardLastFourDigits,
        nameOnCard: apiType.nameOnCard.map(Secret::new),
        // cardFingerprint: apiType.cardFingerprint,
        // cardReferenceId: apiType.cardReferenceId,
        // txnDetailId: apiType.txnDetailId.and_then(|id_str| id_str.parse::<i64>().ok()).map(ETTD::to_txn_detail_id)?,
        dateCreated: apiType.dateCreated?,
        paymentMethodType: apiType
            .paymentMethodType
            .map(ETP::text_to_payment_method_type)?
            .ok()?,
        paymentMethod: apiType.paymentMethod?,
        // cardGlobalFingerprint: apiType.cardGlobalFingerprint,
        paymentSource: apiType.paymentSource,
        authType: apiType
            .authType
            .as_deref()
            .map(ETCa::text_to_auth_type)?
            .ok(),
        partitionKey: apiType.partitionKey,
    })
}

pub fn parseApiDeciderRequest(
    apiType: T::ApiDeciderRequest,
) -> Result<T::DomainDeciderRequestForApiCall, error::ApiError> {
    match parseApiDeciderRequestO(apiType) {
        Some(domainDeciderRequest) => Ok(domainDeciderRequest),
        None => Err(error::ApiError::ParsingError(
            "Failed to parse ApiDeciderRequest",
        )),
    }
}

pub fn parseApiDeciderRequestO(
    apiType: T::ApiDeciderRequest,
) -> Option<T::DomainDeciderRequestForApiCall> {
    Some(T::DomainDeciderRequestForApiCall {
        orderReference: parseFromApiOrderReference(apiType.orderReference)?,
        orderMetadata: parseFromApiOrderMetadataV2(apiType.orderMetadata)?,
        txnDetail: parseFromApiTxnDetail(apiType.txnDetail)?,
        txnCardInfo: parseFromApiTxnCardInfo(apiType.txnCardInfo)?,
        card_token: apiType.card_token,
        txn_type: apiType.txn_type,
        should_create_mandate: apiType.should_create_mandate,
        enforce_gateway_list: apiType.enforce_gateway_list,
        priority_logic_script: apiType.priority_logic_script,
    })
}
