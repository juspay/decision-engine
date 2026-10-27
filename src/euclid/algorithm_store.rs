//! Storage for routing algorithms and the active-slot mapper, scoped to the merchant that owns them.
//!
//! Every query is filtered by the [`AuthorizedMerchant`]'s id (`created_by`), so another merchant's
//! algorithm or mapping is simply not found: ownership is part of the query rather than a comparison
//! each caller has to remember.

#[cfg(feature = "mysql")]
use crate::storage::schema::{routing_algorithm::dsl, routing_algorithm_mapper::dsl as mapper_dsl};
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::{
    routing_algorithm::dsl, routing_algorithm_mapper::dsl as mapper_dsl,
};

use diesel::{associations::HasTable, BoolExpressionMethods, ExpressionMethods};
use error_stack::Report;
use time::PrimitiveDateTime;

use crate::auth::AuthorizedMerchant;
use crate::euclid::types::{
    RoutingAlgorithm, RoutingAlgorithmMapper, RoutingAlgorithmMapperNew,
    RoutingAlgorithmMapperUpdate,
};
use crate::generics::{self, MeshError};
use crate::storage::Storage;

type StoreResult<T> = Result<T, Report<MeshError>>;

/// The merchant's algorithm with this id.
pub async fn find(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    id: &str,
) -> StoreResult<RoutingAlgorithm> {
    generics::generic_find_one::<<RoutingAlgorithm as HasTable>::Table, _, RoutingAlgorithm>(
        db,
        dsl::id
            .eq(id.to_string())
            .and(dsl::created_by.eq(merchant.as_str().to_string())),
    )
    .await
    .map_err(Report::new)
}

/// Every algorithm the merchant has created.
pub async fn list(
    db: &Storage,
    merchant: &AuthorizedMerchant,
) -> StoreResult<Vec<RoutingAlgorithm>> {
    generics::generic_find_all::<<RoutingAlgorithm as HasTable>::Table, _, RoutingAlgorithm>(
        db,
        dsl::created_by.eq(merchant.as_str().to_string()),
    )
    .await
    .map_err(Report::new)
}

/// The merchant's algorithms that occupy an active slot.
pub async fn list_active(
    db: &Storage,
    merchant: &AuthorizedMerchant,
) -> StoreResult<Vec<RoutingAlgorithm>> {
    let ids: Vec<String> = list_activations(db, merchant)
        .await?
        .into_iter()
        .map(|mapping| mapping.routing_algorithm_id)
        .collect();
    generics::generic_find_all::<<RoutingAlgorithm as HasTable>::Table, _, RoutingAlgorithm>(
        db,
        dsl::id
            .eq_any(ids)
            .and(dsl::created_by.eq(merchant.as_str().to_string())),
    )
    .await
    .map_err(Report::new)
}

/// Store a new algorithm as the merchant's: `created_by` is set from `merchant`, whatever the row
/// carried.
pub async fn insert(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    mut algorithm: RoutingAlgorithm,
) -> StoreResult<()> {
    algorithm.created_by = merchant.as_str().to_string();
    generics::generic_insert(db, algorithm).await.map(|_| ())
}

/// Replace the editable fields of the merchant's algorithm.
pub async fn update_definition(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    id: &str,
    name: &str,
    description: &str,
    algorithm_data: String,
    modified_at: PrimitiveDateTime,
) -> StoreResult<()> {
    let conn = db.get_conn().await.map_err(Report::new)?;
    generics::generic_update::<<RoutingAlgorithm as HasTable>::Table, _, _>(
        &conn,
        dsl::id
            .eq(id.to_string())
            .and(dsl::created_by.eq(merchant.as_str().to_string())),
        (
            dsl::name.eq(name.to_string()),
            dsl::description.eq(description.to_string()),
            dsl::algorithm_data.eq(algorithm_data),
            dsl::modified_at.eq(modified_at),
        ),
    )
    .await
    .map(|_| ())
}

/// Delete the merchant's algorithm. Deleting one the merchant does not own deletes nothing.
pub async fn delete(db: &Storage, merchant: &AuthorizedMerchant, id: &str) -> StoreResult<()> {
    let conn = db.get_conn().await.map_err(Report::new)?;
    generics::generic_delete::<<RoutingAlgorithm as HasTable>::Table, _>(
        &conn,
        dsl::id
            .eq(id.to_string())
            .and(dsl::created_by.eq(merchant.as_str().to_string())),
    )
    .await
    .map(|_| ())
    .map_err(Report::new)
}

/// Every active slot the merchant has.
pub async fn list_activations(
    db: &Storage,
    merchant: &AuthorizedMerchant,
) -> StoreResult<Vec<RoutingAlgorithmMapper>> {
    generics::generic_find_all::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(db, mapper_dsl::created_by.eq(merchant.as_str().to_string()))
    .await
    .map_err(Report::new)
}

/// The mapping occupying one of the merchant's slots, if any.
pub async fn find_slot(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    slot: &str,
) -> StoreResult<Option<RoutingAlgorithmMapper>> {
    generics::generic_find_one_optional::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        db,
        mapper_dsl::created_by
            .eq(merchant.as_str().to_string())
            .and(mapper_dsl::algorithm_for.eq(slot.to_string())),
    )
    .await
    .map_err(Report::new)
}

/// The mapping that makes this algorithm active for the merchant, if it is active.
pub async fn find_activation(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    algorithm_id: &str,
) -> StoreResult<Option<RoutingAlgorithmMapper>> {
    generics::generic_find_one_optional::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        db,
        mapper_dsl::created_by
            .eq(merchant.as_str().to_string())
            .and(mapper_dsl::routing_algorithm_id.eq(algorithm_id.to_string())),
    )
    .await
    .map_err(Report::new)
}

/// Point one of the merchant's existing slots at another of its algorithms.
pub async fn repoint_slot(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    slot: &str,
    algorithm: &RoutingAlgorithm,
) -> StoreResult<()> {
    let conn = db.get_conn().await.map_err(Report::new)?;
    generics::generic_update_if_present::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        RoutingAlgorithmMapperUpdate,
        _,
    >(
        &conn,
        mapper_dsl::created_by
            .eq(merchant.as_str().to_string())
            .and(mapper_dsl::algorithm_for.eq(slot.to_string())),
        RoutingAlgorithmMapperUpdate {
            routing_algorithm_id: algorithm.id.clone(),
            algorithm_for: slot.to_string(),
        },
    )
    .await
    .map(|_| ())
}

/// Occupy an empty slot of the merchant's with one of its algorithms.
pub async fn occupy_slot(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    slot: &str,
    algorithm: &RoutingAlgorithm,
) -> StoreResult<()> {
    generics::generic_insert(
        db,
        RoutingAlgorithmMapperNew::new(
            merchant.as_str().to_string(),
            algorithm.id.clone(),
            slot.to_string(),
        ),
    )
    .await
    .map(|_| ())
}

/// Empty the slot this mapping occupies.
pub async fn vacate(
    db: &Storage,
    merchant: &AuthorizedMerchant,
    mapping: &RoutingAlgorithmMapper,
) -> StoreResult<()> {
    let conn = db.get_conn().await.map_err(Report::new)?;
    generics::generic_delete::<<RoutingAlgorithmMapper as HasTable>::Table, _>(
        &conn,
        mapper_dsl::id
            .eq(mapping.id)
            .and(mapper_dsl::created_by.eq(merchant.as_str().to_string())),
    )
    .await
    .map(|_| ())
    .map_err(Report::new)
}
