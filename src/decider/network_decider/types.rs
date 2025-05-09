use std::collections::HashMap;

use crate::decider::gatewaydecider;
use crate::error;
use diesel::sql_types;
use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};

use crate::storage::types as storage_types;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoBadgedCardRequest {
    pub merchant_category_code: MerchantCategoryCode,
    pub acquirer_country: CountryAlpha2,
    pub co_badged_card_data: Option<DebitRoutingRequestData>,
}

impl TryInto<CoBadgedCardRequest> for serde_json::Value {
    type Error = Report<error::ApiError>;

    fn try_into(self) -> Result<CoBadgedCardRequest, Self::Error> {
        serde_json::from_value(self).change_context(error::ApiError::ParsingError(
            "Failed to parse metadata to CoBadgedCardRequest",
        ))
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, strum::EnumString, strum::Display, Hash, PartialEq, Eq,
)]
#[serde(rename_all = "snake_case")]
pub enum MerchantCategoryCode {
    #[serde(rename = "merchant_category_code_0001")]
    Mcc0001,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebitRoutingRequestData {
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
    strum::Display,
    strum::EnumString,
    diesel::AsExpression,
    diesel::FromSqlRow,
)]
#[diesel(sql_type = sql_types::Text)]
pub enum RegulatedName {
    #[serde(rename = "GOVERNMENT NON-EXEMPT INTERCHANGE FEE (WITH FRAUD)")]
    #[strum(serialize = "GOVERNMENT NON-EXEMPT INTERCHANGE FEE (WITH FRAUD)")]
    NonExemptWithFraud,

    #[serde(untagged)]
    #[strum(default)]
    Unknown(String),
}

#[derive(Debug, Clone)]
pub struct CoBadgedCardInfoDomainData {
    pub id: String,
    pub card_bin_min: i64,
    pub card_bin_max: i64,
    pub issuing_bank_name: Option<String>,
    pub card_network: gatewaydecider::types::NETWORK,
    pub country_code: Option<CountryAlpha2>,
    pub card_type: Option<CardType>,
    pub regulated: Option<bool>,
    pub regulated_name: Option<RegulatedName>,
    pub prepaid: Option<bool>,
    pub reloadable: Option<bool>,
    pub pan_or_token: PanOrToken,
    pub card_bin_length: i16,
    pub bin_provider_bin_length: i16,
    pub card_brand_is_additional: bool,
    pub domestic_only: Option<bool>,
    pub created_at: time::PrimitiveDateTime,
    pub modified_at: time::PrimitiveDateTime,
    pub last_updated_provider: Option<String>,
}

impl TryFrom<storage_types::CoBadgedCardInfo> for CoBadgedCardInfoDomainData {
    type Error = String;

    fn try_from(
        db_co_badged_cards_info_record: storage_types::CoBadgedCardInfo,
    ) -> Result<Self, Self::Error> {
        let parsed_network = db_co_badged_cards_info_record
            .card_network
            .parse::<gatewaydecider::types::NETWORK>()
            .map_err(|error| {
                format!(
                    "Failed to parse network for card id {}: {}",
                    db_co_badged_cards_info_record.id, error
                )
            })?;

        Ok(Self {
            id: db_co_badged_cards_info_record.id,
            card_bin_min: db_co_badged_cards_info_record.card_bin_min,
            card_bin_max: db_co_badged_cards_info_record.card_bin_max,
            issuing_bank_name: db_co_badged_cards_info_record.issuing_bank_name,
            card_network: parsed_network,
            country_code: db_co_badged_cards_info_record.country_code,
            card_type: db_co_badged_cards_info_record.card_type,
            regulated: db_co_badged_cards_info_record.regulated,
            regulated_name: db_co_badged_cards_info_record.regulated_name,
            prepaid: db_co_badged_cards_info_record.prepaid,
            reloadable: db_co_badged_cards_info_record.reloadable,
            pan_or_token: db_co_badged_cards_info_record.pan_or_token,
            card_bin_length: db_co_badged_cards_info_record.card_bin_length,
            bin_provider_bin_length: db_co_badged_cards_info_record.bin_provider_bin_length,
            card_brand_is_additional: db_co_badged_cards_info_record.card_brand_is_additional,
            domestic_only: db_co_badged_cards_info_record.domestic_only,
            created_at: db_co_badged_cards_info_record.created_at,
            modified_at: db_co_badged_cards_info_record.modified_at,
            last_updated_provider: db_co_badged_cards_info_record.last_updated_provider,
        })
    }
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
    #[strum(serialize = "credit")]
    Credit,
    #[strum(serialize = "debit")]
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
    #[strum(serialize = "pan")]
    Pan,
    #[strum(serialize = "token")]
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

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct DebitRoutingConfig {
    pub network_fee: HashMap<gatewaydecider::types::NETWORK, NetworkProcessingData>,
    pub interchange_fee: NetworkInterchangeFee,
    pub fraud_check_fee: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NetworkInterchangeFee {
    pub non_regulated: NoneRegulatedNetworkProcessingData,
    pub regulated: NetworkProcessingData,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NoneRegulatedNetworkProcessingData(
    pub  HashMap<
        MerchantCategoryCode,
        HashMap<gatewaydecider::types::NETWORK, NetworkProcessingData>,
    >,
);

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NetworkProcessingData {
    pub percentage: f64,
    pub fixed_amount: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Platform {
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct CoBadgedCardInfoResponse {
    pub co_badged_card_networks: Vec<gatewaydecider::types::NETWORK>,
    pub issuer_country: CountryAlpha2,
    pub is_regulated: bool,
    pub regulated_name: Option<RegulatedName>,
    pub card_type: CardType,
}

impl From<DebitRoutingRequestData> for CoBadgedCardInfoResponse {
    fn from(co_badged_card_data: DebitRoutingRequestData) -> Self {
        CoBadgedCardInfoResponse {
            co_badged_card_networks: co_badged_card_data.co_badged_card_networks,
            issuer_country: co_badged_card_data.issuer_country,
            is_regulated: co_badged_card_data.is_regulated,
            regulated_name: co_badged_card_data.regulated_name,
            card_type: co_badged_card_data.card_type,
        }
    }
}
