```rust
use serde::{Serialize, Deserialize};
use serde_json::Value as AValue;
use eulerhs::prelude::*;
use juspay::extra::parsing::{ParsingErrorType, Step, lift_either};
use d10::char::{D10, char_d10_maybe, d10_char};
use std::option::Option;
use std::vec::Vec;
use std::string::String;
use std::convert::TryFrom;
use std::str::FromStr;
use std::collections::HashMap;
use regex::Regex;

#[derive(Debug, PartialEq, Eq, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Isin {
    Isin(D10, D10, D10, D10, D10, D10),
    Isin9(D10, D10, D10, D10, D10, D10, D10, D10, D10),
    Isin8(D10, D10, D10, D10, D10, D10, D10, D10),
    Isin7(D10, D10, D10, D10, D10, D10, D10),
    IsinWithSpaceAfter4(D10, D10, D10, D10, D10),
}

impl Isin {
    pub fn to_text(&self) -> String {
        match self {
            Isin::Isin(d1, d2, d3, d4, d5, d6) => format!("{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6),
            Isin::Isin9(d1, d2, d3, d4, d5, d6, d7, d8, d9) => format!("{}{}{}{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6, d7, d8, d9),
            Isin::Isin8(d1, d2, d3, d4, d5, d6, d7, d8) => format!("{}{}{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6, d7, d8),
            Isin::Isin7(d1, d2, d3, d4, d5, d6, d7) => format!("{}{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6, d7),
            Isin::IsinWithSpaceAfter4(d1, d2, d3, d4, d5) => format!("{}{}{}{} {}", d1, d2, d3, d4, d5),
        }
    }
}

impl TryFrom<&str> for Isin {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let digits: Vec<Option<D10>> = value.chars().map(|c| char_d10_maybe(c)).collect();
        let digits: Vec<D10> = digits.into_iter().filter_map(|d| d).collect();

        match digits.len() {
            6 => Ok(Isin::Isin(digits[0], digits[1], digits[2], digits[3], digits[4], digits[5])),
            7 => Ok(Isin::Isin7(digits[0], digits[1], digits[2], digits[3], digits[4], digits[5], digits[6])),
            8 => Ok(Isin::Isin8(digits[0], digits[1], digits[2], digits[3], digits[4], digits[5], digits[6], digits[7])),
            9 => Ok(Isin::Isin9(digits[0], digits[1], digits[2], digits[3], digits[4], digits[5], digits[6], digits[7], digits[8])),
            _ => Err("Invalid ISIN format".to_string()),
        }
    }
}

pub fn string_to_int_default_zero(input: &str) -> i32 {
    input.parse::<i32>().unwrap_or(0)
}

pub type CardRange = (i32, i32);

lazy_static! {
    static ref CARD_PATTERNS: HashMap<&'static str, Regex> = {
        let mut m = HashMap::new();
        m.insert("maestro", Regex::new(r"^(5018|5081|5044|504681|504993|5020|502260|5038|603845|603123|6304|6759|676[1-3]|6220|504834|504817|504645|504775|600206)").unwrap());
        m.insert("rupay", Regex::new(r"^(508227|508[5-9]|603741|60698[5-9]|60699|607[0-8]|6079[0-7]|60798[0-4]|60800[1-9]|6080[1-9]|608[1-4]|608500|6521[5-9]|652[2-9]|6530|6531[0-4]|817290|817368|817378|353800)").unwrap());
        m.insert("dinersclub", Regex::new(r"^(36|38|30[0-5])").unwrap());
        m.insert("jcb", Regex::new(r"^35").unwrap());
        m.insert("discover", Regex::new(r"^(6011|65|64[4-9]|622)").unwrap());
        m.insert("mastercard", Regex::new(r"^5[1-5]").unwrap());
        m.insert("amex", Regex::new(r"^3[47]").unwrap());
        m.insert("visa", Regex::new(r"^4").unwrap());
        m.insert("sodexo", Regex::new(r"^(637513)").unwrap());
        m.insert("bajaj", Regex::new(r"^(203040)").unwrap());
        m
    };
}
```