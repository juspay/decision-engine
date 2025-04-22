use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub struct SourceObjectId {
    source_object_id: String,
}

pub fn to_source_object_id(id: String) -> SourceObjectId {
    SourceObjectId {
        source_object_id: id,
    }
}
