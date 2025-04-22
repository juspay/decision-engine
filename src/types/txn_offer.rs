use serde::{Deserialize, Serialize};
// use db::eulermeshimpl::meshConfig;
// use db::mesh::internal::*;
use crate::app::get_tenant_app_state;
use crate::storage::types::TxnOffer as DBTxnOffer;
use std::convert::From;
use std::result::Result;
use std::string::String;
use std::vec::Vec;
// use types::utils::dbconfig::getEulerDbConf;
use crate::types::money::internal::Money;
use crate::types::offer::{to_offer_id, OfferId};
// use juspay::extra::parsing::{Parsed, Step, liftPure, mandated, nonNegative, parseField, project};
use crate::types::txn_details::types::{to_txn_detail_id, TxnDetailId};
// use eulerhs::extra::combinators::toDomainAll;
// use eulerhs::language::MonadFlow;
// use ghc::stack::HasCallStack;
// use ghc::typelits::KnownSymbol;
// use named::*;
// use sequelize::{Clause, Term};
// use test::quickcheck::Arbitrary;
use crate::storage::schema::txn_offer::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TxnOfferPId {
    pub txnOfferPId: i64,
}

pub fn to_txn_offer_pid(id: i64) -> TxnOfferPId {
    TxnOfferPId { txnOfferPId: id }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnOffer {
    pub id: TxnOfferPId,
    pub version: i64,
    pub discountAmount: Money,
    pub offerId: OfferId,
    pub signature: String,
    pub txnDetailId: TxnDetailId,
}

impl From<DBTxnOffer> for TxnOffer {
    fn from(db: DBTxnOffer) -> Self {
        Self {
            id: to_txn_offer_pid(db.id),
            version: db.version,
            discountAmount: Money::from_whole(db.discount_amount),
            offerId: to_offer_id(db.offer_id),
            signature: db.signature,
            txnDetailId: to_txn_detail_id(db.txn_detail_id),
        }
    }
}

pub async fn getOffersDB(
    txn_id: &TxnDetailId,
) -> Result<Vec<DBTxnOffer>, crate::generics::MeshError> {
    // Convert TxnDetailId to the appropriate format for database query if needed
    let txn_id_value = txn_id.0;
    let app_state = get_tenant_app_state().await;
    // Use Diesel's query builder to find all offers for the transaction
    crate::generics::generic_find_all::<<DBTxnOffer as HasTable>::Table, _, DBTxnOffer>(
        &app_state.db,
        dsl::txn_detail_id.eq(txn_id_value),
    )
    .await
}

pub async fn getOffers(txn_id: &TxnDetailId) -> Vec<TxnOffer> {
    // Call the database function and handle results
    match getOffersDB(txn_id).await {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record| TxnOffer::try_from(db_record).ok())
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

// #TOD Implement DB Calls

// pub async fn getOffersDB(
//     txn_id: TxnDetailId,
// ) -> Result<Vec<DB::TxnOffer>, MeshError> {
//     let db_conf = getEulerDbConf::<DB::TxnOfferT>().await?;
//     findAllRows(db_conf, meshConfig, vec![Clause::Is(DB::txnDetailId, Term::Eq(txn_id.into()))]).await
// }

// pub async fn getOffers(
//     txn_id: TxnDetailId,
// ) -> Result<Vec<TxnOffer>, MeshError> {
//     let res = getOffersDB(txn_id).await?;
//     toDomainAll(
//         res,
//         parseTxnOffer,
//         named_args! {
//             function_name: "getOffers",
//             parser_name: "parseTxnOffer",
//         },
//     )
// }
