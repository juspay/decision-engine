use crate::storage::consts;

pub fn generate_nano_id(id_length: usize) -> String {
    nanoid::nanoid!(id_length, &consts::ALPHABETS)
}

/// Generate UUID v4 as strings to be used in storage layer
pub fn generate_uuid() -> String {
    uuid::Uuid::now_v7().to_string()
}
