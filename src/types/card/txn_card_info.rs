use crate::error::ApiError;
use crate::types::card::card_type::CardType;
use crate::types::payment_instrument::PaymentInstrument;
use masking::{PeekInterface, Secret};
use serde::{Deserialize, Deserializer, Serialize};
use time::{OffsetDateTime, PrimitiveDateTime};
// use crate::types::transaction::id::TransactionId;
// use crate::types::txn_details::types::TxnDetailId;
// use juspay::extra::parsing::{Step, lift_either, lift_pure, ParsingErrorType};
// use juspay::extra::secret::{Secret, SecretContext};
use crate::decider::gatewaydecider::utils::mask_secret_option;
use std::fmt::Debug;
use std::option::Option;
use std::string::String;

#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthType {
    Atmpin,
    ThreeDs,
    ThreeDs2,
    Otp,
    OboOtp,
    Vies,
    NoThreeDs,
    NetworkToken,
    Moto,
    Fido,
    Ctp,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Atmpin => write!(f, "ATMPIN"),
            Self::ThreeDs => write!(f, "THREE_DS"),
            Self::ThreeDs2 => write!(f, "THREE_DS_2"),
            Self::Otp => write!(f, "OTP"),
            Self::OboOtp => write!(f, "OBO_OTP"),
            Self::Vies => write!(f, "VIES"),
            Self::NoThreeDs => write!(f, "NO_THREE_DS"),
            Self::NetworkToken => write!(f, "NETWORK_TOKEN"),
            Self::Moto => write!(f, "MOTO"),
            Self::Fido => write!(f, "FIDO"),
            Self::Ctp => write!(f, "CTP"),
        }
    }
}

pub fn text_to_auth_type(ctx: &str) -> Result<AuthType, ApiError> {
    match ctx {
        "ATMPIN" => Ok(AuthType::Atmpin),
        "THREE_DS" => Ok(AuthType::ThreeDs),
        "THREE_DS_2" => Ok(AuthType::ThreeDs2),
        "OTP" => Ok(AuthType::Otp),
        "OBO_OTP" => Ok(AuthType::OboOtp),
        "VIES" => Ok(AuthType::Vies),
        "NO_THREE_DS" => Ok(AuthType::NoThreeDs),
        "NETWORK_TOKEN" => Ok(AuthType::NetworkToken),
        "MOTO" => Ok(AuthType::Moto),
        "FIDO" => Ok(AuthType::Fido),
        "CTP" => Ok(AuthType::Ctp),
        _ => Err(ApiError::ParsingError("Invalid Auth Type")),
    }
}
pub fn auth_type_to_text(ctx: &AuthType) -> String {
    match ctx {
        AuthType::Atmpin => "ATMPIN".to_string(),
        AuthType::ThreeDs => "THREE_DS".to_string(),
        AuthType::ThreeDs2 => "THREE_DS_2".to_string(),
        AuthType::Otp => "OTP".to_string(),
        AuthType::OboOtp => "OBO_OTP".to_string(),
        AuthType::Vies => "VIES".to_string(),
        AuthType::NoThreeDs => "NO_THREE_DS".to_string(),
        AuthType::NetworkToken => "NETWORK_TOKEN".to_string(),
        AuthType::Moto => "MOTO".to_string(),
        AuthType::Fido => "FIDO".to_string(),
        AuthType::Ctp => "CTP".to_string(),
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EMIType {
    NoCostEmi,
    LowCostEmi,
    StandardEmi,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmiDetails {
    #[serde(rename = "emi_type")]
    pub emi_type: EMIType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnCardInfoPId(i64);

pub fn to_txn_card_info_pid(ctx: i64) -> TxnCardInfoPId {
    TxnCardInfoPId(ctx)
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBinPaymentSource {
    #[serde(rename = "is_token_bin")]
    pub is_token_bin: bool,
}

pub fn deserialize_optional_primitive_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<PrimitiveDateTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    if s.is_none() {
        return Ok(None);
    }

    let format = time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");

    match time::PrimitiveDateTime::parse(&s.unwrap(), &format) {
        Ok(o) => Ok(Some(o)),
        Err(err) => {
            crate::logger::debug!("Error: {:?}", err);
            Ok(None)
        }
    }
}

/// A transaction's card and payment method details. `instrument` is the payment method in
/// Decision Engine's canonical form. On the wire, the legacy Euler-format APIs send Euler's
/// fields instead (`paymentMethodType` is the category, `paymentMethod` the specific method); see
/// [`TxnCardInfoWire`].
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(from = "TxnCardInfoWire", into = "TxnCardInfoWire")]
pub struct TxnCardInfo {
    pub id: TxnCardInfoPId,
    pub card_isin: Option<String>,
    pub cardIssuerBankName: Option<String>,
    pub cardSwitchProvider: Option<Secret<String>>,
    pub card_type: Option<CardType>,
    pub card_program: Option<String>,
    /// Issuer country of the card (ISO code or region bucket). Used by the multi-objective
    /// cost cluster key to separate same-currency scenarios (US debit vs EU consumer vs
    /// international) at a single merchant. Optional; absent when issuer enrichment is
    /// unavailable.
    pub card_issuer_country: Option<String>,
    /// Acceptance channel (`ecom`/`pos`/`contactless`) — feeds the in-house category predictor.
    pub channel: Option<String>,
    pub nameOnCard: Option<Secret<String>>,
    pub dateCreated: OffsetDateTime,
    pub instrument: PaymentInstrument,
    pub paymentSource: Option<Secret<String>>,
    pub authType: Option<AuthType>,
    pub partitionKey: Option<PrimitiveDateTime>,
}

/// [`TxnCardInfo`] as the legacy Euler-format APIs send it.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct TxnCardInfoWire {
    #[serde(rename = "id")]
    id: TxnCardInfoPId,
    #[serde(rename = "cardIsin")]
    card_isin: Option<String>,
    #[serde(rename = "cardIssuerBankName")]
    cardIssuerBankName: Option<String>,
    #[serde(rename = "cardSwitchProvider")]
    cardSwitchProvider: Option<Secret<String>>,
    #[serde(rename = "cardType")]
    card_type: Option<CardType>,
    #[serde(rename = "cardProgram", default)]
    card_program: Option<String>,
    #[serde(rename = "cardIssuerCountry", default)]
    card_issuer_country: Option<String>,
    #[serde(rename = "channel", default)]
    channel: Option<String>,
    #[serde(rename = "nameOnCard")]
    nameOnCard: Option<Secret<String>>,
    #[serde(with = "time::serde::iso8601")]
    #[serde(rename = "dateCreated")]
    dateCreated: OffsetDateTime,
    /// Euler's category: `CARD`, `UPI`, `NB`.
    #[serde(rename = "paymentMethodType")]
    paymentMethodType: String,
    /// Euler's specific method: `VISA`, `UPI_PAY`.
    #[serde(rename = "paymentMethod")]
    paymentMethod: String,
    #[serde(rename = "paymentSource")]
    #[serde(serialize_with = "mask_secret_option")]
    paymentSource: Option<Secret<String>>,
    #[serde(rename = "authType")]
    authType: Option<AuthType>,
    #[serde(rename = "partitionKey")]
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    partitionKey: Option<PrimitiveDateTime>,
}

impl From<TxnCardInfoWire> for TxnCardInfo {
    fn from(wire: TxnCardInfoWire) -> Self {
        Self {
            id: wire.id,
            card_isin: wire.card_isin,
            cardIssuerBankName: wire.cardIssuerBankName,
            cardSwitchProvider: wire.cardSwitchProvider,
            card_type: wire.card_type,
            card_program: wire.card_program,
            card_issuer_country: wire.card_issuer_country,
            channel: wire.channel,
            nameOnCard: wire.nameOnCard,
            dateCreated: wire.dateCreated,
            instrument: PaymentInstrument::from_euler(&wire.paymentMethodType, &wire.paymentMethod),
            paymentSource: wire.paymentSource,
            authType: wire.authType,
            partitionKey: wire.partitionKey,
        }
    }
}

impl From<TxnCardInfo> for TxnCardInfoWire {
    fn from(info: TxnCardInfo) -> Self {
        Self {
            id: info.id,
            card_isin: info.card_isin,
            cardIssuerBankName: info.cardIssuerBankName,
            cardSwitchProvider: info.cardSwitchProvider,
            card_type: info.card_type,
            card_program: info.card_program,
            card_issuer_country: info.card_issuer_country,
            channel: info.channel,
            nameOnCard: info.nameOnCard,
            dateCreated: info.dateCreated,
            paymentMethodType: info.instrument.method_code().to_string(),
            paymentMethod: info.instrument.method_type_code().into_owned(),
            paymentSource: info.paymentSource,
            authType: info.authType,
            partitionKey: info.partitionKey,
        }
    }
}

impl TxnCardInfo {
    pub fn get_payment_source_last(&self) -> Option<String> {
        self.paymentSource.as_ref().and_then(|ps| {
            // use `expose_secret()` instead of peek()
            let ps_str = ps.peek();
            ps_str.split('@').next_back().map(|s| s.to_string())
        })
    }

    pub fn get_payment_source(&self) -> Option<String> {
        self.paymentSource.clone().map(|s| s.peek().to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SafeTxnCardInfo {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "cardIsin")]
    pub card_isin: Option<String>,
    #[serde(rename = "cardIssuerBankName")]
    pub cardIssuerBankName: Option<String>,
    #[serde(rename = "cardSwitchProvider")]
    pub cardSwitchProvider: Option<Secret<String>>,
    #[serde(rename = "cardType")]
    pub card_type: Option<CardType>,
    #[serde(rename = "cardProgram", default)]
    pub card_program: Option<String>,
    #[serde(rename = "cardIssuerCountry", default)]
    pub card_issuer_country: Option<String>,
    #[serde(rename = "nameOnCard")]
    pub nameOnCard: Option<Secret<String>>,
    #[serde(with = "time::serde::iso8601")]
    #[serde(rename = "dateCreated")]
    pub dateCreated: OffsetDateTime,
    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: String,
    #[serde(rename = "paymentMethod")]
    pub paymentMethod: String,
    #[serde(rename = "paymentSource")]
    #[serde(serialize_with = "mask_secret_option")]
    pub paymentSource: Option<Secret<String>>,
    #[serde(rename = "authType")]
    pub authType: Option<String>,
    #[serde(rename = "partitionKey")]
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    pub partitionKey: Option<PrimitiveDateTime>,
}

pub fn convert_safe_to_txn_card_info(
    safe_info: SafeTxnCardInfo,
) -> Result<TxnCardInfo, crate::error::ApiError> {
    let id_i64 = safe_info
        .id
        .parse::<i64>()
        .map_err(|_| crate::error::ApiError::ParsingError("id"))?;

    Ok(TxnCardInfo {
        id: TxnCardInfoPId(id_i64),
        card_isin: safe_info.card_isin,
        cardIssuerBankName: safe_info.cardIssuerBankName,
        cardSwitchProvider: safe_info.cardSwitchProvider,
        card_type: safe_info.card_type,
        card_program: safe_info.card_program,
        card_issuer_country: safe_info.card_issuer_country,
        channel: None,
        nameOnCard: safe_info.nameOnCard,
        dateCreated: safe_info.dateCreated,
        // The legacy score API sends Euler's fields.
        instrument: PaymentInstrument::from_euler(
            &safe_info.paymentMethodType,
            &safe_info.paymentMethod,
        ),
        paymentSource: safe_info.paymentSource,
        authType: safe_info
            .authType
            .and_then(|auth| text_to_auth_type(&auth).ok()),
        partitionKey: safe_info.partitionKey,
    })
}
