
use crate::error::ApiError;
// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal;
// use control::category::compose;
// use data::int::Int64;
// use data::text::Text;
use crate::app::get_tenant_app_state;
use crate::storage::types::MerchantGatewayAccountSubInfo as DBMerchantGatewayAccountSubInfo;
// use types::utils::dbconfig::get_euler_db_conf;
use crate::types::merchant::merchant_gateway_account::{MerchantGwAccId, to_merchant_gw_acc_id};
// use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, lift_either, lift_pure, mandated, parse_field, project};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
// use ghc::generics::Generic;
// use ghc::typelits::KnownSymbol;
// use named::Named;
// use prelude::hiding::id;
// use sequelize::{Clause, Term};
// use test::quickcheck::Arbitrary;
use crate::storage::schema::merchant_gateway_account_sub_info::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MerchantGatewayAccountSubInfo {
    pub id: MgasiPId,
    pub merchantGatewayAccountId: MerchantGwAccId,
    pub subInfoType: SubInfoType,
    pub subIdType: SubIdType,
    pub juspaySubAccountId: String,
    pub gatewaySubAccountId: String,
    pub disabled: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Ord, PartialOrd)]
pub struct MgasiPId {
    pub mgasiPId: i64,
}

pub fn to_mgasi_pid(id: i64) -> MgasiPId {
    MgasiPId { mgasiPId: id }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SubInfoType {
    SPLIT_SETTLEMENT,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SubIdType {
    MARKETPLACE,
    VENDOR,
}

pub fn text_to_sub_info_type(ctx: String) -> Result<SubInfoType, ApiError> {
    match ctx.as_str() {
        "SPLIT_SETTLEMENT" => Ok(SubInfoType::SPLIT_SETTLEMENT),
        _ => Err(ApiError::ParsingError("Invalid Sub Info Type")),
    }
}

pub fn text_to_sub_id_type(ctx: String) -> Result<SubIdType, ApiError> {
    match ctx.as_str() {
        "MARKETPLACE" => Ok(SubIdType::MARKETPLACE),
        "VENDOR" => Ok(SubIdType::VENDOR),
        _ => Err(ApiError::ParsingError("Invalid Sub Id Type")),
    }
}

impl TryFrom<DBMerchantGatewayAccountSubInfo> for MerchantGatewayAccountSubInfo {
    type Error = ApiError;

    fn try_from(db_type: DBMerchantGatewayAccountSubInfo) -> Result<Self, ApiError> {
        Ok(MerchantGatewayAccountSubInfo {
            id: to_mgasi_pid(db_type.id),
            merchantGatewayAccountId: to_merchant_gw_acc_id(db_type.merchant_gateway_account_id),
            subInfoType: text_to_sub_info_type(db_type.sub_info_type)?,
            subIdType: text_to_sub_id_type(db_type.sub_id_type)?,
            juspaySubAccountId: db_type.juspay_sub_account_id,
            gatewaySubAccountId: db_type.gateway_sub_account_id,
            disabled: db_type.disabled,
        })
    }
}

pub async fn find_all_mgasi_by_maga_ids_db(
    
    mga_ids: &[MerchantGwAccId],
) -> Result<Vec<DBMerchantGatewayAccountSubInfo>, crate::generics::MeshError> {
    // Extract IDs from MerchantGwAccId objects
    let mga_id_values: Vec<i64> = mga_ids.iter().map(|id| id.merchantGwAccId).collect();
    let app_state = get_tenant_app_state().await;
    // Use Diesel's query builder to find all matching records
    crate::generics::generic_find_all::<
            <DBMerchantGatewayAccountSubInfo as HasTable>::Table,
            _,
            DBMerchantGatewayAccountSubInfo
        >(
            &app_state.db,
            dsl::merchant_gateway_account_id.eq_any(mga_id_values),
        )
        .await
}

pub async fn find_all_mgasi_by_maga_ids(
    mga_ids: &[MerchantGwAccId],
) -> Vec<MerchantGatewayAccountSubInfo> {
    // Call the database function and handle results
    match find_all_mgasi_by_maga_ids_db(mga_ids).await {
        Ok(db_results) => db_results.into_iter()
                                    .filter_map(|db_record| MerchantGatewayAccountSubInfo::try_from(db_record).ok())
                                    .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD Implement db calls

// pub async fn find_all_mgasi_by_maga_ids_db(
//     mga_ids: &[MerchantGwAccId],
// ) -> Result<Vec<DB::MerchantGatewayAccountSubInfo>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::MerchantGatewayAccountSubInfoT>().await?;
//     let mga_ids = mga_ids.iter().map(|id| id.merchant_gw_acc_id).collect::<Vec<_>>();
//     find_all_rows(db_conf, mesh_config(), vec![Clause::Is(DB::merchant_gateway_account_id(), Term::In(mga_ids))]).await
// }

// pub async fn find_all_mgasi_by_maga_ids(
//     mga_ids: &[MerchantGwAccId],
// ) -> Result<Vec<MerchantGatewayAccountSubInfo>, MeshError> {
//     let res = find_all_mgasi_by_maga_ids_db(mga_ids).await?;
//     to_domain_all(
//         res,
//         parse_merchant_gateway_account_sub_info,
//         "findAllMgasiByMagaIds",
//         "parseMerchantGatewayAccountSubInfo",
//     )
//     .await
// }
