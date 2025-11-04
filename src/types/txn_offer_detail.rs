use serde::{Deserialize, Serialize};
use std::option::Option;
use std::string::String;
use time::{OffsetDateTime, PrimitiveDateTime};
// use chrono::NaiveDateTime;
// use data::text::Text;
// use juspay::extra::nonemptytext::NonEmptyText;
use crate::types::txn_details::types::{deserialize_optional_primitive_datetime, TxnDetailId};
use serde::{de, ser};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TxnOfferDetailId(String);

impl TxnOfferDetailId {
    pub fn new(s: String) -> Result<Self, String> {
        Ok(TxnOfferDetailId(s))
    }
}

impl<'de> Deserialize<'de> for TxnOfferDetailId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        TxnOfferDetailId::new(s).map_err(de::Error::custom)
    }
}

impl Serialize for TxnOfferDetailId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnOfferDetail {
    #[serde(rename = "id")]
    pub id: TxnOfferDetailId,
    #[serde(rename = "txnDetailId")]
    pub txn_detail_id: TxnDetailId,
    #[serde(rename = "offerId")]
    pub offer_id: String,
    #[serde(rename = "status")]
    pub status: TxnOfferDetailStatus,
    #[serde(rename = "dateCreated")]
    #[serde(with = "time::serde::iso8601::option")]
    pub date_created: Option<OffsetDateTime>,
    #[serde(rename = "lastUpdated")]
    #[serde(with = "time::serde::iso8601::option")]
    pub last_updated: Option<OffsetDateTime>,
    #[serde(rename = "gatewayInfo")]
    pub gateway_info: Option<String>,
    #[serde(rename = "internalMetadata")]
    pub internal_metadata: Option<String>,
    #[serde(rename = "partitionKey")]
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    pub partition_key: Option<PrimitiveDateTime>,
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
            Self::Created => "CREATED".into(),
            Self::Initiated => "INITIATED".into(),
            Self::Availed => "AVAILED".into(),
            Self::Refunded => "REFUNDED".into(),
            Self::PartiallyRefunded => "PARTIALLY_REFUNDED".into(),
            Self::Failed => "FAILED".into(),
            Self::Revoked => "REVOKED".into(),
        }
    }
}
