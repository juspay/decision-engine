use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantId(pub String);

pub fn to_merchant_id(id: String) -> MerchantId {
    MerchantId(id)
}

pub fn merchant_id_to_text(id: MerchantId) -> String {
    id.0
}

pub fn to_optional_merchant_id(id: Option<String>) -> Option<MerchantId> {
    id.map(to_merchant_id)
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Copy)]
pub struct MerchantPId(pub i64);

pub fn to_merchant_pid(id: i64) -> MerchantPId {
    MerchantPId(id)
}

pub fn merchant_pid_to_text(id: MerchantPId) -> i64 {
    id.0
}
