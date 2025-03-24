
use diesel::associations::HasTable;
use serde::{Deserialize, Serialize};
use crate::app;
use crate::error::ApiError;
// use db::eulermeshimpl::meshConfig;
// use db::mesh::internal;
// use eulerhs::prelude::*;
use crate::app::get_tenant_app_state;
use crate::storage::types::CardInfo as DBCardInfo;
use crate::types::card::card_type::{CardType, to_card_type};
use crate::types::card::isin::{Isin, to_isin};
// use types::utils::dbconfig::getEulerDbConf;
// use juspay::extra::parsing::{Parsed, Step, around, parseField, project, liftPure};
// use juspay::extra::text::emptyTextAsNothing;
// use juspay::extra::secret::SecretContext;
// use eulerhs::extra::combinators::toDomainAll;
// use eulerhs::language::MonadFlow;
// use control::category;
// use data::reflection::Given;
// use ghc::typelits::KnownSymbol;
// use named::*;
// use optics::core::review;
// use sequelize::{Clause, Term};
use crate::storage::schema::card_info::dsl;
use diesel::*;


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct CardInfo {
    // #[serde(rename = "cardIsin")]
    pub cardIsin: Isin,
    // #[serde(rename = "cardSwitchProvider")]
    pub cardSwitchProvider: String,
    // #[serde(rename = "cardType")]
    pub cardType: Option<CardType>,
    // #[serde(rename = "cardSubType")]
    pub cardSubType: Option<String>,
    // #[serde(rename = "cardSubTypeCategory")]
    pub cardSubTypeCategory: Option<String>,
    // #[serde(rename = "cardIssuerCountry")]
    pub cardIssuerCountry: Option<String>,
    // #[serde(rename = "countryCode")]
    pub countryCode: Option<String>,
    // #[serde(rename = "extendedCardType")]
    pub extendedCardType: Option<String>,
}

impl TryFrom<DBCardInfo> for CardInfo {
    type Error = ApiError;

    fn try_from(value: DBCardInfo) -> Result<Self, ApiError> {
        Ok(CardInfo {
            cardIsin: to_isin(value.card_isin)?,
            cardSwitchProvider: value.card_switch_provider,
            cardType: value.card_type.map(|card_type| to_card_type(card_type.as_str())).transpose()?,
            cardSubType: value.card_sub_type,
            cardSubTypeCategory: value.card_sub_type_category,
            cardIssuerCountry: value.card_issuer_country,
            countryCode: value.country_code,
            extendedCardType: value.extended_card_type,
        })
    }
}

// #TOD implement db calls

// pub async fn getDBCardInfoByIsin(
//     isin: Isin,
// ) -> Result<Option<DB::CardInfo>, MeshError> {
//     let dbConf = getEulerDbConf::<DB::CardInfoT>().await?;
//     findOneRow(
//         dbConf,
//         meshConfig(),
//         vec![Clause::Is(DB::cardIsin, Term::Eq(review(isinText, isin)))],
//     )
//     .await
// }


// pub async fn get_feature_by_name(
//     app_state: &crate::app::TenantAppState,
//     feature_name: &str,
// ) -> Option<Feature> {
//     // Try to find the feature using diesel
//     match crate::generics::generic_find_one_optional::<
//         <DBFeature as HasTable>::Table,
//         _,
//         DBFeature
//     >(
//         &app_state.db,
//         dsl::name.eq(feature_name.to_owned()),
//     )
//     .await
//     {
//         Ok(Some(db_feature)) => Some(db_feature.into()),
//         Ok(None) => None,
//         Err(_) => None, // Silently handle any errors by returning None
//     }
// }

// implement get_card_info__by_isin

pub async fn getCardInfoByIsin (
    isin: Isin,
) -> Option<CardInfo> {
    // Try to find the card info by isin using diesel
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_one_optional::<
        <DBCardInfo as HasTable>::Table,
        _,
        DBCardInfo
    >(
        &app_state.db,
        dsl::card_isin.eq(isin.to_text()),
    )
    .await
    {
        Ok(Some(db_card_info)) => match CardInfo::try_from(db_card_info) {
            Ok(card_info) => Some(card_info),
            Err(_) => None, // Silently handle any errors by returning None
        },
        Ok(None) => None,
        Err(_) => None, // Silently handle any errors by returning None
    }
}

pub async fn getAllCardInfoByIsins (
    isin_list: Vec<Isin>,
) -> Vec<CardInfo> {
    // Try to find the card info by isin using diesel
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<
            <DBCardInfo as HasTable>::Table,
            _,
            DBCardInfo
        >(
            &app_state.db,
            dsl::card_isin.eq_any(isin_list.iter().map(|isin| isin.to_text()).collect::<Vec<String>>()),
        )
        .await
        {
            Ok(db_card_info_list) => db_card_info_list.into_iter().map(|db_card_info| CardInfo::try_from(db_card_info)).filter_map(Result::ok).collect(),
            Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
        }
}


// --done

// pub async fn getCardInfoByIsin(
//     isin: Isin,
// ) -> Option<CardInfo> {
//     let dbRes = getDBCardInfoByIsin(isin).await;
//     toDomainAll(
//         dbRes,
//         parseCardInfo,
//         named::function_name("getCardInfoByIsin"),
//         named::parser_name("parseCardInfo"),
//     )
//     .await
// }

// pub async fn getDBAllCardInfoByIsins(
//     isinList: Vec<Isin>,
// ) -> Result<Vec<DB::CardInfo>, MeshError> {
//     let dbConf = getEulerDbConf::<DB::CardInfoT>().await?;
//     findAllRows(
//         dbConf,
//         meshConfig(),
//         vec![Clause::Is(
//             DB::cardIsin,
//             Term::In(isinList.iter().map(|isin| review(isinText, isin)).collect()),
//         )],
//     )
//     .await
// }

// pub async fn getAllCardInfoByIsins(
//     isinList: Vec<Isin>,
// ) -> Vec<CardInfo> {
//     let dbRes = getDBAllCardInfoByIsins(isinList).await;
//     toDomainAll(
//         dbRes,
//         parseCardInfo,
//         named::function_name("getAllCardInfoByIsins"),
//         named::parser_name("parseCardInfo"),
//     )
//     .await
// }
