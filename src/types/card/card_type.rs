use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::string::String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CardType {
    Aadhaar,
    AtmCard,
    Cash,
    Credit,
    Debit,
    Nb,
    Paylater,
    Prepaid,
    Reward,
    Upi,
    Wallet,
    VirtualAccount,
    Otc,
    Rtp,
    Crypto,
    Blank,
    Pan,
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
            "AADHAAR" => Ok(Self::Aadhaar),
            "ATM_CARD" => Ok(Self::AtmCard),
            "CASH" => Ok(Self::Cash),
            "CREDIT" => Ok(Self::Credit),
            "DEBIT" => Ok(Self::Debit),
            "NB" => Ok(Self::Nb),
            "PAYLATER" => Ok(Self::Paylater),
            "PREPAID" => Ok(Self::Prepaid),
            "REWARD" => Ok(Self::Reward),
            "UPI" => Ok(Self::Upi),
            "WALLET" => Ok(Self::Wallet),
            "VIRTUAL_ACCOUNT" => Ok(Self::VirtualAccount),
            "OTC" => Ok(Self::Otc),
            "RTP" => Ok(Self::Rtp),
            "CRYPTO" => Ok(Self::Crypto),
            "BLANK" => Ok(Self::Blank),
            "PAN" => Ok(Self::Pan),
            _ => Err(ApiError::ParsingError("Invalid Card Type")),
        }
    }
}

pub fn card_type_to_text(card_type: &CardType) -> String {
    match card_type {
        CardType::Aadhaar => "AADHAAR".into(),
        CardType::AtmCard => "ATM_CARD".into(),
        CardType::Cash => "CASH".into(),
        CardType::Credit => "CREDIT".into(),
        CardType::Debit => "DEBIT".into(),
        CardType::Nb => "NB".into(),
        CardType::Paylater => "PAYLATER".into(),
        CardType::Prepaid => "PREPAID".into(),
        CardType::Reward => "REWARD".into(),
        CardType::Upi => "UPI".into(),
        CardType::Wallet => "WALLET".into(),
        CardType::VirtualAccount => "VIRTUAL_ACCOUNT".into(),
        CardType::Otc => "OTC".into(),
        CardType::Rtp => "RTP".into(),
        CardType::Crypto => "CRYPTO".into(),
        CardType::Blank => "BLANK".into(),
        CardType::Pan => "PAN".into(),
    }
}

pub fn to_card_type(input: &str) -> Result<CardType, ApiError> {
    CardType::try_from(input.to_string())
}
