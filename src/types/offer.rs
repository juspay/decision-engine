use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OfferId {
    pub offerId: String,
}

pub fn to_offer_id(id: String) -> OfferId {
    OfferId { offerId: id }
}
