use serde::{Deserialize, Serialize};

use crate::decider::gatewaydecider;

use diesel::sql_types;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoBadgedCardRequest {
    pub merchant_category_code: MerchantCategoryCode,
    pub acquirer_country: CountryAlpha2,
    pub co_badged_card_data: Option<DebitRoutingData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MerchantCategoryCode {
    Mcc0001,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebitRoutingData {
    pub co_badged_card_networks: Vec<gatewaydecider::types::NETWORK>,
    pub issuer_country: CountryAlpha2,
    pub is_regulated: bool,
    pub regulated_name: Option<RegulatedName>,
    pub card_type: CardType,
}

#[derive(
    Clone,
    Debug,
    Eq,
    Default,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::EnumString,
    Copy,
    diesel::AsExpression, diesel::FromSqlRow
)]
#[diesel(sql_type = sql_types::Text)]
#[rustfmt::skip]
pub enum CountryAlpha2 {
    AF, AX, AL, DZ, AS, AD, AO, AI, AQ, AG, AR, AM, AW, AU, AT,
    AZ, BS, BH, BD, BB, BY, BE, BZ, BJ, BM, BT, BO, BQ, BA, BW,
    BV, BR, IO, BN, BG, BF, BI, KH, CM, CA, CV, KY, CF, TD, CL,
    CN, CX, CC, CO, KM, CG, CD, CK, CR, CI, HR, CU, CW, CY, CZ,
    DK, DJ, DM, DO, EC, EG, SV, GQ, ER, EE, ET, FK, FO, FJ, FI,
    FR, GF, PF, TF, GA, GM, GE, DE, GH, GI, GR, GL, GD, GP, GU,
    GT, GG, GN, GW, GY, HT, HM, VA, HN, HK, HU, IS, IN, ID, IR,
    IQ, IE, IM, IL, IT, JM, JP, JE, JO, KZ, KE, KI, KP, KR, KW,
    KG, LA, LV, LB, LS, LR, LY, LI, LT, LU, MO, MK, MG, MW, MY,
    MV, ML, MT, MH, MQ, MR, MU, YT, MX, FM, MD, MC, MN, ME, MS,
    MA, MZ, MM, NA, NR, NP, NL, NC, NZ, NI, NE, NG, NU, NF, MP,
    NO, OM, PK, PW, PS, PA, PG, PY, PE, PH, PN, PL, PT, PR, QA,
    RE, RO, RU, RW, BL, SH, KN, LC, MF, PM, VC, WS, SM, ST, SA,
    SN, RS, SC, SL, SG, SX, SK, SI, SB, SO, ZA, GS, SS, ES, LK,
    SD, SR, SJ, SZ, SE, CH, SY, TW, TJ, TZ, TH, TL, TG, TK, TO,
    TT, TN, TR, TM, TC, TV, UG, UA, AE, GB, UM, UY, UZ, VU,
    VE, VN, VG, VI, WF, EH, YE, ZM, ZW,
    #[default]
    US
}

#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    Copy,
    strum::Display,
    strum::EnumString,
    diesel::AsExpression,
    diesel::FromSqlRow,
)]
#[diesel(sql_type = sql_types::Text)]
pub enum RegulatedName {
    #[serde(rename = "GOVERNMENT NON-EXEMPT INTERCHANGE FEE (WITH FRAUD)")]
    NonExemptWithFraud,
    #[serde(rename = "GOVERNMENT EXEMPT INTERCHANGE FEE")]
    ExemptFraud,
}

#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    Copy,
    strum::Display,
    strum::EnumString,
    diesel::AsExpression,
    diesel::FromSqlRow,
)]
#[serde(rename_all = "snake_case")]
#[diesel(sql_type =  ::diesel::sql_types::Text)]
pub enum CardType {
    Credit,
    Debit,
}

#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    Copy,
    strum::Display,
    strum::EnumString,
    diesel::AsExpression,
    diesel::FromSqlRow,
)]
#[diesel(sql_type = sql_types::Text)]
#[serde(rename_all = "snake_case")]
pub enum PanOrToken {
    Pan,
    Token,
}

crate::impl_to_sql_from_sql_text_mysql!(CardType);
crate::impl_to_sql_from_sql_text_mysql!(RegulatedName);
crate::impl_to_sql_from_sql_text_mysql!(PanOrToken);
crate::impl_to_sql_from_sql_text_mysql!(CountryAlpha2);

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DebitRoutingOutput {
    pub co_badged_card_networks: Vec<gatewaydecider::types::NETWORK>,
    pub issuer_country: CountryAlpha2,
    pub is_regulated: bool,
    pub regulated_name: Option<RegulatedName>,
    pub card_type: CardType,
}

/// Implements the `ToSql` and `FromSql` traits on a type to allow it to be serialized/deserialized
/// to/from TEXT data in MySQL using `ToString`/`FromStr`.
#[macro_export]
macro_rules! impl_to_sql_from_sql_text_mysql {
    ($type:ty) => {
        impl ::diesel::serialize::ToSql<::diesel::sql_types::Text, ::diesel::mysql::Mysql>
            for $type
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut ::diesel::serialize::Output<'b, '_, ::diesel::mysql::Mysql>,
            ) -> ::diesel::serialize::Result {
                use ::std::io::Write;
                out.write_all(self.to_string().as_bytes())?;
                Ok(::diesel::serialize::IsNull::No)
            }
        }

        impl ::diesel::deserialize::FromSql<::diesel::sql_types::Text, ::diesel::mysql::Mysql>
            for $type
        {
            fn from_sql(value: ::diesel::mysql::MysqlValue) -> ::diesel::deserialize::Result<Self> {
                use ::core::str::FromStr;
                let s = ::core::str::from_utf8(value.as_bytes())?;
                <$type>::from_str(s).map_err(|_| "Unrecognized enum variant".into())
            }
        }
    };
}
