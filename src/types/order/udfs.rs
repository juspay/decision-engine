use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Eq, Deserialize, PartialEq)]

pub struct UDFs(pub HashMap<i32, String>);

pub fn get_udf(udfs: &UDFs, key: i32) -> Option<&String> {
    udfs.0.get(&key)
}
