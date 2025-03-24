use error_stack::ResultExt;
use serde::{Serialize, Deserialize};
use serde_json::{json, Deserializer, Value};
use std::string::String;
use std::f64;
use std::ops::{Add, Sub};
use std::convert::From;
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use crate::error::ApiError;
// use db::eulermeshimpl as Env;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Money(pub f64);

impl Money {
    pub fn from_double(val: f64) -> Self {
        let fixed = format!("{:.2}", val);
        Money(fixed.parse::<f64>().unwrap())
    }

    pub fn from_whole(val: i64) -> Self {
        Money(val as f64)
    }

    pub fn to_double(&self) -> f64 {
        self.0
    }

    pub fn m_add(&self, other: &Money) -> Money {
        Money(self.0 + other.0)
    }

    pub fn m_sub(&self, other: &Money) -> Money {
        Money(self.0 - other.0)
    }
}

// impl Serialize for Money {
//     fn serialize<S>(&self, serializer: S) -> Result<Value, ApiError>
//     where
//         S: serde::Serializer,
//     {
//         if Env::new_money_format_enabled() {
//             json!({
//                 "version": "v1",
//                 "value": self.0,
//             }).serialize(serializer)
//         } else {
//             let value = (self.0 * 10000.0).round();
//             serializer.serialize_f64(value)
//         }
//     }
// }

// impl<'de> Deserialize<'de> for Money {
//     fn deserialize<String>(deserializer: &str) -> Result<Money, ApiError>
//     {
//         match serde_json::from_str::<Money>(&deserializer) {
//             Ok(val) =>  {
//                 let ob = val.as_object();
//                 if let Some(Value::String(version)) = obj.get("version") {
//                     if version == "v1" {
//                         if let Some(Value::Number(val)) = obj.get("value") {
//                             if let Some(f) = val.as_f64() {
//                                 return Ok(Money(f));
//                             }
//                         }
//                         return Err(ApiError::ParsingError("Expected numeric 'value' in v1"));
//                     } else {
//                         return Err(ApiError::ParsingError("Unsupported version"));
//                     }
//                 }
//             },
//             Err(e) => ApiError::ParsingError("Failed to parse RefundMetricRow in clickhouse results"),
//         }
//     }
//     Err(serde_json::Error::custom("Invalid old format"))
// }


// impl<'de> Deserialize<'de> for Money {
//     fn deserialize<String>(deserializer: &str) -> Result<Money, ApiError>
//     {
//         match serde_json::from_str::<Money>(&deserializer) {
//             Ok(val) => {
//                 let ob = val.as_object();
//                 if let Some(Value::String(version)) = obj.get("version") {
//                     if version == "v1" {
//                         if let Some(Value::Number(val)) = obj.get("value") {
//                             if let Some(f) = val.as_f64() {
//                                 return Ok(Money(f));
//                             }
//                         }
//                         return Err(ApiError::ParsingError("Expected numeric 'value' in v1"));
//                     } else {
//                         return Err(ApiError::ParsingError("Unsupported version"));
//                     }
//                 }
//             },
//             Err(e) => ApiError::ParsingError("Failed to parse RefundMetricRow in clickhouse results"),
//         }
//     }
// }

impl Add for Money {
    type Output = Money;

    fn add(self, other: Money) -> Money {
        Money(self.0 + other.0)
    }
}

impl Sub for Money {
    type Output = Money;

    fn sub(self, other: Money) -> Money {
        Money(self.0 - other.0)
    }
}
