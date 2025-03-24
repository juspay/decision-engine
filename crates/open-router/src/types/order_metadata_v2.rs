use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderMetadataV2PId {
    pub order_metadata_v2_pid: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderMetadataV2 {
    pub id: OrderMetadataV2PId,
    pub date_created: PrimitiveDateTime,
    pub last_updated: PrimitiveDateTime,
    pub metadata: Option<String>,
    pub order_reference_id: i64,
    pub ip_address: Option<String>,
    pub partition_key: Option<PrimitiveDateTime>,
}

pub fn to_order_metadata_v2_pid(order_metadata_v2_pid: i64) -> OrderMetadataV2PId {
    OrderMetadataV2PId {
        order_metadata_v2_pid,
    }
}