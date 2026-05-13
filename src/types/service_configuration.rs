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
