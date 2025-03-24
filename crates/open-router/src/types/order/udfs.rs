use serde::de::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::Map;


#[derive(Debug, Clone, Serialize, Eq, Deserialize, PartialEq)]
pub struct UDFs {
    #[serde(rename = "toMap")]
    pub to_map: std::collections::HashMap<i32, String>,
}

impl UDFs {
    pub fn to_json(&self) -> serde_json::Value {
        let mut arr = vec![serde_json::Value::Null; 10];
        for (k, v) in &self.to_map {
            if let Some(index) = k.checked_sub(1) {
                if index < 10 {
                    arr[index as usize] = serde_json::Value::String(v.clone());
                }
            }
        }
        serde_json::Value::Array(arr)
    }

    pub fn from_json(value: &serde_json::Value) -> Result<Self, serde_json::Error> {
        let arr = value.as_array().ok_or_else(|| serde_json::Error::custom("Expected an array"))?;
        if arr.len() != 10 {
            return Err(serde_json::Error::custom("Wrong number of values"));
        }
        let mut map = std::collections::HashMap::new();
        for (i, v) in arr.iter().enumerate() {
            if let serde_json::Value::String(s) = v {
                map.insert((i + 1) as i32, s.clone());
            } else if !v.is_null() {
                return Err(serde_json::Error::custom("Invalid value"));
            }
        }
        Ok(UDFs { to_map: map })
    }
}

pub fn get_udf(udfs: &UDFs, key: i32) -> Option<&String> {
    udfs.to_map.get(&key)
}
