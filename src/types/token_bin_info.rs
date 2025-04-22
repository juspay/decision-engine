use serde::{Deserialize, Serialize};
// use db::eulermeshimpl::meshConfig;
// use db::mesh::internal::*;
// use eulerhs::language::MonadFlow;
// use eulerhs::prelude::*;
// use eulerhs::extra::combinators::toDomainAll;
use crate::app::get_tenant_app_state;
use crate::storage::types::TokenBinInfo as DBTokenBinInfo;
// use types::utils::dbconfig::getEulerDbConf;
// use juspay::extra::parsing::{Parsed, Step, parseField, project};
// use named::*;
// use sequelize::{Clause::Is, Term::In};
use crate::storage::schema::token_bin_info::dsl;
use diesel::associations::HasTable;
use diesel::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenBinInfo {
    #[serde(rename = "tokenBin")]
    pub tokenBin: String,
    #[serde(rename = "cardBin")]
    pub cardBin: String,
    #[serde(rename = "provider")]
    pub provider: String,
}

impl From<DBTokenBinInfo> for TokenBinInfo {
    fn from(dbType: DBTokenBinInfo) -> Self {
        Self {
            tokenBin: dbType.token_bin,
            cardBin: dbType.card_bin,
            provider: dbType.provider,
        }
    }
}

// #TOD implement db calls

// pub async fn getDBAllTokenBinInfoByTokenBins<M: MonadFlow>(
//     tokenBins: Vec<String>,
// ) -> M::Result<Result<Vec<DB::TokenBinInfo>, MeshError>> {
//     let dbConf = getEulerDbConf::<DB::TokenBinInfoT>().await?;
//     findAllRows(dbConf, meshConfig(), vec![Is(DB::tokenBin, In(tokenBins))]).await
// }

// pub async fn getAllTokenBinInfoByTokenBins<M: MonadFlow>(
//     tokenBins: Vec<String>,
// ) -> M::Result<Vec<TokenBinInfo>> {
//     let dbRes = getDBAllTokenBinInfoByTokenBins::<M>(tokenBins).await?;
//     toDomainAll(
//         dbRes,
//         parseTokenBinInfo,
//         named! { function_name: "getAllTokenBinInfoByTokenBins" },
//         named! { parser_name: "parseTokenBinInfo" },
//     )
// }

pub async fn getAllTokenBinInfoByTokenBins(token_bins: Vec<String>) -> Vec<TokenBinInfo> {
    // Perform database query using Diesel's generic_find_all
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_all::<
            <DBTokenBinInfo as HasTable>::Table,
            _,
            DBTokenBinInfo
        >(
            &app_state.db,
            dsl::token_bin.eq_any(token_bins),
        ).await {
            Ok(db_results) => db_results.into_iter()
                                        .filter_map(|db_record| TokenBinInfo::try_from(db_record).ok())
                                        .collect(),
            Err(_) => Vec::new(), // Silently handle any errors by returning an empty vec
        }
}
