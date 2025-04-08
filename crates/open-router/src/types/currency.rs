use serde::{Deserialize, Serialize};

use crate::error::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Currency {
    AED,
    ALL,
    AMD,
    ARS,
    AUD,
    AWG,
    AZN,
    BBD,
    BDT,
    BHD,
    BMD,
    BND,
    BOB,
    BRL,
    BSD,
    BWP,
    BZD,
    CAD,
    CHF,
    CNY,
    COP,
    CRC,
    CUP,
    CZK,
    DKK,
    DOP,
    DZD,
    EGP,
    ETB,
    EUR,
    FJD,
    GBP,
    GHS,
    GIP,
    GMD,
    GTQ,
    GYD,
    HKD,
    HNL,
    HRK,
    HTG,
    HUF,
    IDR,
    ILS,
    INR,
    JMD,
    JOD,
    JPY,
    KES,
    KGS,
    KHR,
    KRW,
    KWD,
    KYD,
    KZT,
    LAK,
    LBP,
    LKR,
    LRD,
    LSL,
    MAD,
    MDL,
    MKD,
    MMK,
    MNT,
    MOP,
    MUR,
    MVR,
    MWK,
    MXN,
    MYR,
    NAD,
    NGN,
    NIO,
    NOK,
    NPR,
    NZD,
    OMR,
    PEN,
    PGK,
    PHP,
    PKR,
    PLN,
    QAR,
    RUB,
    SAR,
    SCR,
    SEK,
    SGD,
    SLL,
    SOS,
    SSP,
    SVC,
    SZL,
    THB,
    TTD,
    TWD,
    TZS,
    USD,
    UYU,
    UZS,
    YER,
    ZAR,
    TRY,
    AFN,
    ANG,
    AOA,
    BAM,
    BGN,
    BIF,
    BOV,
    BTN,
    BYN,
    CDF,
    CHE,
    CHW,
    CLF,
    CLP,
    COU,
    CUC,
    CVE,
    DJF,
    ERN,
    FKP,
    GEL,
    GNF,
    IQD,
    IRR,
    ISK,
    KMF,
    KPW,
    LYD,
    MGA,
    MRO,
    MXV,
    MZN,
    PAB,
    PYG,
    RSD,
    RWF,
    SBD,
    SDG,
    SHP,
    SRD,
    STD,
    SYP,
    TJS,
    TMT,
    TND,
    TOP,
    UAH,
    UGX,
    USN,
    UYI,
    VEF,
    VND,
    VUV,
    WST,
    XAF,
    XAG,
    XAU,
    XBA,
    XBB,
    XBC,
    XBD,
    XCD,
    XDR,
    XOF,
    XPD,
    XPF,
    XPT,
    XSU,
    XTS,
    XUA,
    XXX,
    ZMW,
    ZWL,
}

impl Currency {
    pub fn to_text(&self) -> &str {
        match self {
            Self::AED => "AED",
            Self::ALL => "ALL",
            Self::AMD => "AMD",
            Self::ARS => "ARS",
            Self::AUD => "AUD",
            Self::AWG => "AWG",
            Self::AZN => "AZN",
            Self::BBD => "BBD",
            Self::BDT => "BDT",
            Self::BHD => "BHD",
            Self::BMD => "BMD",
            Self::BND => "BND",
            Self::BOB => "BOB",
            Self::BRL => "BRL",
            Self::BSD => "BSD",
            Self::BWP => "BWP",
            Self::BZD => "BZD",
            Self::CAD => "CAD",
            Self::CHF => "CHF",
            Self::CNY => "CNY",
            Self::COP => "COP",
            Self::CRC => "CRC",
            Self::CUP => "CUP",
            Self::CZK => "CZK",
            Self::DKK => "DKK",
            Self::DOP => "DOP",
            Self::DZD => "DZD",
            Self::EGP => "EGP",
            Self::ETB => "ETB",
            Self::EUR => "EUR",
            Self::FJD => "FJD",
            Self::GBP => "GBP",
            Self::GHS => "GHS",
            Self::GIP => "GIP",
            Self::GMD => "GMD",
            Self::GTQ => "GTQ",
            Self::GYD => "GYD",
            Self::HKD => "HKD",
            Self::HNL => "HNL",
            Self::HRK => "HRK",
            Self::HTG => "HTG",
            Self::HUF => "HUF",
            Self::IDR => "IDR",
            Self::ILS => "ILS",
            Self::INR => "INR",
            Self::JMD => "JMD",
            Self::JOD => "JOD",
            Self::JPY => "JPY",
            Self::KES => "KES",
            Self::KGS => "KGS",
            Self::KHR => "KHR",
            Self::KRW => "KRW",
            Self::KWD => "KWD",
            Self::KYD => "KYD",
            Self::KZT => "KZT",
            Self::LAK => "LAK",
            Self::LBP => "LBP",
            Self::LKR => "LKR",
            Self::LRD => "LRD",
            Self::LSL => "LSL",
            Self::MAD => "MAD",
            Self::MDL => "MDL",
            Self::MKD => "MKD",
            Self::MMK => "MMK",
            Self::MNT => "MNT",
            Self::MOP => "MOP",
            Self::MUR => "MUR",
            Self::MVR => "MVR",
            Self::MWK => "MWK",
            Self::MXN => "MXN",
            Self::MYR => "MYR",
            Self::NAD => "NAD",
            Self::NGN => "NGN",
            Self::NIO => "NIO",
            Self::NOK => "NOK",
            Self::NPR => "NPR",
            Self::NZD => "NZD",
            Self::OMR => "OMR",
            Self::PEN => "PEN",
            Self::PGK => "PGK",
            Self::PHP => "PHP",
            Self::PKR => "PKR",
            Self::PLN => "PLN",
            Self::QAR => "QAR",
            Self::RUB => "RUB",
            Self::SAR => "SAR",
            Self::SCR => "SCR",
            Self::SEK => "SEK",
            Self::SGD => "SGD",
            Self::SLL => "SLL",
            Self::SOS => "SOS",
            Self::SSP => "SSP",
            Self::SVC => "SVC",
            Self::SZL => "SZL",
            Self::THB => "THB",
            Self::TTD => "TTD",
            Self::TWD => "TWD",
            Self::TZS => "TZS",
            Self::USD => "USD",
            Self::UYU => "UYU",
            Self::UZS => "UZS",
            Self::YER => "YER",
            Self::ZAR => "ZAR",
            Self::TRY => "TRY",
            Self::AFN => "AFN",
            Self::ANG => "ANG",
            Self::AOA => "AOA",
            Self::BAM => "BAM",
            Self::BGN => "BGN",
            Self::BIF => "BIF",
            Self::BOV => "BOV",
            Self::BTN => "BTN",
            Self::BYN => "BYN",
            Self::CDF => "CDF",
            Self::CHE => "CHE",
            Self::CHW => "CHW",
            Self::CLF => "CLF",
            Self::CLP => "CLP",
            Self::COU => "COU",
            Self::CUC => "CUC",
            Self::CVE => "CVE",
            Self::DJF => "DJF",
            Self::ERN => "ERN",
            Self::FKP => "FKP",
            Self::GEL => "GEL",
            Self::GNF => "GNF",
            Self::IQD => "IQD",
            Self::IRR => "IRR",
            Self::ISK => "ISK",
            Self::KMF => "KMF",
            Self::KPW => "KPW",
            Self::LYD => "LYD",
            Self::MGA => "MGA",
            Self::MRO => "MRO",
            Self::MXV => "MXV",
            Self::MZN => "MZN",
            Self::PAB => "PAB",
            Self::PYG => "PYG",
            Self::RSD => "RSD",
            Self::RWF => "RWF",
            Self::SBD => "SBD",
            Self::SDG => "SDG",
            Self::SHP => "SHP",
            Self::SRD => "SRD",
            Self::STD => "STD",
            Self::SYP => "SYP",
            Self::TJS => "TJS",
            Self::TMT => "TMT",
            Self::TND => "TND",
            Self::TOP => "TOP",
            Self::UAH => "UAH",
            Self::UGX => "UGX",
            Self::USN => "USN",
            Self::UYI => "UYI",
            Self::VEF => "VEF",
            Self::VND => "VND",
            Self::VUV => "VUV",
            Self::WST => "WST",
            Self::XAF => "XAF",
            Self::XAG => "XAG",
            Self::XAU => "XAU",
            Self::XBA => "XBA",
            Self::XBB => "XBB",
            Self::XBC => "XBC",
            Self::XBD => "XBD",
            Self::XCD => "XCD",
            Self::XDR => "XDR",
            Self::XOF => "XOF",
            Self::XPD => "XPD",
            Self::XPF => "XPF",
            Self::XPT => "XPT",
            Self::XSU => "XSU",
            Self::XTS => "XTS",
            Self::XUA => "XUA",
            Self::XXX => "XXX",
            Self::ZMW => "ZMW",
            Self::ZWL => "ZWL",
        }
    }

    pub fn text_to_curr(text: &str) -> Result<Self, ApiError> {
        match text {
            "AED" => Ok(Self::AED),
            "ALL" => Ok(Self::ALL),
            "AMD" => Ok(Self::AMD),
            "ARS" => Ok(Self::ARS),
            "AUD" => Ok(Self::AUD),
            "AWG" => Ok(Self::AWG),
            "AZN" => Ok(Self::AZN),
            "BBD" => Ok(Self::BBD),
            "BDT" => Ok(Self::BDT),
            "BHD" => Ok(Self::BHD),
            "BMD" => Ok(Self::BMD),
            "BND" => Ok(Self::BND),
            "BOB" => Ok(Self::BOB),
            "BRL" => Ok(Self::BRL),
            "BSD" => Ok(Self::BSD),
            "BWP" => Ok(Self::BWP),
            "BZD" => Ok(Self::BZD),
            "CAD" => Ok(Self::CAD),
            "CHF" => Ok(Self::CHF),
            "CNY" => Ok(Self::CNY),
            "COP" => Ok(Self::COP),
            "CRC" => Ok(Self::CRC),
            "CUP" => Ok(Self::CUP),
            "CZK" => Ok(Self::CZK),
            "DKK" => Ok(Self::DKK),
            "DOP" => Ok(Self::DOP),
            "DZD" => Ok(Self::DZD),
            "EGP" => Ok(Self::EGP),
            "ETB" => Ok(Self::ETB),
            "EUR" => Ok(Self::EUR),
            "FJD" => Ok(Self::FJD),
            "GBP" => Ok(Self::GBP),
            "GHS" => Ok(Self::GHS),
            "GIP" => Ok(Self::GIP),
            "GMD" => Ok(Self::GMD),
            "GTQ" => Ok(Self::GTQ),
            "GYD" => Ok(Self::GYD),
            "HKD" => Ok(Self::HKD),
            "HNL" => Ok(Self::HNL),
            "HRK" => Ok(Self::HRK),
            "HTG" => Ok(Self::HTG),
            "HUF" => Ok(Self::HUF),
            "IDR" => Ok(Self::IDR),
            "ILS" => Ok(Self::ILS),
            "INR" => Ok(Self::INR),
            "JMD" => Ok(Self::JMD),
            "JOD" => Ok(Self::JOD),
            "JPY" => Ok(Self::JPY),
            "KES" => Ok(Self::KES),
            "KGS" => Ok(Self::KGS),
            "KHR" => Ok(Self::KHR),
            "KRW" => Ok(Self::KRW),
            "KWD" => Ok(Self::KWD),
            "KYD" => Ok(Self::KYD),
            "KZT" => Ok(Self::KZT),
            "LAK" => Ok(Self::LAK),
            "LBP" => Ok(Self::LBP),
            "LKR" => Ok(Self::LKR),
            "LRD" => Ok(Self::LRD),
            "LSL" => Ok(Self::LSL),
            "MAD" => Ok(Self::MAD),
            "MDL" => Ok(Self::MDL),
            "MKD" => Ok(Self::MKD),
            "MMK" => Ok(Self::MMK),
            "MNT" => Ok(Self::MNT),
            "MOP" => Ok(Self::MOP),
            "MUR" => Ok(Self::MUR),
            "MVR" => Ok(Self::MVR),
            "MWK" => Ok(Self::MWK),
            "MXN" => Ok(Self::MXN),
            "MYR" => Ok(Self::MYR),
            "NAD" => Ok(Self::NAD),
            "NGN" => Ok(Self::NGN),
            "NIO" => Ok(Self::NIO),
            "NOK" => Ok(Self::NOK),
            "NPR" => Ok(Self::NPR),
            "NZD" => Ok(Self::NZD),
            "OMR" => Ok(Self::OMR),
            "PEN" => Ok(Self::PEN),
            "PGK" => Ok(Self::PGK),
            "PHP" => Ok(Self::PHP),
            "PKR" => Ok(Self::PKR),
            "PLN" => Ok(Self::PLN),
            "QAR" => Ok(Self::QAR),
            "RUB" => Ok(Self::RUB),
            "SAR" => Ok(Self::SAR),
            "SCR" => Ok(Self::SCR),
            "SEK" => Ok(Self::SEK),
            "SGD" => Ok(Self::SGD),
            "SLL" => Ok(Self::SLL),
            "SOS" => Ok(Self::SOS),
            "SSP" => Ok(Self::SSP),
            "SVC" => Ok(Self::SVC),
            "SZL" => Ok(Self::SZL),
            "THB" => Ok(Self::THB),
            "TTD" => Ok(Self::TTD),
            "TWD" => Ok(Self::TWD),
            "TZS" => Ok(Self::TZS),
            "USD" => Ok(Self::USD),
            "UYU" => Ok(Self::UYU),
            "UZS" => Ok(Self::UZS),
            "YER" => Ok(Self::YER),
            "ZAR" => Ok(Self::ZAR),
            "TRY" => Ok(Self::TRY),
            "AFN" => Ok(Self::AFN),
            "ANG" => Ok(Self::ANG),
            "AOA" => Ok(Self::AOA),
            "BAM" => Ok(Self::BAM),
            "BGN" => Ok(Self::BGN),
            "BIF" => Ok(Self::BIF),
            "BOV" => Ok(Self::BOV),
            "BTN" => Ok(Self::BTN),
            "BYN" => Ok(Self::BYN),
            "CDF" => Ok(Self::CDF),
            "CHE" => Ok(Self::CHE),
            "CHW" => Ok(Self::CHW),
            "CLF" => Ok(Self::CLF),
            "CLP" => Ok(Self::CLP),
            "COU" => Ok(Self::COU),
            "CUC" => Ok(Self::CUC),
            "CVE" => Ok(Self::CVE),
            "DJF" => Ok(Self::DJF),
            "ERN" => Ok(Self::ERN),
            "FKP" => Ok(Self::FKP),
            "GEL" => Ok(Self::GEL),
            "GNF" => Ok(Self::GNF),
            "IQD" => Ok(Self::IQD),
            "IRR" => Ok(Self::IRR),
            "ISK" => Ok(Self::ISK),
            "KMF" => Ok(Self::KMF),
            "KPW" => Ok(Self::KPW),
            "LYD" => Ok(Self::LYD),
            "MGA" => Ok(Self::MGA),
            "MRO" => Ok(Self::MRO),
            "MXV" => Ok(Self::MXV),
            "MZN" => Ok(Self::MZN),
            "PAB" => Ok(Self::PAB),
            "PYG" => Ok(Self::PYG),
            "RSD" => Ok(Self::RSD),
            "RWF" => Ok(Self::RWF),
            "SBD" => Ok(Self::SBD),
            "SDG" => Ok(Self::SDG),
            "SHP" => Ok(Self::SHP),
            "SRD" => Ok(Self::SRD),
            "STD" => Ok(Self::STD),
            "SYP" => Ok(Self::SYP),
            "TJS" => Ok(Self::TJS),
            "TMT" => Ok(Self::TMT),
            "TND" => Ok(Self::TND),
            "TOP" => Ok(Self::TOP),
            "UAH" => Ok(Self::UAH),
            "UGX" => Ok(Self::UGX),
            "USN" => Ok(Self::USN),
            "UYI" => Ok(Self::UYI),
            "VEF" => Ok(Self::VEF),
            "VND" => Ok(Self::VND),
            "VUV" => Ok(Self::VUV),
            "WST" => Ok(Self::WST),
            "XAF" => Ok(Self::XAF),
            "XAG" => Ok(Self::XAG),
            "XAU" => Ok(Self::XAU),

            "XBA" => Ok(Self::XBA),
            "XBB" => Ok(Self::XBB),
            "XBC" => Ok(Self::XBC),
            "XBD" => Ok(Self::XBD),
            "XCD" => Ok(Self::XCD),
            "XDR" => Ok(Self::XDR),
            "XOF" => Ok(Self::XOF),
            "XPD" => Ok(Self::XPD),
            "XPF" => Ok(Self::XPF),
            "XPT" => Ok(Self::XPT),
            "XSU" => Ok(Self::XSU),
            "XTS" => Ok(Self::XTS),
            "XUA" => Ok(Self::XUA),
            "XXX" => Ok(Self::XXX),
            "ZMW" => Ok(Self::ZMW),
            "ZWL" => Ok(Self::ZWL),
            _ => Err(ApiError::ParsingError("Invalid Currency")),
        }
    }
}
