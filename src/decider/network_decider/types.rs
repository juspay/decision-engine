use std::collections::HashMap;

use crate::decider::gatewaydecider;
use crate::error;
use crate::utils::CustomResult;
use diesel::sql_types;
use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, strum::EnumString, strum::Display)]
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

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct DebitRoutingConfig {
    pub network_fee: HashMap<gatewaydecider::types::NETWORK, NetworkProcessingData>,
    pub interchange_fee: NetworkInterchangeFee,
    pub fraud_check_fee: f64,
}

impl DebitRoutingConfig {
    pub fn get_non_regulated_interchange_fee(
        &self,
        merchant_category_code: &str,
        network: &gatewaydecider::types::NETWORK,
    ) -> CustomResult<&NetworkProcessingData, error::ApiError> {
        self.interchange_fee
            .non_regulated
            .0
            .get(merchant_category_code)
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for merchant category code",
            ))?
            .get(network)
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for non regulated",
            ))
            .attach_printable(
                "Failed to fetch interchange fee for non regulated banks in debit routing",
            )
    }

    pub fn get_network_fee(
        &self,
        network: &gatewaydecider::types::NETWORK,
    ) -> CustomResult<&NetworkProcessingData, error::ApiError> {
        Ok(self.network_fee
            .get(network)
            .ok_or(error::ApiError::MissingRequiredField(
                "interchange fee for non regulated",
            ))
            .attach_printable(
                "Failed to fetch interchange fee for non regulated banks in debit routing",
            )?)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NetworkInterchangeFee {
    pub non_regulated: NoneRegulatedNetworkProcessingData,
    pub regulated: NetworkProcessingData,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NoneRegulatedNetworkProcessingData(
    pub HashMap<String, HashMap<gatewaydecider::types::NETWORK, NetworkProcessingData>>,
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
