
use serde::{Serialize, Deserialize};
// use db::euler_mesh_impl::mesh_config;
// use db::mesh::internal;
// use control::category::compose as compose; // Equivalent to Haskell's Control.Category ((>>>))
use std::option::Option;
use std::vec::Vec;
use std::string::String;
use crate::app::get_tenant_app_state;
use crate::storage::types::MerchantGatewayCardInfo as DBMerchantGatewayCardInfo;
// use types::utils::dbconfig::get_euler_db_conf;
use crate::types::gateway_card_info::{GciPId, to_gci_pid};
use crate::types::merchant::id::{MerchantPId, to_merchant_pid};
use crate::types::merchant::merchant_gateway_account::{MerchantGwAccId, to_merchant_gw_acc_id};
use crate::types::money::internal::{Money};
// use juspay::extra::parsing::{Parsed, Step, around, lift_pure, mandated, non_negative, parse_field, project};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
// use ghc::generics::Generic;
// use ghc::typelits::KnownSymbol;
// use named::named_macro as named; // Equivalent to Haskell's Named (!)
// use prelude::*;
// use sequelize::{Clause, Term};
// use test::quickcheck::Arbitrary;
use crate::storage::schema::merchant_gateway_card_info::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MgciPId {
    pub mgciPId: i64,
}

pub fn to_mgci_pid(id: i64) -> MgciPId {
    MgciPId {
        mgciPId: id,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantGatewayCardInfo {
    pub id: MgciPId,
    pub disabled: bool,
    pub gatewayCardInfoId: GciPId,
    pub merchantAccountId: MerchantPId,
    pub emandateRegisterMaxAmount: Option<Money>,
    pub merchantGatewayAccountId: Option<MerchantGwAccId>,
}

impl From<DBMerchantGatewayCardInfo> for MerchantGatewayCardInfo {
    fn from(db_type: DBMerchantGatewayCardInfo) -> Self {
        MerchantGatewayCardInfo {
            id: to_mgci_pid(db_type.id),
            disabled: db_type.disabled,
            gatewayCardInfoId: to_gci_pid(db_type.gateway_card_info_id),
            merchantAccountId: to_merchant_pid(db_type.merchant_account_id),
            emandateRegisterMaxAmount: db_type.emandate_register_max_amount.map(|x| Money::from_double(x)),
            merchantGatewayAccountId: db_type.merchant_gateway_account_id.map(|x| to_merchant_gw_acc_id(x)),
        }
    }
}

pub async fn find_all_mgcis_by_macc_and_gci_p_id_db(
    
    m_pid: &MerchantPId,
    gci_ids: &[GciPId],
) -> Result<Vec<DBMerchantGatewayCardInfo>, crate::generics::MeshError> {
    // Extract IDs from GciPId objects
    let gci_id_values: Vec<i64> = gci_ids.iter().map(|gci| gci.gciPId).collect();
    let app_state = get_tenant_app_state().await;
    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_all::<
            <DBMerchantGatewayCardInfo as HasTable>::Table,
            _,
            DBMerchantGatewayCardInfo
        >(
            &app_state.db,
            dsl::gateway_card_info_id.eq_any(gci_id_values)
                .and(dsl::merchant_account_id.eq(m_pid.merchantPId))
                .and(dsl::disabled.eq(false)),
        ).await
}

pub async fn find_all_mgcis_by_macc_and_gci_p_id(
    
    m_pid: MerchantPId,
    gci_ids: Vec<GciPId>,
) -> Vec<MerchantGatewayCardInfo> {
    // Call the database function and handle results
    match find_all_mgcis_by_macc_and_gci_p_id_db(&m_pid, &gci_ids).await {
        Ok(db_results) => db_results.into_iter()
                                   .filter_map(|db_record| MerchantGatewayCardInfo::try_from(db_record).ok())
                                   .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD implement db calls

// pub async fn find_all_mgcis_by_macc_and_gci_p_id_db(
//     m_pid: MerchantPId,
//     gci_ids: Vec<GciPId>,
// ) -> Result<Vec<DB::MerchantGatewayCardInfo>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::MerchantGatewayCardInfoT>().await?;
//     let gci_ids: Vec<i64> = gci_ids.iter().map(|gci| gci.unGciPId).collect();
//     find_all_rows(
//         db_conf,
//         mesh_config,
//         vec![Clause::And(vec![
//             Clause::Is(DB::gatewayCardInfoId, Term::In(gci_ids)),
//             Clause::Is(DB::merchantAccountId, Term::Eq(m_pid.unMerchantPId)),
//             Clause::Is(DB::disabled, Term::Eq(false)),
//         ])],
//     )
//     .await
// }

// pub async fn find_all_mgcis_by_macc_and_gci_p_id(
//     m_pid: MerchantPId,
//     gci_ids: Vec<GciPId>,
// ) -> Vec<MerchantGatewayCardInfo> {
//     let res = find_all_mgcis_by_macc_and_gci_p_id_db(m_pid, gci_ids).await?;
//     to_domain_all(
//         res,
//         parse_merchant_gateway_card_info,
//         named!(function_name = "findAllByJuspayBankCodeById"),
//         named!(parser_name = "parseMerchantGatewayCardInfo"),
//     )
// }
