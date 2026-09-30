use crate::app::get_tenant_app_state;
use crate::logger;
use crate::redis::cache::{evict_service_config, write_through_service_config};
#[cfg(feature = "mysql")]
use crate::storage::schema::service_configuration::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::service_configuration::dsl;
use async_bb8_diesel::AsyncConnection;
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

enum AtomicUpdateError {
    Diesel(diesel::result::Error),
    Transform(crate::generics::MeshError),
}

impl From<diesel::result::Error> for AtomicUpdateError {
    fn from(e: diesel::result::Error) -> Self {
        Self::Diesel(e)
    }
}

/// Read-modify-write a config row atomically: `transform` receives the current value and
/// returns the new one, all inside one DB transaction holding a `SELECT ... FOR UPDATE`
/// row lock, so concurrent writers to the same key can't lose each other's updates.
/// Inserts the row if it doesn't exist yet.
pub async fn update_config_atomic<F>(
    name: String,
    transform: F,
) -> error_stack::Result<(), crate::generics::MeshError>
where
    F: FnOnce(Option<String>) -> Result<Option<String>, crate::generics::MeshError>
        + Send
        + 'static,
{
    let app_state = get_tenant_app_state().await;
    let conn = app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;

    let cache_name = name.clone();
    let new_value = conn
        .run(move |conn| {
            conn.transaction::<Option<String>, AtomicUpdateError, _>(|conn| {
                // FOR UPDATE locks nothing when the row is absent, so also take an advisory
                // lock on the key to serialize concurrent first-time inserts. Savepoint-wrapped:
                // backends without advisory locks (e.g. CockroachDB) fall through to the row lock.
                #[cfg(feature = "postgres")]
                let _ = conn.transaction::<_, diesel::result::Error, _>(|conn| {
                    diesel::sql_query(
                        "SELECT pg_advisory_xact_lock(hashtext('service_configuration'), hashtext($1))",
                    )
                    .bind::<diesel::sql_types::Text, _>(name.clone())
                    .execute(conn)
                    .map(|_| ())
                });

                let existing = dsl::service_configuration
                    .filter(dsl::name.eq(&name))
                    .order(dsl::id.asc())
                    .limit(1)
                    .for_update()
                    .get_result::<ServiceConfiguration>(conn)
                    .optional()?;

                let new_value = transform(existing.as_ref().and_then(|c| c.value.clone()))
                    .map_err(AtomicUpdateError::Transform)?;

                match existing {
                    Some(row) => {
                        diesel::update(dsl::service_configuration.filter(dsl::id.eq(row.id)))
                            .set(dsl::value.eq(new_value.clone()))
                            .execute(conn)?;
                    }
                    None => {
                        diesel::insert_into(dsl::service_configuration)
                            .values(ServiceConfigurationNew {
                                name: name.clone(),
                                value: new_value.clone(),
                                new_value: None,
                                previous_value: None,
                                new_value_status: None,
                            })
                            .execute(conn)?;
                    }
                }
                Ok(new_value)
            })
        })
        .await
        .map_err(|e| match e {
            AtomicUpdateError::Transform(err) => error_stack::report!(err),
            AtomicUpdateError::Diesel(err) => {
                logger::error!("update_config_atomic: transaction failed: {:?}", err);
                error_stack::report!(crate::generics::MeshError::Others)
            }
        })?;

    match new_value {
        Some(v) => write_through_service_config(cache_name, &v).await,
        None => evict_service_config(cache_name).await,
    }
    Ok(())
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
