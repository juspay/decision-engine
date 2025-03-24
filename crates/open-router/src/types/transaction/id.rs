use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionId {
    transactionId: String,
}

pub fn transaction_id_to_text(id: TransactionId) -> String {
    id.transactionId
}

pub fn to_transaction_id(id: String) -> TransactionId {
    TransactionId {
        transactionId: id,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpgTransactionId {
    epgTransactionId: String,
}