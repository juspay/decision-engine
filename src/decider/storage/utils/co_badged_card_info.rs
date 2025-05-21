use crate::storage::types::CoBadgedCardInfo;
use crate::utils::CustomResult;
use crate::{error, generics};
#[cfg(not(feature = "db_migration"))]
use crate::storage::schema::co_badged_cards_info::dsl;
#[cfg(feature = "db_migration")]
use crate::storage::schema_pg::co_badged_cards_info::dsl;
use diesel::associations::HasTable;
use diesel::*;
use error_stack::ResultExt;

use crate::logger;

pub async fn find_co_badged_cards_info_by_card_bin(
    app_state: &crate::app::TenantAppState,
    card_bin: i64,
) -> CustomResult<Vec<CoBadgedCardInfo>, error::StorageError> {
    match generics::generic_find_all::<<CoBadgedCardInfo as HasTable>::Table, _, CoBadgedCardInfo>(
        &app_state.db,
        dsl::card_bin_min
            .le(card_bin)
            .and(dsl::card_bin_max.ge(card_bin)),
    )
    .await
    {
        Ok(records) => Ok(records),
        Err(err) => {
            logger::error!("Co-badged card info fetch error : {:?}", err);
            Err(error::StorageError::NotFoundError)
                .attach_printable("Failed fetch co-badged card info")
        }
    }
}
