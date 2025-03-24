```rust
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::string::String;
use std::fmt;
use std::convert::TryFrom;
use data_text::Text;
use juspay::extra::parsing::{ParsingErrorType, Step, lift_either};
use test_quickcheck::{Arbitrary, arbitrary};
use test_quickcheck_arbitrary_generic::generic_arbitrary;

#[derive(Debug, PartialEq, Eq, Ord, Serialize, Deserialize)]
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

impl TryFrom<Text> for CardType {
    type Error = ParsingErrorType;

    fn try_from(value: Text) -> Result<Self, Self::Error> {
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
            other => Err(ParsingErrorType::UnexpectedTextValue("CardType".to_string(), other.to_string())),
        }
    }
}

impl Arbitrary for CardType {
    fn arbitrary(g: &mut test_quickcheck::Gen) -> Self {
        generic_arbitrary(g)
    }
}

pub fn card_type_to_text(card_type: &CardType) -> Text {
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

pub fn to_card_type() -> Step<Context, Text, CardType> {
    lift_either(|text| CardType::try_from(text))
}
```