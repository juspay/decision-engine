use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantId{
    pub merchantId: String,
}

pub fn to_merchant_id(id: String) -> MerchantId {
    MerchantId { merchantId: id }
}

pub fn merchant_id_to_text(id: MerchantId) -> String{
    id.merchantId
}

pub fn to_optional_merchant_id(id: Option<String>) -> Option<MerchantId> {
    match id {
        Some(id) => Some(to_merchant_id(id)),
        None => None,
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct MerchantPId {
    pub merchantPId: i64,
}

pub fn to_merchant_pid(id: i64) -> MerchantPId {
    MerchantPId { merchantPId: id }
}

pub fn merchant_pid_to_text(id: MerchantPId) -> i64{
    id.merchantPId
}
