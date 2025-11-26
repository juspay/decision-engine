use crate::app::get_tenant_app_state;
#[cfg(feature = "mysql")]
use crate::storage::schema::service_configuration::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::service_configuration::dsl;
use diesel::associations::HasTable;
use diesel::*;
use serde_json::json;
use std::option::Option;
use std::string::String;
// use sequelize::{Clause::{Is, And}, Term::{Eq, In}};
use crate::shard_queue::{
    find_config_in_mem, store_config_in_mem, ShardQueueItem, GLOBAL_SHARD_QUEUE_HANDLER,
};
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

    let service_config = crate::generics::generic_insert(&app_state.db, config).await?;

    // Push to shard queue so IMC gets updated automatically via polling
    // Store the ServiceConfiguration object directly as JSON value for shard queue
    if let Ok(config_json) = serde_json::to_value(&service_config) {
        let queue_item = ShardQueueItem::new(name.clone(), config_json);
        if let Err(e) = GLOBAL_SHARD_QUEUE_HANDLER.push_to_shard(queue_item).await {
            crate::logger::error!("Failed to push config '{}' to shard queue: {:?}", name, e);
        } else {
            crate::logger::debug!("Pushed config '{}' to shard queue for IMC update", name);
        }
    }

    Ok(())
}

pub async fn update_config(
    name: String,
    value: Option<String>,
) -> error_stack::Result<(), crate::generics::MeshError> {
    let app_state = get_tenant_app_state().await;
    let values = ServiceConfigurationUpdate { value };
    let conn = &app_state
        .db
        .get_conn()
        .await
        .map_err(|_| crate::generics::MeshError::DatabaseConnectionError)?;
    // Use Diesel's query builder with multiple conditions
    crate::generics::generic_update::<
        <ServiceConfiguration as HasTable>::Table,
        ServiceConfigurationUpdate,
        _,
    >(&conn, dsl::name.eq(name), values)
    .await?;

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
    crate::generics::generic_delete::<<ServiceConfiguration as HasTable>::Table, _>(
        &conn,
        dsl::name.eq(name),
    )
    .await?;

    Ok(())
}
