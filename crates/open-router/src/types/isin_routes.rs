
// use db::euler_mesh_impl::mesh_config;
// use db::mesh::internal::*;
use serde::{Serialize, Deserialize};
use time::PrimitiveDateTime;
use std::string::String;
use std::vec::Vec;
use std::option::Option;
use std::time::SystemTime;
use crate::error::ApiError;
use crate::app::get_tenant_app_state;
use crate::storage::types::IsinRoutes as DBIsinRoutes;
use crate::types::gateway::{Gateway, text_to_gateway};
use crate::types::merchant::id::{MerchantId, to_merchant_id,merchant_id_to_text};
// use types::utils::db_config::get_euler_db_conf;
// use juspay::extra::parsing::{Parsed, Step, lift_pure, mandated, non_negative, parse_field, project, to_utc};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
// use named::*;
// use optics_core::review;
// use prelude::*;
// use sequelize::{Clause, Term};
// use test::quickcheck::Arbitrary;
use crate::storage::schema::isin_routes::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsinRoutesPId {
    pub isinRoutesPId: i64,
}

pub fn to_isin_routes_pid(id: i64) -> IsinRoutesPId {
    IsinRoutesPId {
        isinRoutesPId: id,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IsinRoutes {
    pub id: IsinRoutesPId,
    pub isin: String,
    pub merchantId: MerchantId,
    pub preferredGateway: Gateway,
    pub preferenceScore: f64,
    pub dateCreated: PrimitiveDateTime,
    pub lastUpdated: PrimitiveDateTime,
}

impl TryFrom<DBIsinRoutes> for IsinRoutes {
    type Error = ApiError;

    fn try_from(db_isin_routes: DBIsinRoutes) -> Result<Self, ApiError> {
        Ok(IsinRoutes {
            id: to_isin_routes_pid(db_isin_routes.id),
            isin: db_isin_routes.isin,
            merchantId: to_merchant_id(db_isin_routes.merchant_id),
            preferredGateway: text_to_gateway(db_isin_routes.preferred_gateway.as_str()).map_err(|_| ApiError::ParsingError("Invalid Gateway"))?,
            preferenceScore: db_isin_routes.preference_score,
            dateCreated: db_isin_routes.date_created,
            lastUpdated: db_isin_routes.last_updated,
        })
    }
}

pub async fn find_all_by_isin_and_merchant_id_db(
    isin_list: Vec<String>,
    mid: &MerchantId,
) -> Result<Vec<DBIsinRoutes>, crate::generics::MeshError> {
    // Convert MerchantId to String for database query
    let merchant_id_str = merchant_id_to_text(mid.clone());
    let app_state = get_tenant_app_state().await;
    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_all::<
            <DBIsinRoutes as HasTable>::Table,
            _,
            DBIsinRoutes
        >(
            &app_state.db,
            dsl::merchant_id.eq(merchant_id_str)
                .and(dsl::isin.eq_any(isin_list)),
        )
        .await
}

pub async fn find_all_by_isin_and_merchant_id(
    
    isin_list: Vec<String>,
    mid: &MerchantId,
) -> Vec<IsinRoutes> {
    // Call the database function and handle results
    match find_all_by_isin_and_merchant_id_db(isin_list, mid).await {
        Ok(db_results) => db_results.into_iter()
                                   .filter_map(|db_record| IsinRoutes::try_from(db_record).ok())
                                   .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD implement db calls

// pub async fn find_all_by_isin_and_merchant_id_db(
//     isin_list: Vec<String>,
//     mid: MerchantId,
// ) -> Result<Vec<DB::IsinRoutes>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::IsinRoutesT>().await?;
//     find_all_rows(
//         db_conf,
//         mesh_config(),
//         vec![Clause::And(vec![
//             Clause::Is(DB::merchantId, Term::Eq(review(merchant_id_text, mid))),
//             Clause::Is(DB::isin, Term::In(isin_list)),
//         ])],
//     )
//     .await
// }

// pub async fn find_all_by_isin_and_merchant_id(
//     isin_list: Vec<String>,
//     mid: MerchantId,
// ) -> Result<Vec<IsinRoutes>, MeshError> {
//     let res = find_all_by_isin_and_merchant_id_db(isin_list, mid).await?;
//     to_domain_all(
//         res,
//         parse_isin_routes,
//         named!("#function_name", "findAllByIsinAndMerchantId"),
//         named!("#parser_name", "parseIsinRoutes"),
//     )
//     .await
// }
