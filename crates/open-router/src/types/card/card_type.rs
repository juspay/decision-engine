use serde::{Serialize, Deserialize};
use std::string::String;
use std::fmt;
use std::convert::TryFrom;
use crate::error::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CardType {
    Aadhaar,
    ATMCard,
    Cash,
    Credit,
    Debit,
    NB,
    Paylater,
    Prepaid,
    Reward,
    UPI,
    Wallet,
    VirtualAccount,
    Otc,
    Rtp,
    Crypto,
    Blank,
    PAN,
}

impl fmt::Display for CardType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", card_type_to_text(self))
    }
}

impl TryFrom<String> for CardType {
    type Error = ApiError;

    fn try_from(value: String) -> Result<Self, ApiError> {
        match value.as_str() {
            "AADHAAR" => Ok(CardType::Aadhaar),
            "ATM_CARD" => Ok(CardType::ATMCard),
            "CASH" => Ok(CardType::Cash),
            "CREDIT" => Ok(CardType::Credit),
            "DEBIT" => Ok(CardType::Debit),
            "NB" => Ok(CardType::NB),
            "PAYLATER" => Ok(CardType::Paylater),
            "PREPAID" => Ok(CardType::Prepaid),
            "REWARD" => Ok(CardType::Reward),
            "UPI" => Ok(CardType::UPI),
            "WALLET" => Ok(CardType::Wallet),
            "VIRTUAL_ACCOUNT" => Ok(CardType::VirtualAccount),
            "OTC" => Ok(CardType::Otc),
            "RTP" => Ok(CardType::Rtp),
            "CRYPTO" => Ok(CardType::Crypto),
            "BLANK" => Ok(CardType::Blank),
            "PAN" => Ok(CardType::PAN),
            _ => Err(ApiError::ParsingError("Invalid Card Type")),
        }
    }
}

pub fn card_type_to_text(card_type: &CardType) -> String {
    match card_type {
        CardType::Aadhaar => "AADHAAR".into(),
        CardType::ATMCard => "ATM_CARD".into(),
        CardType::Cash => "CASH".into(),
        CardType::Credit => "CREDIT".into(),
        CardType::Debit => "DEBIT".into(),
        CardType::NB => "NB".into(),
        CardType::Paylater => "PAYLATER".into(),
        CardType::Prepaid => "PREPAID".into(),
        CardType::Reward => "REWARD".into(),
        CardType::UPI => "UPI".into(),
        CardType::Wallet => "WALLET".into(),
        CardType::VirtualAccount => "VIRTUAL_ACCOUNT".into(),
        CardType::Otc => "OTC".into(),
        CardType::Rtp => "RTP".into(),
        CardType::Crypto => "CRYPTO".into(),
        CardType::Blank => "BLANK".into(),
        CardType::PAN => "PAN".into(),
    }
}

pub fn to_card_type(input: &str) -> Result<CardType, ApiError> {
    CardType::try_from(input.to_string())
}