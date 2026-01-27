use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomerId(pub String);

pub fn customer_id_text(id: String) -> CustomerId {
    CustomerId(id)
}
