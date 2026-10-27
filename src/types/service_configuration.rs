use crate::app::get_tenant_app_state;
use crate::redis::cache::{evict_service_config, write_through_service_config};
#[cfg(feature = "mysql")]
use crate::storage::schema::service_configuration::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::service_configuration::dsl;
use diesel::associations::HasTable;
use diesel::*;
use error_stack::ResultExt;
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

/// The longest configuration name either backend stores: MySQL's unique index on `name` covers this
/// many characters in full. Names are built from merchant, connector and account ids; a longer one is
/// refused on both backends alike rather than only where the column would reject it.
pub const MAX_NAME_LEN: usize = 768;

fn check_name(name: &str) -> error_stack::Result<(), crate::generics::MeshError> {
    if name.chars().count() > MAX_NAME_LEN {
        return Err(error_stack::report!(crate::generics::MeshError::Others)).attach_printable(
            format!("configuration name longer than {MAX_NAME_LEN} characters"),
        );
    }
    Ok(())
}

pub async fn insert_config(
    name: String,
    value: Option<String>,
) -> error_stack::Result<(), crate::generics::MeshError> {
    check_name(&name)?;
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

/// The single statement behind [`upsert_config`]: insert the row or, when the unique index on `name`
/// already holds it, set its value.
macro_rules! upsert_statement {
    ($row:expr, $value:expr) => {{
        #[cfg(feature = "postgres")]
        let statement = diesel::insert_into(dsl::service_configuration)
            .values($row)
            .on_conflict(dsl::name)
            .do_update()
            .set(dsl::value.eq($value));
        #[cfg(feature = "mysql")]
        let statement = diesel::insert_into(dsl::service_configuration)
            .values($row)
            .on_conflict(diesel::dsl::DuplicatedKeys)
            .do_update()
            .set(dsl::value.eq($value));
        statement
    }};
}

/// Sets `name` to `value`, creating the row on its first write. One statement, resolved by the
/// unique index on `name`, so concurrent first writes leave one row instead of two.
pub async fn upsert_config(
    name: String,
    value: Option<String>,
) -> error_stack::Result<(), crate::generics::MeshError> {
    use async_bb8_diesel::AsyncRunQueryDsl;

    check_name(&name)?;
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;
    let row = ServiceConfigurationNew {
        name: name.clone(),
        value: value.clone(),
        new_value: None,
        previous_value: None,
        new_value_status: None,
    };
    upsert_statement!(row, value.clone())
        .execute_async(&*conn)
        .await
        .change_context(crate::generics::MeshError::Others)?;

    match value {
        Some(v) => write_through_service_config(name, &v).await,
        None => evict_service_config(name).await,
    }
    Ok(())
}

/// Sets `name`'s value only if it still holds `expected`: a compare-and-swap for read-modify-write
/// updates. Returns `false`, writing nothing, when another writer changed the value since it was
/// read; the caller re-reads and tries again.
pub async fn compare_and_update_config(
    name: String,
    expected: Option<String>,
    value: Option<String>,
) -> error_stack::Result<bool, crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;
    let values = ServiceConfigurationUpdate {
        value: value.clone(),
    };
    let conn = &app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;
    let updated = match expected {
        Some(expected) => {
            crate::generics::generic_update_if_present::<
                <ServiceConfiguration as HasTable>::Table,
                ServiceConfigurationUpdate,
                _,
            >(
                conn,
                dsl::name.eq(name.clone()).and(dsl::value.eq(expected)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_stored_only_up_to_the_indexed_length() {
        assert!(check_name(&"x".repeat(MAX_NAME_LEN)).is_ok());
        assert!(check_name(&"x".repeat(MAX_NAME_LEN + 1)).is_err());
        // Counted in characters, as the column is: a multi-byte name at the limit still fits.
        assert!(check_name(&"é".repeat(MAX_NAME_LEN)).is_ok());
    }

    fn row() -> ServiceConfigurationNew {
        ServiceConfigurationNew {
            name: "DEBIT_ROUTING_ENABLED_m1".to_string(),
            value: Some("true".to_string()),
            new_value: None,
            previous_value: None,
            new_value_status: None,
        }
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn the_upsert_resolves_a_name_collision_by_setting_the_value() {
        let sql = diesel::debug_query::<diesel::pg::Pg, _>(&upsert_statement!(
            row(),
            Some("true".to_string())
        ))
        .to_string();
        assert!(
            sql.contains(r#"ON CONFLICT ("name") DO UPDATE SET "value" = "#),
            "{sql}"
        );
    }

    #[cfg(feature = "mysql")]
    #[test]
    fn the_upsert_resolves_a_name_collision_by_setting_the_value() {
        let sql = diesel::debug_query::<diesel::mysql::Mysql, _>(&upsert_statement!(
            row(),
            Some("true".to_string())
        ))
        .to_string();
        assert!(sql.contains("ON DUPLICATE KEY UPDATE `value` = "), "{sql}");
    }
}
