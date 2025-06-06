use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

use crate::decider::gatewaydecider::types::Gateway;

use super::money::internal::Money;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub struct TxnOfferInfoPId {
    pub txn_offer_info_pid: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OfferStatus {
    INITIATED,
    AVAILED,
    INVALID,
    FAILED,
    REFUNDED,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OfferType {
    CASHBACK,
    VOUCHER,
    DISCOUNT,
    REWARD_POINT,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnOfferInfo {
    pub id: TxnOfferInfoPId,
    pub amount: Money,
    pub date_created: PrimitiveDateTime,
    pub gateway: Option<String>,
    pub last_updated: PrimitiveDateTime,
    pub status: OfferStatus,
}
