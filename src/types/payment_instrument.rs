//! A payment's instrument in one canonical form.
//!
//! Decision Engine speaks Hyperswitch's vocabulary: the payment method is the category (`card`,
//! `wallet`, `upi`) and the payment method type the specific method within it (`credit`,
//! `google_pay`, `upi_collect`). Every entry point parses its payload into a
//! [`PaymentInstrument`] once, and the engine works only on that:
//!
//! - Hyperswitch's format, with [`PaymentInstrument::from_hyperswitch`] (strict, for requests
//!   Decision Engine validates) or [`PaymentInstrument::from_hyperswitch_lenient`] (for values
//!   Hyperswitch already validated, such as routing rule parameters, and for stored data).
//! - Euler's legacy format, which carries the category in `paymentMethodType` and the specific
//!   method in `paymentMethod`, with [`PaymentInstrument::from_euler`].
//!
//! Nothing downstream has to know which format a payment arrived in.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The payment method category. Hyperswitch's sixteen come first; the rest are Euler's own
/// categories, which only [`PaymentInstrument::from_euler`] produces.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::EnumIter,
)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE", ascii_case_insensitive)]
pub enum PaymentMethod {
    Card,
    CardRedirect,
    PayLater,
    Wallet,
    BankRedirect,
    BankTransfer,
    Crypto,
    BankDebit,
    Reward,
    RealTimePayment,
    Upi,
    Voucher,
    GiftCard,
    OpenBanking,
    MobilePayment,
    NetworkToken,
    #[strum(serialize = "NB")]
    NetBanking,
    ConsumerFinance,
    Cash,
    AtmCard,
    Aadhaar,
    MerchantContainer,
    WalletContainer,
    #[strum(serialize = "PAPERNACH")]
    PaperNach,
    VirtualAccount,
    Otc,
    Rtp,
    CardQr,
    Pan,
    /// The request named no payment method.
    Unknown,
}

impl PaymentMethod {
    /// Hyperswitch's payment methods, in Hyperswitch's order.
    pub const HYPERSWITCH: [Self; 16] = [
        Self::Card,
        Self::CardRedirect,
        Self::PayLater,
        Self::Wallet,
        Self::BankRedirect,
        Self::BankTransfer,
        Self::Crypto,
        Self::BankDebit,
        Self::Reward,
        Self::RealTimePayment,
        Self::Upi,
        Self::Voucher,
        Self::GiftCard,
        Self::OpenBanking,
        Self::MobilePayment,
        Self::NetworkToken,
    ];

    /// The upper-case code the engine keys and compares on: `CARD`, `PAY_LATER`, `NB`.
    pub fn code(self) -> &'static str {
        self.into()
    }

    /// Hyperswitch's spelling: `card`, `pay_later`. Only meaningful for Hyperswitch's methods.
    pub fn hyperswitch_name(self) -> String {
        self.code().to_ascii_lowercase()
    }

    pub fn is_hyperswitch(self) -> bool {
        Self::HYPERSWITCH.contains(&self)
    }

    /// One of Hyperswitch's payment methods, in any case.
    pub fn from_hyperswitch(value: &str) -> Option<Self> {
        Self::from_str(value.trim())
            .ok()
            .filter(|method| method.is_hyperswitch())
    }

    /// An Euler category code (`CARD`, `NB`, `PAYLATER`); anything else is [`Self::Unknown`].
    pub fn from_euler(value: &str) -> Self {
        match value.trim().to_ascii_uppercase().as_str() {
            "PAYLATER" => Self::PayLater,
            other => Self::from_str(other).unwrap_or(Self::Unknown),
        }
    }
}

impl fmt::Display for PaymentMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Hyperswitch's payment method types (its `PaymentMethodType`), spelled as Hyperswitch
/// serializes them.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::EnumIter,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum PaymentMethodType {
    Ach,
    Affirm,
    AfterpayClearpay,
    Alfamart,
    AliPay,
    AliPayHk,
    Alma,
    AmazonPay,
    ApplePay,
    Atome,
    Bacs,
    BancontactCard,
    BcaBankTransfer,
    Becs,
    Benefit,
    BhnCardNetwork,
    Bizum,
    Blik,
    Bluecode,
    BniVa,
    Boleto,
    Breadpay,
    BriVa,
    Card,
    CardRedirect,
    Cashapp,
    CimbVa,
    #[strum(to_string = "classic_reward", serialize = "classic")]
    ClassicReward,
    Credit,
    CryptoCurrency,
    Dana,
    DanamonVa,
    Debit,
    DirectCarrierBilling,
    DuitNow,
    Efecty,
    Eft,
    EftDebitOrder,
    Eps,
    Evoucher,
    FamilyMart,
    Flexiti,
    Fps,
    Gcash,
    Giropay,
    Givex,
    GooglePay,
    GoPay,
    Ideal,
    Indomaret,
    IndonesianBankTransfer,
    InstantBankTransfer,
    InstantBankTransferFinland,
    InstantBankTransferPoland,
    Interac,
    KakaoPay,
    Klarna,
    Knet,
    Lawson,
    LocalBankRedirect,
    LocalBankTransfer,
    MandiriVa,
    MbWay,
    Mifinity,
    MiniStop,
    MobilePay,
    Momo,
    MomoAtm,
    Multibanco,
    Neteller,
    NetworkToken,
    OnlineBankingCzechRepublic,
    OnlineBankingFinland,
    OnlineBankingFpx,
    OnlineBankingPoland,
    OnlineBankingSlovakia,
    OnlineBankingThailand,
    OpenBanking,
    OpenBankingPis,
    OpenBankingUk,
    Oxxo,
    PagoEfectivo,
    PayBright,
    PayEasy,
    Payjustnow,
    Paypal,
    PaySafeCard,
    Paysera,
    Payshap,
    PayshapProxy,
    Paze,
    PermataBankTransfer,
    Pix,
    PixAutomaticoPush,
    PixAutomaticoQr,
    PixEmv,
    PixKey,
    PixQr,
    PromptPay,
    Przelewy24,
    Pse,
    Qris,
    RedCompra,
    RedPagos,
    RevolutPay,
    SamsungPay,
    Seicomart,
    Sepa,
    SepaBankTransfer,
    SepaGuarenteedDebit,
    SevenEleven,
    Skrill,
    Sofort,
    Swish,
    Ted,
    TouchNGo,
    Trustly,
    Twint,
    UpiCollect,
    UpiIntent,
    UpiQr,
    Venmo,
    VietQr,
    Vipps,
    Walley,
    WeChatPay,
    Wero,
}

impl PaymentMethodType {
    /// Hyperswitch's spelling: `credit`, `google_pay`.
    pub fn hyperswitch_name(self) -> &'static str {
        self.into()
    }

    /// The payment methods this type belongs under: Hyperswitch's own mapping, plus the pairs its
    /// routing rules accept (ACH as a bank debit or a bank transfer, for instance).
    pub fn payment_methods(self) -> &'static [PaymentMethod] {
        match self {
            Self::Ach => &[PaymentMethod::BankDebit, PaymentMethod::BankTransfer],
            Self::Affirm => &[PaymentMethod::PayLater],
            Self::AfterpayClearpay => &[PaymentMethod::PayLater],
            Self::Alfamart => &[PaymentMethod::Voucher],
            Self::AliPay => &[PaymentMethod::Wallet],
            Self::AliPayHk => &[PaymentMethod::Wallet],
            Self::Alma => &[PaymentMethod::PayLater],
            Self::AmazonPay => &[PaymentMethod::Wallet],
            Self::ApplePay => &[PaymentMethod::Wallet],
            Self::Atome => &[PaymentMethod::PayLater],
            Self::Bacs => &[PaymentMethod::BankDebit, PaymentMethod::BankTransfer],
            Self::BancontactCard => &[PaymentMethod::BankRedirect],
            Self::BcaBankTransfer => &[PaymentMethod::BankTransfer],
            Self::Becs => &[PaymentMethod::BankDebit],
            Self::Benefit => &[PaymentMethod::CardRedirect],
            Self::BhnCardNetwork => &[PaymentMethod::GiftCard],
            Self::Bizum => &[PaymentMethod::BankRedirect],
            Self::Blik => &[PaymentMethod::BankRedirect],
            Self::Bluecode => &[PaymentMethod::Wallet],
            Self::BniVa => &[PaymentMethod::BankTransfer],
            Self::Boleto => &[PaymentMethod::Voucher],
            Self::Breadpay => &[PaymentMethod::PayLater],
            Self::BriVa => &[PaymentMethod::BankTransfer],
            Self::Card => &[PaymentMethod::Card],
            Self::CardRedirect => &[PaymentMethod::CardRedirect],
            Self::Cashapp => &[PaymentMethod::Wallet],
            Self::CimbVa => &[PaymentMethod::BankTransfer],
            Self::ClassicReward => &[PaymentMethod::Reward],
            Self::Credit => &[PaymentMethod::Card],
            Self::CryptoCurrency => &[PaymentMethod::Crypto],
            Self::Dana => &[PaymentMethod::Wallet],
            Self::DanamonVa => &[PaymentMethod::BankTransfer],
            Self::Debit => &[PaymentMethod::Card],
            Self::DirectCarrierBilling => &[PaymentMethod::MobilePayment],
            Self::DuitNow => &[PaymentMethod::RealTimePayment],
            Self::Efecty => &[PaymentMethod::Voucher],
            Self::Eft => &[PaymentMethod::BankRedirect],
            Self::EftDebitOrder => &[PaymentMethod::BankDebit],
            Self::Eps => &[PaymentMethod::BankRedirect],
            Self::Evoucher => &[PaymentMethod::Reward],
            Self::FamilyMart => &[PaymentMethod::Voucher],
            Self::Flexiti => &[PaymentMethod::PayLater],
            Self::Fps => &[PaymentMethod::RealTimePayment],
            Self::Gcash => &[PaymentMethod::Wallet],
            Self::Giropay => &[PaymentMethod::BankRedirect],
            Self::Givex => &[PaymentMethod::GiftCard],
            Self::GooglePay => &[PaymentMethod::Wallet],
            Self::GoPay => &[PaymentMethod::Wallet],
            Self::Ideal => &[PaymentMethod::BankRedirect],
            Self::Indomaret => &[PaymentMethod::Voucher],
            Self::IndonesianBankTransfer => &[PaymentMethod::BankTransfer],
            Self::InstantBankTransfer => &[PaymentMethod::BankTransfer],
            Self::InstantBankTransferFinland => &[PaymentMethod::BankTransfer],
            Self::InstantBankTransferPoland => &[PaymentMethod::BankTransfer],
            Self::Interac => &[PaymentMethod::BankRedirect],
            Self::KakaoPay => &[PaymentMethod::Wallet],
            Self::Klarna => &[PaymentMethod::PayLater],
            Self::Knet => &[PaymentMethod::CardRedirect],
            Self::Lawson => &[PaymentMethod::Voucher],
            Self::LocalBankRedirect => &[PaymentMethod::BankRedirect],
            Self::LocalBankTransfer => &[PaymentMethod::BankTransfer],
            Self::MandiriVa => &[PaymentMethod::BankTransfer],
            Self::MbWay => &[PaymentMethod::Wallet],
            Self::Mifinity => &[PaymentMethod::Wallet],
            Self::MiniStop => &[PaymentMethod::Voucher],
            Self::MobilePay => &[PaymentMethod::Wallet],
            Self::Momo => &[PaymentMethod::Wallet],
            Self::MomoAtm => &[PaymentMethod::CardRedirect],
            Self::Multibanco => &[PaymentMethod::BankTransfer],
            Self::Neteller => &[PaymentMethod::Wallet],
            Self::NetworkToken => &[PaymentMethod::NetworkToken],
            Self::OnlineBankingCzechRepublic => &[PaymentMethod::BankRedirect],
            Self::OnlineBankingFinland => &[PaymentMethod::BankRedirect],
            Self::OnlineBankingFpx => &[PaymentMethod::BankRedirect],
            Self::OnlineBankingPoland => &[PaymentMethod::BankRedirect],
            Self::OnlineBankingSlovakia => &[PaymentMethod::BankRedirect],
            Self::OnlineBankingThailand => &[PaymentMethod::BankRedirect],
            Self::OpenBanking => &[PaymentMethod::BankRedirect],
            Self::OpenBankingPis => &[PaymentMethod::BankRedirect, PaymentMethod::OpenBanking],
            Self::OpenBankingUk => &[PaymentMethod::BankRedirect],
            Self::Oxxo => &[PaymentMethod::Voucher],
            Self::PagoEfectivo => &[PaymentMethod::Voucher],
            Self::PayBright => &[PaymentMethod::PayLater],
            Self::PayEasy => &[PaymentMethod::Voucher],
            Self::Payjustnow => &[PaymentMethod::PayLater],
            Self::Paypal => &[PaymentMethod::Wallet],
            Self::PaySafeCard => &[PaymentMethod::GiftCard],
            Self::Paysera => &[PaymentMethod::Wallet],
            Self::Payshap => &[PaymentMethod::BankTransfer],
            Self::PayshapProxy => &[PaymentMethod::BankTransfer],
            Self::Paze => &[PaymentMethod::Wallet],
            Self::PermataBankTransfer => &[PaymentMethod::BankTransfer],
            Self::Pix => &[PaymentMethod::BankTransfer],
            Self::PixAutomaticoPush => &[PaymentMethod::BankTransfer],
            Self::PixAutomaticoQr => &[PaymentMethod::BankTransfer],
            Self::PixEmv => &[PaymentMethod::BankTransfer],
            Self::PixKey => &[PaymentMethod::BankTransfer],
            Self::PixQr => &[PaymentMethod::BankTransfer],
            Self::PromptPay => &[PaymentMethod::RealTimePayment],
            Self::Przelewy24 => &[PaymentMethod::BankRedirect],
            Self::Pse => &[PaymentMethod::BankTransfer],
            Self::Qris => &[PaymentMethod::RealTimePayment],
            Self::RedCompra => &[PaymentMethod::Voucher],
            Self::RedPagos => &[PaymentMethod::Voucher],
            Self::RevolutPay => &[PaymentMethod::Wallet],
            Self::SamsungPay => &[PaymentMethod::Wallet],
            Self::Seicomart => &[PaymentMethod::Voucher],
            Self::Sepa => &[PaymentMethod::BankDebit, PaymentMethod::BankTransfer],
            Self::SepaBankTransfer => &[PaymentMethod::BankTransfer],
            Self::SepaGuarenteedDebit => &[PaymentMethod::BankDebit],
            Self::SevenEleven => &[PaymentMethod::Voucher],
            Self::Skrill => &[PaymentMethod::Wallet],
            Self::Sofort => &[PaymentMethod::BankRedirect],
            Self::Swish => &[PaymentMethod::BankRedirect, PaymentMethod::Wallet],
            Self::Ted => &[PaymentMethod::BankTransfer],
            Self::TouchNGo => &[PaymentMethod::Wallet],
            Self::Trustly => &[PaymentMethod::BankRedirect],
            Self::Twint => &[PaymentMethod::Wallet],
            Self::UpiCollect => &[PaymentMethod::Upi],
            Self::UpiIntent => &[PaymentMethod::Upi],
            Self::UpiQr => &[PaymentMethod::Upi],
            Self::Venmo => &[PaymentMethod::Wallet],
            Self::VietQr => &[PaymentMethod::RealTimePayment],
            Self::Vipps => &[PaymentMethod::Wallet],
            Self::Walley => &[PaymentMethod::PayLater],
            Self::WeChatPay => &[PaymentMethod::Wallet],
            Self::Wero => &[PaymentMethod::Wallet],
        }
    }
}

/// The specific method within a category: one of Hyperswitch's types, or a value it does not
/// define (an Euler method such as `UPI_PAY`, or a type newer than this build).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MethodType {
    Hyperswitch(PaymentMethodType),
    Other(String),
}

impl MethodType {
    /// Parses anything: Hyperswitch's types by name, anything else kept upper-cased. Empty and
    /// `null` are no type.
    pub fn parse_lenient(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("null") {
            return None;
        }
        Some(
            PaymentMethodType::from_str(value)
                .map(Self::Hyperswitch)
                .unwrap_or_else(|_| Self::Other(value.to_ascii_uppercase())),
        )
    }

    /// The upper-case code the engine keys and compares on: `CREDIT`, `GOOGLE_PAY`, `UPI_PAY`.
    pub fn code(&self) -> Cow<'static, str> {
        match self {
            Self::Hyperswitch(method_type) => {
                Cow::Owned(method_type.hyperswitch_name().to_ascii_uppercase())
            }
            Self::Other(code) => Cow::Owned(code.clone()),
        }
    }
}

/// Why a request's payment method and type were not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentError {
    /// `paymentMethod` is not one of Hyperswitch's payment methods.
    UnknownMethod { method: String, swapped: bool },
    /// `paymentMethodType` is not one of Hyperswitch's payment method types.
    UnknownType { method_type: String, swapped: bool },
    /// The type does not belong under the method (`card` with `google_pay`).
    TypeNotInMethod {
        method: PaymentMethod,
        method_type: PaymentMethodType,
    },
}

impl fmt::Display for InstrumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SWAP_HINT: &str = " paymentMethod and paymentMethodType follow Hyperswitch's \
            meaning: send the category (card) as paymentMethod and the specific method (credit) \
            as paymentMethodType; this request looks swapped.";
        match self {
            Self::UnknownMethod { method, swapped } => write!(
                f,
                "paymentMethod `{method}` is not a payment method (card, wallet, upi...).{}",
                if *swapped { SWAP_HINT } else { "" }
            ),
            Self::UnknownType {
                method_type,
                swapped,
            } => write!(
                f,
                "paymentMethodType `{method_type}` is not a payment method type (credit, debit, \
                 google_pay...).{}",
                if *swapped { SWAP_HINT } else { "" }
            ),
            Self::TypeNotInMethod {
                method,
                method_type,
            } => write!(
                f,
                "paymentMethodType `{}` is not a {} payment method type.",
                method_type.hyperswitch_name(),
                method.hyperswitch_name()
            ),
        }
    }
}

/// A payment's instrument: its category and, when known, the specific method within it. Built
/// only by the parsers, so a method and type that do not belong together never reach the engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "InstrumentWire", into = "InstrumentWire")]
pub struct PaymentInstrument {
    method: PaymentMethod,
    method_type: Option<MethodType>,
}

impl PaymentInstrument {
    /// Hyperswitch's format, checked: `method` is one of its payment methods and `method_type`
    /// one of its types, belonging to that method. Case is ignored.
    pub fn from_hyperswitch(method: &str, method_type: &str) -> Result<Self, InstrumentError> {
        let swapped = PaymentMethod::from_hyperswitch(method_type).is_some()
            && PaymentMethod::from_hyperswitch(method).is_none();
        let payment_method =
            PaymentMethod::from_hyperswitch(method).ok_or(InstrumentError::UnknownMethod {
                method: method.to_string(),
                swapped,
            })?;
        let payment_method_type =
            PaymentMethodType::from_str(method_type.trim()).map_err(|_| {
                InstrumentError::UnknownType {
                    method_type: method_type.to_string(),
                    swapped,
                }
            })?;
        if !payment_method_type
            .payment_methods()
            .contains(&payment_method)
        {
            return Err(InstrumentError::TypeNotInMethod {
                method: payment_method,
                method_type: payment_method_type,
            });
        }
        Ok(Self {
            method: payment_method,
            method_type: Some(MethodType::Hyperswitch(payment_method_type)),
        })
    }

    /// Hyperswitch's format, unchecked, for values Hyperswitch already validated and for stored
    /// data: an unknown method is [`PaymentMethod::Unknown`] and an unknown type is kept as is.
    pub fn from_hyperswitch_lenient(method: Option<&str>, method_type: Option<&str>) -> Self {
        Self {
            method: method
                .and_then(|method| {
                    PaymentMethod::from_hyperswitch(method)
                        .or_else(|| PaymentMethod::from_str(method.trim()).ok())
                })
                .unwrap_or(PaymentMethod::Unknown),
            method_type: method_type.and_then(MethodType::parse_lenient),
        }
    }

    /// Euler's legacy format: the category in `paymentMethodType`, the specific method in
    /// `paymentMethod`.
    pub fn from_euler(payment_method_type: &str, payment_method: &str) -> Self {
        Self {
            method: PaymentMethod::from_euler(payment_method_type),
            method_type: MethodType::parse_lenient(payment_method),
        }
    }

    pub fn method(&self) -> PaymentMethod {
        self.method
    }

    pub fn method_type(&self) -> Option<&MethodType> {
        self.method_type.as_ref()
    }

    /// The category's code, as the engine keys on it: `CARD`.
    pub fn method_code(&self) -> &'static str {
        self.method.code()
    }

    /// The specific method's code, as the engine keys on it: `CREDIT`, or empty when unknown.
    pub fn method_type_code(&self) -> Cow<'static, str> {
        self.method_type
            .as_ref()
            .map(MethodType::code)
            .unwrap_or(Cow::Borrowed(""))
    }

    pub fn is(&self, method: PaymentMethod) -> bool {
        self.method == method
    }
}

/// The stored form, under the request's field names: `{ "paymentMethod": "CARD",
/// "paymentMethodType": "CREDIT" }`.
#[derive(Serialize, Deserialize)]
struct InstrumentWire {
    #[serde(rename = "paymentMethod", default)]
    payment_method: String,
    #[serde(rename = "paymentMethodType", default)]
    payment_method_type: String,
}

impl From<InstrumentWire> for PaymentInstrument {
    fn from(wire: InstrumentWire) -> Self {
        Self::from_hyperswitch_lenient(Some(&wire.payment_method), Some(&wire.payment_method_type))
    }
}

impl From<PaymentInstrument> for InstrumentWire {
    fn from(instrument: PaymentInstrument) -> Self {
        Self {
            payment_method: instrument.method_code().to_string(),
            payment_method_type: instrument.method_type_code().into_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn hyperswitch_pairs_parse_in_any_case() {
        let card = PaymentInstrument::from_hyperswitch("card", "CREDIT");
        assert_eq!(
            card.as_ref().map(PaymentInstrument::method),
            Ok(PaymentMethod::Card)
        );
        assert_eq!(
            card.as_ref().map(|i| i.method_type_code().into_owned()),
            Ok("CREDIT".to_string())
        );
        assert!(PaymentInstrument::from_hyperswitch("WALLET", "google_pay").is_ok());
        assert!(PaymentInstrument::from_hyperswitch("upi", "upi_collect").is_ok());
        // A type Hyperswitch's rules accept under two methods is valid under either.
        assert!(PaymentInstrument::from_hyperswitch("bank_debit", "ach").is_ok());
        assert!(PaymentInstrument::from_hyperswitch("bank_transfer", "ach").is_ok());
        assert!(PaymentInstrument::from_hyperswitch("reward", "classic").is_ok());
    }

    #[test]
    fn the_legacy_order_is_named_as_swapped() {
        let error = PaymentInstrument::from_hyperswitch("CREDIT", "CARD")
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(
            error.contains("paymentMethod `CREDIT` is not a payment method"),
            "{error}"
        );
        assert!(error.contains("looks swapped"), "{error}");
    }

    #[test]
    fn a_type_must_belong_to_its_method() {
        assert_eq!(
            PaymentInstrument::from_hyperswitch("card", "google_pay"),
            Err(InstrumentError::TypeNotInMethod {
                method: PaymentMethod::Card,
                method_type: PaymentMethodType::GooglePay,
            })
        );
        let error = PaymentInstrument::from_hyperswitch("card", "visa")
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(error.contains("paymentMethodType `visa`"), "{error}");
        assert!(!error.contains("looks swapped"), "{error}");
    }

    #[test]
    fn euler_carries_the_category_in_the_type_field() {
        let card = PaymentInstrument::from_euler("CARD", "VISA");
        assert_eq!(card.method(), PaymentMethod::Card);
        assert_eq!(card.method_type_code(), "VISA");
        assert_eq!(
            PaymentInstrument::from_euler("NB", "NB_HDFC").method(),
            PaymentMethod::NetBanking
        );
        assert_eq!(
            PaymentInstrument::from_euler("PAYLATER", "").method(),
            PaymentMethod::PayLater
        );
        assert_eq!(
            PaymentInstrument::from_euler("UPI", "UPI_PAY").method_type_code(),
            "UPI_PAY"
        );
    }

    #[test]
    fn lenient_parsing_keeps_what_it_cannot_name() {
        let instrument = PaymentInstrument::from_hyperswitch_lenient(Some("card"), Some("null"));
        assert_eq!(instrument.method(), PaymentMethod::Card);
        assert_eq!(instrument.method_type(), None);
        let instrument = PaymentInstrument::from_hyperswitch_lenient(None, Some("new_wallet"));
        assert_eq!(instrument.method(), PaymentMethod::Unknown);
        assert_eq!(instrument.method_type_code(), "NEW_WALLET");
    }

    #[test]
    fn the_stored_form_round_trips() -> Result<(), serde_json::Error> {
        let instrument = PaymentInstrument::from_hyperswitch_lenient(Some("card"), Some("debit"));
        let stored = serde_json::to_value(&instrument)?;
        assert_eq!(
            stored,
            serde_json::json!({ "paymentMethod": "CARD", "paymentMethodType": "DEBIT" })
        );
        assert_eq!(
            serde_json::from_value::<PaymentInstrument>(stored)?,
            instrument
        );
        Ok(())
    }

    /// Pinned to Hyperswitch's own enums (`common_enums::PaymentMethod` and
    /// `PaymentMethodType`): adding one there means adding it here.
    #[test]
    fn the_vocabulary_matches_hyperswitch() {
        let methods: Vec<String> = PaymentMethod::HYPERSWITCH
            .iter()
            .map(|method| method.hyperswitch_name())
            .collect();
        assert_eq!(
            methods,
            [
                "card",
                "card_redirect",
                "pay_later",
                "wallet",
                "bank_redirect",
                "bank_transfer",
                "crypto",
                "bank_debit",
                "reward",
                "real_time_payment",
                "upi",
                "voucher",
                "gift_card",
                "open_banking",
                "mobile_payment",
                "network_token",
            ]
        );
        assert_eq!(PaymentMethodType::iter().count(), 127);
        for method_type in PaymentMethodType::iter() {
            assert!(
                !method_type.payment_methods().is_empty(),
                "{method_type:?} has no payment method"
            );
            assert_eq!(
                PaymentMethodType::from_str(method_type.hyperswitch_name()),
                Ok(method_type)
            );
        }
        assert_eq!(
            PaymentMethodType::OpenBankingPis.hyperswitch_name(),
            "open_banking_pis"
        );
        assert_eq!(PaymentMethodType::TouchNGo.hyperswitch_name(), "touch_n_go");
        assert_eq!(
            PaymentMethodType::Przelewy24.hyperswitch_name(),
            "przelewy24"
        );
    }
}
