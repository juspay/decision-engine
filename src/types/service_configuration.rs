use crate::app::get_tenant_app_state;
use crate::redis::cache::{evict_service_config, write_through_service_config};
#[cfg(feature = "mysql")]
use crate::storage::schema::service_configuration::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::service_configuration::dsl;
use diesel::associations::HasTable;
use diesel::*;
use std::option::Option;
use std::string::String;
// use sequelize::{Clause::{Is, And}, Term::{Eq, In}};
use crate::storage::types::{
    ServiceConfiguration, ServiceConfigurationNew, ServiceConfigurationUpdate,
};

pub async fn find_config_by_name(
    name: String,
) -> Result<Option<ServiceConfiguration>, crate::generics::MeshError> {
    // Extract IDs from GciPId objects
    let app_state = get_tenant_app_state().await;
    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_find_one_optional::<
        <ServiceConfiguration as HasTable>::Table,
        _,
        ServiceConfiguration,
    >(&app_state.db, dsl::name.eq(name))
    .await
}

pub async fn insert_config(
    name: String,
    value: Option<String>,
) -> error_stack::Result<(), crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;

    let config = ServiceConfigurationNew {
        name: name.clone(),
        value: value.clone(),
        new_value: None,
        previous_value: None,
        new_value_status: None,
    };

    crate::generics::generic_insert(&app_state.db, config).await?;

    match value {
        Some(v) => write_through_service_config(name, &v).await,
        None => evict_service_config(name).await,
    }
    Ok(())
}

pub async fn update_config(
    name: String,
    value: Option<String>,
) -> error_stack::Result<(), crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;
    let values = ServiceConfigurationUpdate {
        value: value.clone(),
    };
    let conn = &app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;
    crate::generics::generic_update::<
        <ServiceConfiguration as HasTable>::Table,
        ServiceConfigurationUpdate,
        _,
    >(conn, dsl::name.eq(name.clone()), values)
    .await?;

    match value {
        Some(v) => write_through_service_config(name, &v).await,
        None => evict_service_config(name).await,
    }
    Ok(())
}

/// Replace this config's value, but only if the row still holds `expected`.
///
/// Returns `Ok(false)` when it does not — somebody wrote between the caller's read and this write,
/// so the caller must re-read and re-apply rather than clobber them. `Ok(true)` means the swap
/// landed and the cache has been written through.
///
/// This exists because several values in this table are *containers* that callers read-modify-
/// write: a feature flag's list of enabled merchants, an index of cost-ingestion sources. A plain
/// [`update_config`] makes the last writer win the whole container, silently discarding the entries
/// the other writer had just added. Comparing against the value that was read turns that lost
/// update into a retry.
///
/// `expected` of `None` means "the row exists and its value is NULL", which is a different
/// condition from the row being absent — insert, don't swap, for the latter.
pub async fn compare_and_swap_config(
    name: String,
    expected: Option<&str>,
    value: Option<String>,
) -> error_stack::Result<bool, crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;
    let conn = &app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;
    let values = ServiceConfigurationUpdate {
        value: value.clone(),
    };

    // The two predicates have different types, so the branches cannot be collapsed: SQL equality
    // against NULL is never true, and `IS NULL` is the only way to match a null-valued row.
    let updated = match expected {
        Some(previous) => {
            crate::generics::generic_update_if_present::<
                <ServiceConfiguration as HasTable>::Table,
                ServiceConfigurationUpdate,
                _,
            >(
                conn,
                dsl::name
                    .eq(name.clone())
                    .and(dsl::value.eq(previous.to_string())),
                values,
            )
            .await?
        }
        None => {
            crate::generics::generic_update_if_present::<
                <ServiceConfiguration as HasTable>::Table,
                ServiceConfigurationUpdate,
                _,
            >(
                conn,
                dsl::name.eq(name.clone()).and(dsl::value.is_null()),
                values,
            )
            .await?
        }
    };

    if updated == 0 {
        return Ok(false);
    }

    // Only on a swap that actually landed — a lost race must leave the cache alone, since the
    // value it would write is the one the database just rejected.
    match value {
        Some(v) => write_through_service_config(name, &v).await,
        None => evict_service_config(name).await,
    }
    Ok(true)
}

pub async fn delete_config(name: String) -> Result<(), crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;

    let conn = &app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;
    crate::generics::generic_delete::<<ServiceConfiguration as HasTable>::Table, _>(
        conn,
        dsl::name.eq(name.clone()),
    )
    .await?;

    evict_service_config(name).await;
    Ok(())
}
