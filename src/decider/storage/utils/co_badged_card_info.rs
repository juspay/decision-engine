use crate::generics;
use crate::storage::types::CoBadgedCardInfo;

use crate::storage::schema::co_badged_cards_info::dsl;
use diesel::associations::HasTable;
use diesel::*;

use crate::{
    decider::gatewaydecider::types::{ErrorResponse, UnifiedError},
    logger,
};

pub async fn find_co_badged_cards_info_by_card_bin(
    app_state: &crate::app::TenantAppState,
    card_bin: i64,
) -> Result<Vec<CoBadgedCardInfo>, ErrorResponse> {
    let co_badged_cards_info_list = match generics::generic_find_all::<
        <CoBadgedCardInfo as HasTable>::Table,
        _,
        CoBadgedCardInfo,
    >(
        &app_state.db,
        dsl::card_bin_min
            .le(card_bin)
            .and(dsl::card_bin_max.ge(card_bin)),
    )
    .await
    {
        Ok(records) => records,
        Err(err) => {
            logger::error!("parseCoBadgedCardInfo: {:?}", err);
            return Err(ErrorResponse {
                status: "500".to_string(),
                error_code: "INTERNAL_SERVER_ERROR".to_string(),
                error_message: "Internal Server Error".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "INTERNAL_SERVER_ERROR".to_string(),
                    user_message: "Internal Server Error.".to_string(),
                    developer_message: "record parsing failed.".to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            });
        }
    };
    Ok(co_badged_cards_info_list)
}
