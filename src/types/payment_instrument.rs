//! A payment's instrument in one canonical form.
//!
//! The payment method is the category (`card`, `wallet`, `upi`) and the payment method type the
//! specific method within it (`credit`, `google_pay`, `upi_collect`). Every entry point parses its
//! payload into a [`PaymentInstrument`] once, and the engine works only on that:
//!
//! - The standard format, with [`PaymentInstrument::parse`] (for requests Decision Engine
//!   validates) or [`PaymentInstrument::parse_lenient`] (for values the caller already
//!   validated, such as routing rule parameters, and for stored data).
//! - The legacy transaction format, which carries the category in `paymentMethodType` and the
//!   specific method in `paymentMethod`, with [`PaymentInstrument::from_legacy_txn`].
//!
//! Values this build does not know are kept by name rather than rejected, so a method or type
//! newer than this build routes (generically) without a release. The enums below are the
//! vocabulary the dashboard offers and the categories the engine has rules for.
//!
//! Nothing downstream has to know which format a payment arrived in.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The payment method category. The sixteen standard categories come first; the rest are legacy
/// categories, which only [`PaymentInstrument::from_legacy_txn`] produces.
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
    /// No payment method, or one the engine has no rules for (kept by name in [`PaymentInstrument`]).
    Unknown,
}

impl PaymentMethod {
    /// The standard payment method categories.
    pub const STANDARD: [Self; 16] = [
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

    /// The lower-case name: `card`, `pay_later`. Only meaningful for the standard categories.
    pub fn name(self) -> String {
        self.code().to_ascii_lowercase()
    }

    pub fn is_standard(self) -> bool {
        Self::STANDARD.contains(&self)
    }

    /// One of the standard categories, in any case.
    pub fn parse_standard(value: &str) -> Option<Self> {
        Self::from_str(value.trim())
            .ok()
            .filter(|method| method.is_standard())
    }

    /// A legacy category code (`CARD`, `NB`, `PAYLATER`); anything else is [`Self::Unknown`].
    pub fn from_legacy(value: &str) -> Self {
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

/// The standard payment method types, by their lower-case names.
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
    /// The lower-case name: `credit`, `google_pay`.
    pub fn name(self) -> &'static str {
        self.into()
    }

    /// The categories this type belongs under, including the pairs routing rules accept (ACH as
    /// a bank debit or a bank transfer, for instance).
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

/// The routing keys that name payment methods, generated from the enums so rules, the dashboard
/// and request checks share one vocabulary: `payment_method` (the methods), `payment_method_type`
/// (every type) and one key per method listing its types (`card` → `credit, debit, card`).
pub fn payment_method_routing_keys() -> Vec<(String, Vec<String>)> {
    use strum::IntoEnumIterator;

    let type_names = |method_types: &mut dyn Iterator<Item = PaymentMethodType>| {
        method_types
            .map(|method_type| method_type.name().to_string())
            .collect::<Vec<_>>()
    };
    let mut keys = vec![
        (
            "payment_method".to_string(),
            PaymentMethod::STANDARD
                .iter()
                .map(|method| method.name())
                .collect(),
        ),
        (
            "payment_method_type".to_string(),
            type_names(&mut PaymentMethodType::iter()),
        ),
    ];
    for method in PaymentMethod::STANDARD {
        keys.push((
            method.name(),
            type_names(
                &mut PaymentMethodType::iter()
                    .filter(|method_type| method_type.payment_methods().contains(&method)),
            ),
        ));
    }
    keys
}

/// The category as the payment named it: one the engine knows, or a value it does not (a legacy
/// category this build has no variant for, or a category newer than this build).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Category {
    Known(PaymentMethod),
    Other(String),
}

impl Category {
    /// Parses anything: the engine's categories by name, anything else kept upper-cased. Empty
    /// and `null` are [`PaymentMethod::Unknown`].
    fn parse(value: &str) -> Self {
        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("null") {
            return Self::Known(PaymentMethod::Unknown);
        }
        PaymentMethod::from_str(value)
            .map(Self::Known)
            .unwrap_or_else(|_| Self::Other(value.to_ascii_uppercase()))
    }

    fn method(&self) -> PaymentMethod {
        match self {
            Self::Known(method) => *method,
            Self::Other(_) => PaymentMethod::Unknown,
        }
    }

    fn code(&self) -> Cow<'static, str> {
        match self {
            Self::Known(method) => Cow::Borrowed(method.code()),
            Self::Other(code) => Cow::Owned(code.clone()),
        }
    }
}

/// The specific method within a category: one of the standard types, or a value it does not
/// define (a legacy method such as `UPI_PAY`, or a type newer than this build).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MethodType {
    Known(PaymentMethodType),
    Other(String),
}

impl MethodType {
    /// Parses anything: the standard types by name, anything else kept upper-cased. Empty and
    /// `null` are no type.
    pub fn parse_lenient(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("null") {
            return None;
        }
        Some(
            PaymentMethodType::from_str(value)
                .map(Self::Known)
                .unwrap_or_else(|_| Self::Other(value.to_ascii_uppercase())),
        )
    }

    /// A legacy specific method: legacy spellings of standard types are mapped to them
    /// (`GOOGLEPAY` is `google_pay`), anything else is parsed as [`Self::parse_lenient`] does.
    fn from_legacy(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "GOOGLEPAY" => Some(Self::Known(PaymentMethodType::GooglePay)),
            _ => Self::parse_lenient(value),
        }
    }

    /// The upper-case code the engine keys and compares on: `CREDIT`, `GOOGLE_PAY`, `UPI_PAY`.
    pub fn code(&self) -> Cow<'static, str> {
        match self {
            Self::Known(method_type) => Cow::Owned(method_type.name().to_ascii_uppercase()),
            Self::Other(code) => Cow::Owned(code.clone()),
        }
    }
}

/// Why a request's payment method and type were not accepted. Values this build does not know
/// are accepted; only these mistakes are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentError {
    /// The request names no `paymentMethod`.
    MissingMethod,
    /// The two fields are the wrong way round: `paymentMethod` names a payment method type, or
    /// `paymentMethodType` names a payment method (`CREDIT` and `CARD`).
    Swapped { method: String, method_type: String },
    /// A known type under a known method it does not belong to (`card` with `google_pay`).
    TypeNotInMethod {
        method: PaymentMethod,
        method_type: PaymentMethodType,
    },
}

impl fmt::Display for InstrumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMethod => f.write_str(
                "paymentMethod is required: the payment method category (card, wallet, upi...).",
            ),
            Self::Swapped {
                method,
                method_type,
            } => write!(
                f,
                "paymentMethod `{method}` and paymentMethodType `{method_type}` look swapped. \
                 paymentMethod is the category and paymentMethodType the specific method: send the \
                 category (card) as paymentMethod and the specific method (credit) as \
                 paymentMethodType."
            ),
            Self::TypeNotInMethod {
                method,
                method_type,
            } => write!(
                f,
                "paymentMethodType `{}` is not a {} payment method type.",
                method_type.name(),
                method.name()
            ),
        }
    }
}

/// A payment's instrument: its category and, when known, the specific method within it. Built
/// only by the parsers, so a known method and type that do not belong together never reach the
/// engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "InstrumentWire", into = "InstrumentWire")]
pub struct PaymentInstrument {
    method: Category,
    method_type: Option<MethodType>,
}

/// A payment that names no method.
impl Default for PaymentInstrument {
    fn default() -> Self {
        Self {
            method: Category::Known(PaymentMethod::Unknown),
            method_type: None,
        }
    }
}

impl PaymentInstrument {
    /// The standard format, as Decision Engine accepts it from its callers. Case is ignored
    /// and values this build does not know are kept, so a method or type newer than this build
    /// routes without a release. Only clear mistakes are rejected: no method, the two fields
    /// swapped, or a known type under a known method it does not belong to.
    pub fn parse(method: &str, method_type: Option<&str>) -> Result<Self, InstrumentError> {
        let names_a_method = |value: &str| PaymentMethod::from_str(value.trim()).is_ok();
        let names_a_type = |value: &str| PaymentMethodType::from_str(value.trim()).is_ok();
        // Swapped: the method field names only a type (`credit`), or the type field names a
        // category while the method field does not (`visa` with `card`).
        let swapped = (names_a_type(method) && !names_a_method(method))
            || method_type
                .is_some_and(|method_type| names_a_method(method_type) && !names_a_method(method));
        let instrument = Self::parse_lenient(Some(method), method_type);
        if instrument.method == Category::Known(PaymentMethod::Unknown) {
            return Err(InstrumentError::MissingMethod);
        }
        if swapped {
            return Err(InstrumentError::Swapped {
                method: method.to_string(),
                method_type: method_type.unwrap_or_default().to_string(),
            });
        }
        if let (Category::Known(payment_method), Some(MethodType::Known(payment_method_type))) =
            (&instrument.method, &instrument.method_type)
        {
            if payment_method.is_standard()
                && !payment_method_type
                    .payment_methods()
                    .contains(payment_method)
            {
                return Err(InstrumentError::TypeNotInMethod {
                    method: *payment_method,
                    method_type: *payment_method_type,
                });
            }
        }
        Ok(instrument)
    }

    /// The standard format, unchecked, for values the caller already validated and for stored
    /// data: whatever the payment names is kept.
    pub fn parse_lenient(method: Option<&str>, method_type: Option<&str>) -> Self {
        Self {
            method: Category::parse(method.unwrap_or_default()),
            method_type: method_type.and_then(MethodType::parse_lenient),
        }
    }

    /// The legacy transaction format: the category in `paymentMethodType`, the specific method in
    /// `paymentMethod`.
    pub fn from_legacy_txn(payment_method_type: &str, payment_method: &str) -> Self {
        let method = match PaymentMethod::from_legacy(payment_method_type) {
            PaymentMethod::Unknown => Category::parse(payment_method_type),
            method => Category::Known(method),
        };
        Self {
            method,
            method_type: MethodType::from_legacy(payment_method),
        }
    }

    /// The category the engine routes on; [`PaymentMethod::Unknown`] for one it has no rules
    /// for.
    pub fn method(&self) -> PaymentMethod {
        self.method.method()
    }

    pub fn method_type(&self) -> Option<&MethodType> {
        self.method_type.as_ref()
    }

    /// The category's code, as the engine keys on it: `CARD`, or the value as named for a
    /// category this build does not know.
    pub fn method_code(&self) -> Cow<'static, str> {
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
        self.method == Category::Known(method)
    }

    pub fn is_type(&self, method_type: PaymentMethodType) -> bool {
        self.method_type == Some(MethodType::Known(method_type))
    }

    /// The analytics columns `(payment_method, payment_method_type)`: the codes, or `None` where
    /// the payment named nothing.
    pub fn analytics_columns(&self) -> (Option<String>, Option<String>) {
        let method_type = self.method_type_code();
        (
            (self.method != Category::Known(PaymentMethod::Unknown))
                .then(|| self.method_code().into_owned()),
            (!method_type.is_empty()).then(|| method_type.into_owned()),
        )
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
        Self::parse_lenient(Some(&wire.payment_method), Some(&wire.payment_method_type))
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
    fn pairs_parse_in_any_case() {
        let card = PaymentInstrument::parse("card", Some("CREDIT"));
        assert_eq!(
            card.as_ref().map(PaymentInstrument::method),
            Ok(PaymentMethod::Card)
        );
        assert_eq!(
            card.as_ref().map(|i| i.method_type_code().into_owned()),
            Ok("CREDIT".to_string())
        );
        assert!(PaymentInstrument::parse("WALLET", Some("google_pay")).is_ok());
        assert!(PaymentInstrument::parse("upi", Some("upi_collect")).is_ok());
        assert!(PaymentInstrument::parse("wallet", None).is_ok());
        // A type routing rules accept under two methods is valid under either.
        assert!(PaymentInstrument::parse("bank_debit", Some("ach")).is_ok());
        assert!(PaymentInstrument::parse("bank_transfer", Some("ach")).is_ok());
        assert!(PaymentInstrument::parse("reward", Some("classic")).is_ok());
    }

    /// A method or type newer than this build is routed by name, not rejected.
    #[test]
    fn unknown_values_are_kept() {
        let instrument = PaymentInstrument::parse("new_method", Some("new_type"));
        assert_eq!(
            instrument.as_ref().map(PaymentInstrument::method),
            Ok(PaymentMethod::Unknown)
        );
        assert_eq!(
            instrument.as_ref().map(|i| i.method_code().into_owned()),
            Ok("NEW_METHOD".to_string())
        );
        assert_eq!(
            instrument
                .as_ref()
                .map(|i| i.method_type_code().into_owned()),
            Ok("NEW_TYPE".to_string())
        );
        let card = PaymentInstrument::parse("card", Some("new_card_type"));
        assert_eq!(
            card.as_ref().map(PaymentInstrument::method),
            Ok(PaymentMethod::Card)
        );
    }

    #[test]
    fn the_legacy_order_is_named_as_swapped() {
        for (method, method_type) in [
            ("CREDIT", Some("CARD")),
            ("credit", None),
            ("VISA", Some("card")),
        ] {
            let error = PaymentInstrument::parse(method, method_type)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
            assert!(error.contains("look swapped"), "{method}: {error}");
        }
    }

    #[test]
    fn a_method_is_required() {
        assert_eq!(
            PaymentInstrument::parse(" ", Some("credit")),
            Err(InstrumentError::MissingMethod)
        );
    }

    #[test]
    fn a_known_type_must_belong_to_its_known_method() {
        assert_eq!(
            PaymentInstrument::parse("card", Some("google_pay")),
            Err(InstrumentError::TypeNotInMethod {
                method: PaymentMethod::Card,
                method_type: PaymentMethodType::GooglePay,
            })
        );
        let error = PaymentInstrument::parse("card", Some("google_pay"))
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(
            error.contains("is not a card payment method type"),
            "{error}"
        );
    }

    #[test]
    fn legacy_txns_carry_the_category_in_the_type_field() {
        let card = PaymentInstrument::from_legacy_txn("CARD", "VISA");
        assert_eq!(card.method(), PaymentMethod::Card);
        assert_eq!(card.method_type_code(), "VISA");
        assert_eq!(
            PaymentInstrument::from_legacy_txn("NB", "NB_HDFC").method(),
            PaymentMethod::NetBanking
        );
        assert_eq!(
            PaymentInstrument::from_legacy_txn("PAYLATER", "").method(),
            PaymentMethod::PayLater
        );
        assert_eq!(
            PaymentInstrument::from_legacy_txn("UPI", "UPI_PAY").method_type_code(),
            "UPI_PAY"
        );
        // A legacy spelling of a standard type is mapped to it.
        assert!(PaymentInstrument::from_legacy_txn("WALLET", "GOOGLEPAY")
            .is_type(PaymentMethodType::GooglePay));
        // A category this build has no variant for keeps its name.
        let other = PaymentInstrument::from_legacy_txn("EMI", "HDFC");
        assert_eq!(other.method(), PaymentMethod::Unknown);
        assert_eq!(other.method_code(), "EMI");
    }

    #[test]
    fn lenient_parsing_keeps_what_it_cannot_name() {
        let instrument = PaymentInstrument::parse_lenient(Some("card"), Some("null"));
        assert_eq!(instrument.method(), PaymentMethod::Card);
        assert_eq!(instrument.method_type(), None);
        let instrument = PaymentInstrument::parse_lenient(None, Some("new_wallet"));
        assert_eq!(
            instrument,
            PaymentInstrument {
                method_type: MethodType::parse_lenient("new_wallet"),
                ..PaymentInstrument::default()
            }
        );
        assert_eq!(instrument.method_code(), "UNKNOWN");
        assert_eq!(instrument.method_type_code(), "NEW_WALLET");
        assert_eq!(
            instrument.analytics_columns(),
            (None, Some("NEW_WALLET".to_string()))
        );
    }

    #[test]
    fn the_stored_form_round_trips() -> Result<(), serde_json::Error> {
        let instrument = PaymentInstrument::parse_lenient(Some("card"), Some("debit"));
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

    #[test]
    fn routing_keys_list_each_method_with_its_types() {
        let keys: std::collections::HashMap<String, Vec<String>> =
            payment_method_routing_keys().into_iter().collect();
        let sorted = |key: &str| {
            let mut values = keys.get(key).cloned().unwrap_or_default();
            values.sort();
            values
        };
        assert_eq!(keys.get("payment_method").map(Vec::len), Some(16));
        assert_eq!(keys.get("payment_method_type").map(Vec::len), Some(127));
        assert_eq!(sorted("card"), ["card", "credit", "debit"]);
        assert_eq!(sorted("upi"), ["upi_collect", "upi_intent", "upi_qr"]);
        assert!(sorted("bank_debit").contains(&"ach".to_string()));
        assert!(sorted("bank_transfer").contains(&"ach".to_string()));
        assert_eq!(sorted("reward"), ["classic_reward", "evoucher"]);
    }

    /// Pins the standard vocabulary, which is kept in step with Hyperswitch's
    /// `common_enums::PaymentMethod` and `PaymentMethodType` so its payments parse as known.
    #[test]
    fn the_standard_vocabulary_is_stable() {
        let methods: Vec<String> = PaymentMethod::STANDARD
            .iter()
            .map(|method| method.name())
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
                PaymentMethodType::from_str(method_type.name()),
                Ok(method_type)
            );
        }
        assert_eq!(PaymentMethodType::OpenBankingPis.name(), "open_banking_pis");
        assert_eq!(PaymentMethodType::TouchNGo.name(), "touch_n_go");
        assert_eq!(PaymentMethodType::Przelewy24.name(), "przelewy24");
    }
}
