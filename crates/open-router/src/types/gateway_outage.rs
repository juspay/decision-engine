use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;
// use db::eulermeshimpl::meshConfig;
// use db::mesh::internal;
use crate::types::bank_code::{to_bank_code_id, BankCodeId};
// use control::category::{self, liftPure};
use crate::app::get_tenant_app_state;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
// use std::time::{LocalTime, UTCTime};
use crate::types::gateway::{text_to_gateway, Gateway};
use crate::types::merchant::id::{to_optional_merchant_id, MerchantId};
// use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, liftEither, parseField, project, toUTC};
use crate::types::payment::payment_method::{text_to_payment_method_type, PaymentMethodType};
// use eulerhs::extra::combinators::toDomainAll;
// use eulerhs::language::MonadFlow;
// use named::{self, Named};
// use prelude::hiding::{id};
// use sequelize::{Clause, Term};
// use test::quickcheck::Arbitrary;
use crate::types::txn_details::types::TxnObjectType;
// use data::text::encoding::encodeUtf8;
use crate::types::card::card_type::CardType;
// use eulerhs::extra::aeson::aesonOmitNothingFields;
use crate::storage::types::GatewayOutage as DBGatewayOutage;

use crate::storage::schema::gateway_outage::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayOutageId {
    #[serde(rename = "gatewayOutageId")]
    pub gatewayOutageId: String,
}

pub fn to_gateway_outage_id(id: String) -> GatewayOutageId {
    GatewayOutageId {
        gatewayOutageId: id,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledOutageMetadata {
    #[serde(rename = "cardType")]
    pub cardType: Option<CardType>,
    #[serde(rename = "flowType")]
    pub flowType: Option<String>,
    #[serde(rename = "txnObjectType")]
    pub txnObjectType: Option<TxnObjectType>,
    #[serde(rename = "app")]
    pub app: Option<String>,
    #[serde(rename = "handle")]
    pub handle: Option<String>,
    #[serde(rename = "sourceObject")]
    pub sourceObject: Option<String>,
}

pub fn to_schedule_outage_metadata(data: String) -> Result<ScheduledOutageMetadata, ApiError> {
    match serde_json::from_str::<ScheduledOutageMetadata>(&data) {
        Ok(res) => Ok(res),
        _ => Err(ApiError::ParsingError("Invalid Schedule Outage Metadata")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayOutage {
    #[serde(rename = "id")]
    pub id: GatewayOutageId,
    #[serde(rename = "version")]
    pub version: i32,
    #[serde(rename = "endTime")]
    pub endTime: PrimitiveDateTime,
    #[serde(rename = "gateway")]
    pub gateway: Option<Gateway>,
    #[serde(rename = "merchantId")]
    pub merchantId: Option<MerchantId>,
    #[serde(rename = "startTime")]
    pub startTime: PrimitiveDateTime,
    #[serde(rename = "bank")]
    pub bank: Option<String>,
    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: Option<PaymentMethodType>,
    #[serde(rename = "paymentMethod")]
    pub paymentMethod: Option<String>,
    #[serde(rename = "description")]
    pub description: Option<String>,
    #[serde(rename = "dateCreated")]
    pub dateCreated: Option<PrimitiveDateTime>,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: Option<PrimitiveDateTime>,
    #[serde(rename = "juspayBankCodeId")]
    pub juspayBankCodeId: Option<BankCodeId>,
    #[serde(rename = "metadata")]
    pub metadata: Option<ScheduledOutageMetadata>,
}

impl TryFrom<DBGatewayOutage> for GatewayOutage {
    type Error = ApiError;

    fn try_from(db_type: DBGatewayOutage) -> Result<Self, ApiError> {
        Ok(Self {
            id: to_gateway_outage_id(db_type.id),
            version: db_type.version,
            endTime: db_type.end_time,
            gateway: db_type
                .gateway
                .map(|gw| text_to_gateway(gw.as_str()))
                .transpose()?,
            merchantId: to_optional_merchant_id(db_type.merchant_id),
            startTime: db_type.start_time,
            bank: db_type.bank,
            paymentMethodType: db_type
                .payment_method_type
                .map(text_to_payment_method_type)
                .transpose()?,
            paymentMethod: db_type.payment_method,
            description: db_type.description,
            dateCreated: db_type.date_created,
            lastUpdated: db_type.last_updated,
            juspayBankCodeId: db_type.juspay_bank_code_id.map(to_bank_code_id),
            metadata: db_type
                .metadata
                .map(to_schedule_outage_metadata)
                .transpose()?,
        })
    }
}

pub async fn getPotentialGwOutagesDB(
    time_now: PrimitiveDateTime,
) -> Result<Vec<DBGatewayOutage>, crate::generics::MeshError> {
    // Query gateway outages that are currently active
    let app_state = get_tenant_app_state().await;
    crate::generics::generic_find_all::<<DBGatewayOutage as HasTable>::Table, _, DBGatewayOutage>(
        &app_state.db,
        dsl::start_time
            .lt(time_now)
            .and(dsl::end_time.gt(time_now))
            .and(dsl::gateway.is_not_null()),
    )
    .await
}

pub async fn getPotentialGwOutages(time_now: PrimitiveDateTime) -> Vec<GatewayOutage> {
    // Call the database function and handle the results
    match getPotentialGwOutagesDB(time_now).await {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record| GatewayOutage::try_from(db_record).ok())
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD Implement DB Calls

// pub fn getPotentialGwOutagesDB<M: MonadFlow>(
//     time_now: LocalTime,
// ) -> Result<Vec<DB::GatewayOutage>, MeshError> {
//     let db_conf = getEulerDbConf::<DB::GatewayOutageT>();
//     findAllRows(
//         db_conf,
//         meshConfig,
//         vec![
//             Clause::And(vec![
//                 Clause::Is(DB::startTime, Term::LessThan(time_now)),
//                 Clause::Is(DB::endTime, Term::GreaterThan(time_now)),
//                 Clause::Is(DB::gateway, Term::Not(Term::Eq(None))),
//             ]),
//         ],
//     )
// }

// pub fn getPotentialGwOutages<M: MonadFlow>(
//     time_now: LocalTime,
// ) -> Vec<GatewayOutage> {
//     let res = getPotentialGwOutagesDB(time_now);
//     toDomainAll(
//         res,
//         GatewayOutage::parseGatewayOutage,
//         Named::new("getPotentialGwOutages"),
//         Named::new("parseGatewayOutage"),
//     )
// }
