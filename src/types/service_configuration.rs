use crate::app::get_tenant_app_state;
use crate::redis::cache::{evict_service_config, write_through_service_config};
use crate::storage::backend::schema::service_configuration::dsl;
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
    use error_stack::ResultExt;

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

/// A per-merchant setting kept in `service_configuration` under `"{PREFIX}_{merchant}"`.
///
/// The key and its value type are declared once here; code that names one of these rows builds
/// the name with [`MerchantConfig::name`], and code that reads or writes it whole goes through the
/// typed functions below ([`get`], [`create`], [`update`], [`upsert`], [`delete`]).
pub trait MerchantConfig {
    const PREFIX: &'static str;
    type Value: serde::Serialize + serde::de::DeserializeOwned;

    fn name(merchant_id: &str) -> String {
        format!("{}_{merchant_id}", Self::PREFIX)
    }
}

/// SR v3 routing inputs (bucket sizes, hedging, margin, sub-level overrides).
pub struct SrV3Input;
impl MerchantConfig for SrV3Input {
    const PREFIX: &'static str = "SR_V3_INPUT_CONFIG";
    type Value = crate::types::routing_configuration::SuccessRateData;
}

/// Debit-routing rule configuration.
pub struct DebitRoutingRule;
impl MerchantConfig for DebitRoutingRule {
    const PREFIX: &'static str = "DEBIT_ROUTING_CONFIG";
    type Value = crate::types::routing_configuration::DebitRoutingData;
}

/// Whether debit routing is switched on for the merchant.
pub struct DebitRoutingEnabled;
impl MerchantConfig for DebitRoutingEnabled {
    const PREFIX: &'static str = "DEBIT_ROUTING_ENABLED";
    type Value = bool;
}

/// The dimensions SR clusters are split on.
pub struct SrDimensions;
impl MerchantConfig for SrDimensions {
    const PREFIX: &'static str = "SR_DIMENSION_CONFIG";
    type Value = crate::euclid::types::SrDimensionConfig;
}

/// Why a typed configuration read or write failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration storage failed")]
    Storage,
    #[error("the stored configuration is not valid")]
    Invalid,
    #[error("the configuration already exists")]
    AlreadyExists,
}

type ConfigResult<T> = error_stack::Result<T, ConfigError>;

/// The merchant's stored value, `None` when nothing is stored. A stored value that does not parse
/// is [`ConfigError::Invalid`], carrying the parse error, never mistaken for absence.
pub async fn get<K: MerchantConfig>(
    merchant: &crate::auth::AuthorizedMerchant,
) -> ConfigResult<Option<K::Value>> {
    use error_stack::ResultExt;
    find_config_by_name(K::name(merchant.as_str()))
        .await
        .change_context(ConfigError::Storage)?
        .and_then(|row| row.value)
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .change_context(ConfigError::Invalid)
}

fn serialize<K: MerchantConfig>(value: &K::Value) -> ConfigResult<String> {
    use error_stack::ResultExt;
    serde_json::to_string(value).change_context(ConfigError::Storage)
}

/// Stores the merchant's first value; [`ConfigError::AlreadyExists`] when one is stored.
pub async fn create<K: MerchantConfig>(
    merchant: &crate::auth::AuthorizedMerchant,
    value: &K::Value,
) -> ConfigResult<()> {
    use error_stack::ResultExt;
    let name = K::name(merchant.as_str());
    let serialized = serialize::<K>(value)?;
    if find_config_by_name(name.clone())
        .await
        .change_context(ConfigError::Storage)?
        .is_some()
    {
        return Err(error_stack::report!(ConfigError::AlreadyExists));
    }
    insert_config(name, Some(serialized))
        .await
        .change_context(ConfigError::Storage)
}

/// Replaces the merchant's stored value; fails when nothing is stored.
pub async fn update<K: MerchantConfig>(
    merchant: &crate::auth::AuthorizedMerchant,
    value: &K::Value,
) -> ConfigResult<()> {
    use error_stack::ResultExt;
    let serialized = serialize::<K>(value)?;
    update_config(K::name(merchant.as_str()), Some(serialized))
        .await
        .change_context(ConfigError::Storage)
}

/// Sets the merchant's value, whether or not one is stored.
pub async fn upsert<K: MerchantConfig>(
    merchant: &crate::auth::AuthorizedMerchant,
    value: &K::Value,
) -> ConfigResult<()> {
    use error_stack::ResultExt;
    let serialized = serialize::<K>(value)?;
    upsert_config(K::name(merchant.as_str()), Some(serialized))
        .await
        .change_context(ConfigError::Storage)
}

/// Removes the merchant's stored value.
pub async fn delete<K: MerchantConfig>(
    merchant: &crate::auth::AuthorizedMerchant,
) -> ConfigResult<()> {
    delete_config(K::name(merchant.as_str()))
        .await
        .map_err(|error| error_stack::report!(error).change_context(ConfigError::Storage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merchant_config_names_keep_their_stored_spelling() {
        assert_eq!(SrV3Input::name("m1"), "SR_V3_INPUT_CONFIG_m1");
        assert_eq!(DebitRoutingRule::name("m1"), "DEBIT_ROUTING_CONFIG_m1");
        assert_eq!(DebitRoutingEnabled::name("m1"), "DEBIT_ROUTING_ENABLED_m1");
        assert_eq!(SrDimensions::name("m1"), "SR_DIMENSION_CONFIG_m1");
    }

    #[test]
    fn the_debit_routing_flag_reads_the_stored_text() {
        // Stored as `true`/`false` text by the dashboard toggle; the typed value parses it as is.
        assert_eq!(serde_json::from_str::<bool>("true").ok(), Some(true));
        assert_eq!(serde_json::to_string(&false).ok().as_deref(), Some("false"));
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
