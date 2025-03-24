use eulerhs::prelude::*;
use control::category::Category;
use data::aeson::encode_pretty::encodePretty;
use types::currency::textToCurr;
use types::money::fromDouble;
use ghc::typelits::KnownSymbol;
use optics::core::preview;
use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, defaulting, liftEither, liftPure, mandated, nonEmptyText, nonNegative, parseField, project, secret};
use data::byte_string::lazy as BSL;
use data::int_map::strict as IM;
use data::text as T;
use data::text::encoding as TE;
use gatewaydecider::types as T;
use types::card as ETCa;
use types::customer as ETC;
use types::gateway as ETG;
use types::mandate as ETMa;
use types::merchant as ETM;
use types::order as ETO;
use types::order_address as ETOA;
use types::order_metadata_v2 as ETOMV2;
use types::payment as ETP;
use types::txn_detail as ETTD;
use type::reflection as TR;
use types::account as RI;
use types::locker as LI;
use std::option::Option;
use std::vec::Vec;
use std::string::String;

pub fn parseString<'a, ctx, A>(s: &'a str) -> Step<ctx, String, A>
where
    A: std::str::FromStr + std::fmt::Debug + 'static,
{
    liftEither(move |s| {
        s.parse::<A>()
            .map_err(|_| ParsingErrorType::UnexpectedTextValue(format!("{:?}", std::any::type_name::<A>()), T::from(s)))
    })
}

pub fn parseFromApiOrderReference(apiType: T::ApiOrderReference) -> Parsed<ETO::Order> {
    ETO::Order {
        id: go(project("id").and_then(mandated).and_then(parseString::<_, i64>()).and_then(liftPure(ETO::OrderPrimId))),
        amount: go(project("amount").and_then(mandated).and_then(liftPure(fromDouble))),
        currency: go(project("currency").and_then(mandated).and_then(textToCurr)),
        dateCreated: go(project("dateCreated")),
        merchantId: go(project("merchantId").and_then(mandated).and_then(ETM::toMerchantId)),
        orderId: go(project("orderId").and_then(mandated).and_then(ETO::toOrderId)),
        status: go(project("status").and_then(ETO::textToOrderStatus)),
        customerId: go(project("customerId").and_then(liftPure(|x| x.and_then(preview(ETC::customerIdText))))),
        description: go(project("description")),
        udfs: parseUDFs(),
        preferredGateway: go(project("preferredGateway").and_then(around(ETG::textToGateway))),
        productId: go(project("productId").and_then(around(ETO::toProductId))),
        orderType: parseOrderType(),
        internalMetadata: go(project("internalMetadata")),
        metadata: go(project("metadata")),
    }
}

fn go<'a, field, b>(step: Step<'a, field, T::ApiOrderReference, b>) -> Parsed<b>
where
    field: KnownSymbol,
{
    parseField(apiType, step)
}

fn parseUDFs() -> Parsed<ETO::UDFs> {
    ETO::UDFs(IM::from_iter(
        udfLine()
            .into_iter()
            .enumerate()
            .filter_map(|(i, parsed)| parsed.map(|p| (i as i32 + 1, p)))
    ))
}

fn udfLine() -> Vec<Parsed<Option<T::Text>>> {
    vec![
        go(project("udf1")),
        go(project("udf2")),
        go(project("udf3")),
        go(project("udf4")),
        go(project("udf5")),
        go(project("udf6")),
        go(project("udf7")),
        go(project("udf8")),
        go(project("udf9")),
        go(project("udf10")),
    ]
}

fn parseOrderType() -> Parsed<ETO::OrderType> {
    match (projectOT(), projectMF()) {
        (Parsed::Failed(errs), _) | (_, Parsed::Failed(errs)) => Parsed::Failed(errs),
        (Parsed::Result(Some(ot)), _) => Parsed::Result(ot),
        (Parsed::Result(None), Parsed::Result(mmf)) => Parsed::Result(determineOT(None, mmf)),
    }
}

fn projectOT() -> Parsed<Option<ETO::OrderType>> {
    go(project("orderType").and_then(around(ETO::textToOrderType)))
}

fn projectMF() -> Parsed<ETMa::MandateFeature> {
    go(project("mandateFeature").and_then(around(ETMa::textToMandateFeature)).and_then(defaulting(ETMa::Disabled)))
}

fn determineOT(orderType: Option<ETO::OrderType>, mandateFeature: ETMa::MandateFeature) -> ETO::OrderType {
    match orderType {
        Some(ot) => ot,
        None => match mandateFeature {
            ETMa::Required => ETO::MandateRegister,
            _ => ETO::OrderPayment,
        },
    }
}

pub fn parseFromApiOrderMetadataV2(apiType: T::ApiOrderMetadataV2) -> Parsed<ETOMV2::OrderMetadataV2> {
    ETOMV2::OrderMetadataV2 {
        id: go(project("id").and_then(mandated).and_then(parseString::<_, i64>()).and_then(ETOMV2::toOrderMetadataV2PId)),
        dateCreated: go(project("dateCreated")),
        lastUpdated: go(project("lastUpdated")),
        metadata: go(project("metadata").and_then(around(liftPure(|x| TE::decode_utf8(BSL::to_vec(&x).as_slice()).unwrap())))),
        orderReferenceId: go(project("orderReferenceId").and_then(parseString::<_, i64>()).and_then(nonNegative).and_then(ETO::toOrderPrimId)),
        ipAddress: go(project("ipAddress")),
        partitionKey: go(project("partitionKey")),
    }
}

pub fn parseFromApiTxnDetail(apiType: T::ApiTxnDetail) -> Parsed<ETTD::TxnDetail> {
    ETTD::TxnDetail {
        id: go(project("id").and_then(mandated).and_then(parseString::<_, i64>()).and_then(nonNegative).and_then(ETTD::toTxnDetailId)),
        dateCreated: go(project("dateCreated").and_then(mandated)),
        orderId: go(project("orderId").and_then(ETO::toOrderId)),
        status: go(project("status").and_then(ETTD::toTxnStatus)),
        txnId: go(project("txnId").and_then(ETTD::toTransactionId)),
        txnType: go(project("txnType").and_then(nonEmptyText)),
        addToLocker: go(project("addToLocker").and_then(defaulting(false))),
        merchantId: go(project("merchantId").and_then(mandated).and_then(ETM::toMerchantId)),
        gateway: go(project("gateway").and_then(around(ETG::textToGateway))),
        expressCheckout: go(project("expressCheckout").and_then(defaulting(false))),
        isEmi: go(project("isEmi").and_then(defaulting(false))),
        emiBank: go(project("emiBank")),
        emiTenure: go(project("emiTenure")),
        txnUuid: go(project("txnUuid").and_then(mandated)),
        merchantGatewayAccountId: go(project("merchantGatewayAccountId").and_then(around(ETM::toMerchantGwAccId))),
        netAmount: go(project("netAmount").and_then(mandated).and_then(nonNegative).and_then(liftPure(fromDouble))),
        txnAmount: go(project("txnAmount").and_then(mandated).and_then(nonNegative).and_then(liftPure(fromDouble))),
        txnObjectType: go(project("txnObjectType").and_then(mandated).and_then(ETTD::textToTxnObjectType)),
        sourceObject: go(project("sourceObject")),
        sourceObjectId: go(project("sourceObjectId").and_then(around(ETTD::toSourceObjectId))),
        currency: go(project("currency").and_then(mandated).and_then(textToCurr)),
        surchargeAmount: go(project("surchargeAmount").and_then(around(nonNegative).and_then(liftPure(fromDouble)))),
        taxAmount: go(project("taxAmount").and_then(around(nonNegative).and_then(liftPure(fromDouble)))),
        internalMetadata: go(project("internalMetadata")),
        metadata: go(project("metadata")),
        offerDeductionAmount: go(project("offerDeductionAmount").and_then(around(nonNegative).and_then(liftPure(fromDouble)))),
        internalTrackingInfo: go(project("internalTrackingInfo")),
        partitionKey: go(project("partitionKey")),
        txnAmountBreakup: go(project("txnAmountBreakup").and_then(around(ETTD::toTransactionCharges))),
    }
}

pub fn parseFromApiTxnCardInfo(apiType: T::ApiTxnCardInfo) -> Parsed<ETCa::TxnCardInfo> {
    ETCa::TxnCardInfo {
        id: go(project("id").and_then(mandated).and_then(parseString::<_, i64>()).and_then(nonNegative).and_then(ETCa::toTxnCardInfoPId)),
        txnId: go(project("txnId").and_then(ETTD::toTransactionId)),
        cardIsin: go(project("cardIsin")),
        cardIssuerBankName: go(project("cardIssuerBankName")),
        cardSwitchProvider: go(project("cardSwitchProvider").and_then(around(secret))),
        cardType: go(project("cardType").and_then(around(ETCa::toCardType))),
        nameOnCard: go(project("nameOnCard").and_then(around(secret))),
        txnDetailId: go(project("txnDetailId").and_then(mandated).and_then(parseString::<_, i64>()).and_then(nonNegative).and_then(ETTD::toTxnDetailId)),
        dateCreated: go(project("dateCreated").and_then(mandated)),
        paymentMethodType: go(project("paymentMethodType").and_then(mandated).and_then(ETP::textToPaymentMethodType)),
        paymentMethod: go(project("paymentMethod").and_then(mandated)),
        paymentSource: go(project("paymentSource")),
        authType: go(project("authType").and_then(around(ETCa::textToAuthType)).and_then(around(secret))),
        partitionKey: go(project("partitionKey")),
    }
}

pub fn parseApiDeciderRequest(apiType: T::ApiDeciderRequest) -> Parsed<T::DomainDeciderRequestForApiCall> {
    T::DomainDeciderRequestForApiCall {
        orderReference: parseFromApiOrderReference(apiType.orderReference),
        orderMetadata: parseFromApiOrderMetadataV2(apiType.orderMetadata),
        txnDetail: parseFromApiTxnDetail(apiType.txnDetail),
        txnCardInfo: parseFromApiTxnCardInfo(apiType.txnCardInfo),
        card_token: go(project("card_token")),
        txn_type: go(project("txn_type")),
        should_create_mandate: go(project("should_create_mandate")),
        enforce_gateway_list: go(project("enforce_gateway_list")),
        priority_logic_script: go(project("priority_logic_script")),
    }
}