
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::PrimitiveDateTime;
use std::string::String;
use std::option::Option;
use std::time::SystemTime;
// use chrono::NaiveDateTime;
// use data::text::Text;
// use juspay::extra::nonemptytext::NonEmptyText;
use crate::types::txn_details::types::TxnDetailId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnOfferDetailId {
    #[serde(rename = "txnOfferDetailId")]
    pub txnOfferDetailId: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnOfferDetail {
    #[serde(rename = "id")]
    pub id: TxnOfferDetailId,
    #[serde(rename = "txnDetailId")]
    pub txnDetailId: TxnDetailId,
    #[serde(rename = "offerId")]
    pub offerId: String,
    #[serde(rename = "status")]
    pub status: TxnOfferDetailStatus,
    #[serde(rename = "dateCreated")]
    pub dateCreated: Option<SystemTime>,
    #[serde(rename = "lastUpdated")]
    pub lastUpdated: Option<SystemTime>,
    #[serde(rename = "gatewayInfo")]
    pub gatewayInfo: Option<String>,
    #[serde(rename = "internalMetadata")]
    pub internalMetadata: Option<String>,
    #[serde(rename = "partitionKey")]
    pub partitionKey: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TxnOfferDetailStatus {
    Created,
    Initiated,
    Availed,
    Refunded,
    PartiallyRefunded,
    Failed,
    Revoked,
}

impl TxnOfferDetailStatus {
    pub fn to_text(&self) -> String {
        match self {
            TxnOfferDetailStatus::Created => "CREATED".into(),
            TxnOfferDetailStatus::Initiated => "INITIATED".into(),
            TxnOfferDetailStatus::Availed => "AVAILED".into(),
            TxnOfferDetailStatus::Refunded => "REFUNDED".into(),
            TxnOfferDetailStatus::PartiallyRefunded => "PARTIALLY_REFUNDED".into(),
            TxnOfferDetailStatus::Failed => "FAILED".into(),
            TxnOfferDetailStatus::Revoked => "REVOKED".into(),
        }
    }
}
