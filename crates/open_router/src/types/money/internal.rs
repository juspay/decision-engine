
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::string::String;
use std::f64;
use std::ops::{Add, Sub};
use std::convert::From;
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use db::eulermeshimpl as Env;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
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

impl Serialize for Money {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if Env::new_money_format_enabled() {
            let mut state = serializer.serialize_struct("Money", 2)?;
            state.serialize_field("version", "v1")?;
            state.serialize_field("value", &self.0)?;
            state.end()
        } else {
            let value = (self.0 * 10000.0).round();
            serializer.serialize_f64(value)
        }
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        parse_new_format(&value).or_else(|_| parse_old_format(&value))
    }
}

fn parse_new_format(value: &Value) -> Result<Money, serde_json::Error> {
    if let Value::Object(obj) = value {
        if let Some(Value::String(version)) = obj.get("version") {
            if version == "v1" {
                if let Some(Value::Number(num)) = obj.get("value") {
                    if let Some(val) = num.as_f64() {
                        return Ok(Money(val));
                    }
                }
            }
        }
    }
    Err(serde_json::Error::custom("Unsupported version"))
}

fn parse_old_format(value: &Value) -> Result<Money, serde_json::Error> {
    if let Value::Number(num) = value {
        if let Some(val) = num.as_f64() {
            return Ok(Money(val / 10000.0));
        }
    }
    Err(serde_json::Error::custom("Invalid old format"))
}

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
