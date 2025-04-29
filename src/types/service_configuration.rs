use crate::app::get_tenant_app_state;
use crate::storage::schema::service_configuration::dsl;
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
) -> Result<(), crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;

    let config = ServiceConfigurationNew {
        name,
        value,
        new_value: None,
        previous_value: None,
        new_value_status: None,
    };

    crate::generics::generic_insert(&app_state.db, config)
        .await
        .map_err(|_| crate::generics::MeshError::Others)?;

    Ok(())
}

pub async fn update_config(
    name: String,
    value: Option<String>,
) -> Result<(), crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;
    let values = ServiceConfigurationUpdate { value };
    let conn = &app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;
    // Use Diesel's query builder with multiple conditions
    let rows = crate::generics::generic_update::<
        <ServiceConfiguration as HasTable>::Table,
        ServiceConfigurationUpdate,
        _,
    >(&conn, dsl::name.eq(name), values)
    .await
    .map_err(|_| crate::generics::MeshError::Others)?;

    if rows == 0 {
        return Err(crate::generics::MeshError::NoRowstoUpdate);
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
    // Use Diesel's query builder with multiple conditions
    let rows = crate::generics::generic_delete::<<ServiceConfiguration as HasTable>::Table, _>(
        &conn,
        dsl::name.eq(name),
    )
    .await
    .map_err(|_| crate::generics::MeshError::Others)?;

    if rows == 0 {
        return Err(crate::generics::MeshError::NoRowstoDelete);
    }

    Ok(())
}
