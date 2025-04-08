use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::option::Option;
use std::string::String;
use std::vec::Vec;

use crate::error::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Isin {
    Isin(char, char, char, char, char, char),
    Isin9(char, char, char, char, char, char, char, char, char),
    Isin8(char, char, char, char, char, char, char, char),
    Isin7(char, char, char, char, char, char, char),
    IsinWithSpaceAfter4(char, char, char, char, char),
}

impl Isin {
    pub fn to_text(&self) -> String {
        match self {
            Self::Isin(d1, d2, d3, d4, d5, d6) => format!("{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6),
            Self::Isin9(d1, d2, d3, d4, d5, d6, d7, d8, d9) => {
                format!("{}{}{}{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6, d7, d8, d9)
            }
            Self::Isin8(d1, d2, d3, d4, d5, d6, d7, d8) => {
                format!("{}{}{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6, d7, d8)
            }
            Self::Isin7(d1, d2, d3, d4, d5, d6, d7) => {
                format!("{}{}{}{}{}{}{}", d1, d2, d3, d4, d5, d6, d7)
            }
            Self::IsinWithSpaceAfter4(d1, d2, d3, d4, d5) => {
                format!("{}{}{}{} {}", d1, d2, d3, d4, d5)
            }
        }
    }
}

fn char_d10_maybe(c: char) -> Option<char> {
    match c {
        '0'..='9' => Some(c),
        _ => None,
    }
}

impl TryFrom<&str> for Isin {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let digits: Vec<Option<char>> = value.chars().map(char_d10_maybe).collect();
        let digits: Vec<char> = digits.into_iter().flatten().collect();

        match digits.len() {
            6 => Ok(Self::Isin(
                digits[0], digits[1], digits[2], digits[3], digits[4], digits[5],
            )),
            7 => Ok(Self::Isin7(
                digits[0], digits[1], digits[2], digits[3], digits[4], digits[5], digits[6],
            )),
            8 => Ok(Self::Isin8(
                digits[0], digits[1], digits[2], digits[3], digits[4], digits[5], digits[6],
                digits[7],
            )),
            9 => Ok(Self::Isin9(
                digits[0], digits[1], digits[2], digits[3], digits[4], digits[5], digits[6],
                digits[7], digits[8],
            )),
            _ => Err("Invalid ISIN format".to_string()),
        }
    }
}

pub fn string_to_int_default_zero(input: &str) -> i32 {
    input.parse::<i32>().unwrap_or(0)
}

pub fn to_isin(isin: String) -> Result<Isin, ApiError> {
    Isin::try_from(isin.as_str()).map_err(|_| ApiError::ParsingError("Invalid ISIN format"))
}
