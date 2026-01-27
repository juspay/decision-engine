use serde::{Deserialize, Deserializer, Serialize};
use serde_with::serde_as;
use time::{OffsetDateTime, PrimitiveDateTime};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderMetadataV2PId(i64);

pub fn deserialize_optional_primitive_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<PrimitiveDateTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    if s.is_none() {
        return Ok(None);
    }

    let format = time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");

    match time::PrimitiveDateTime::parse(&s.unwrap(), &format) {
        Ok(o) => {
            crate::logger::debug!("Parsed Time: {:?}", o);
            Ok(Some(o))
        }
        Err(err) => {
            crate::logger::debug!("Error: {:?}", err);
            Ok(None)
        }
    }
}

#[serde_as]
#[serde(rename_all = "camelCase")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderMetadataV2 {
    pub id: OrderMetadataV2PId,
    #[serde(with = "time::serde::iso8601")]
    pub date_created: OffsetDateTime,
    #[serde(with = "time::serde::iso8601")]
    pub last_updated: OffsetDateTime,
    pub metadata: Option<String>,
    pub order_reference_id: i64,
    pub ip_address: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    pub partition_key: Option<PrimitiveDateTime>,
}

pub fn to_order_metadata_v2_pid(order_metadata_v2_pid: i64) -> OrderMetadataV2PId {
    OrderMetadataV2PId(order_metadata_v2_pid)
}
