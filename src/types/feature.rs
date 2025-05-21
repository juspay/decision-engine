use crate::app::get_tenant_app_state;
// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal::*;
use crate::storage::types::{BitBool, Feature as DBFeature};
// use types::utils::dbconfig::get_euler_db_conf;
use crate::types::merchant::id::{merchant_id_to_text, to_merchant_id, MerchantId};
// use juspay::extra::parsing::{Parsed, Step, around, lift_pure, mandated, parse_field, project};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;

use diesel::*;
use serde::{Deserialize, Serialize};
use std::clone::Clone;
use std::convert::From;
use std::fmt::Debug;
use std::option::Option;
use std::result::Result;
use std::string::String;
// use ghc_stack::HasCallStack;
// use ghc_typelits::KnownSymbol;
// use named::Named;
// use optics_core::review;
// use test::quickcheck::{Arbitrary, arbitrary};
// use test::quickcheck_arbitrary_generic::generic_arbitrary;
// use test::quickcheck_instances_text::*;
// use test::quickcheck_instances_time::*;
// use data_int::Int64;
// use data_text::Text;
#[cfg(not(feature = "db_migration"))]
use crate::storage::schema::feature::dsl;
#[cfg(feature = "db_migration")]
use crate::storage::schema_pg::feature::dsl;
use diesel::associations::HasTable;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FeaturePId {
    #[cfg(not(feature = "db_migration"))]
    pub featurePId: i64,
    #[cfg(feature = "db_migration")]
    pub featurePId: i32,
}

#[cfg(not(feature = "db_migration"))]
pub fn to_feature_pid(other_id: i64) -> FeaturePId {
    FeaturePId {
        featurePId: other_id,
    }
}

#[cfg(feature = "db_migration")]
pub fn to_feature_pid(other_id: i32) -> FeaturePId {
    FeaturePId {
        featurePId: other_id,
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub id: FeaturePId,
    pub enabled: bool,
    pub name: String,
    pub merchantId: Option<MerchantId>,
}

impl From<DBFeature> for Feature {
    fn from(value: DBFeature) -> Self {
        Self {
            id: to_feature_pid(value.id),
            enabled: value.enabled.0,
            name: value.name,
            merchantId: value.merchant_id.map(to_merchant_id),
        }
    }
}

// Generic Diesel Implementation
pub async fn get_feature_by_name(feature_name: &str) -> Option<Feature> {
    // Try to find the feature using diesel
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_one_optional::<
        <DBFeature as HasTable>::Table,
        _,
        DBFeature
    >(
        &app_state.db,
        dsl::name.eq(feature_name.to_owned()),
    )
    .await
    {
        Ok(Some(db_feature)) => Some(db_feature.into()),
        Ok(None) => None,
        Err(_) => None, // Silently handle any errors by returning None
    }
}

pub async fn get_db_feature_enabled(
    feature_name: &str,
    mid: &MerchantId,
    enabled: bool,
) -> Result<Option<DBFeature>, crate::generics::MeshError> {
    // Convert MerchantId to String for database query
    let merchant_id_str = merchant_id_to_text(mid.clone());
    let app_state = get_tenant_app_state().await;
    // Use generic_find_one_optional for diesel query
    crate::generics::generic_find_one_optional::<<DBFeature as HasTable>::Table, _, DBFeature>(
        &app_state.db,
        dsl::name
            .eq(feature_name.to_owned())
            .and(dsl::merchant_id.eq(merchant_id_str))
            .and(dsl::enabled.eq(BitBool(enabled))),
    )
    .await

}

pub async fn get_feature_enabled(
    feature_name: &str,
    mid: &MerchantId,
    enabled: bool,
) -> Option<Feature> {
    match get_db_feature_enabled(feature_name, mid, enabled).await {
        Ok(Some(db_feature)) => Some(db_feature.into()),
        Ok(None) => None,
        Err(_) => None, // Silently handle any errors by returning None
    }
}

// #TOD implement db calls --done

// pub async fn get_feature_by_name(
//     f_name: String,
// ) -> Option<Feature> {
//     let db_res = get_db_feature_by_name(f_name).await;
//     to_domain_all(
//         db_res,
//         parse_feature,
//         Named::new("function_name", "getFeatureByName"),
//         Named::new("parser_name", "parseFeature"),
//     )
//     .await
// }

// pub async fn get_db_feature_enabled(
//     feature_name: String,
//     mid: MerchantId,
//     enabled: bool,
// ) -> Result<Option<DB::Feature>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::FeatureT>().await;
//     find_one_row(
//         db_conf,
//         mesh_config,
//         vec![And(vec![
//             Is(DB::name, Eq(feature_name)),
//             Is(DB::merchantId, Eq(Some(review(merchantIdText, mid)))),
//             Is(DB::enabled, Eq(enabled)),
//         ])],
//     )
//     .await
// }

// pub async fn get_feature_enabled(
//     feature_name: String,
//     mid: MerchantId,
//     enabled: bool,
// ) -> Option<Feature> {
//     let res = get_db_feature_enabled(feature_name, mid, enabled).await;
//     to_domain_all(
//         res,
//         parse_feature,
//         Named::new("function_name", "getFeature"),
//         Named::new("parser_name", "parseFeature"),
//     )
//     .await
// }
