use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionId(pub String);

pub fn transaction_id_to_text(id: TransactionId) -> String {
    id.0
}

pub fn to_transaction_id(id: String) -> TransactionId {
    TransactionId(id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpgTransactionId {
    epgTransactionId: String,
}
