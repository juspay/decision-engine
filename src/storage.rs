use crate::error::{self, ContainerError};

#[cfg(feature = "mysql")]
use crate::config::Database;
#[cfg(feature = "postgres")]
use crate::config::PgDatabase;

#[cfg(feature = "mysql")]
use crate::logger;

use crate::generics::StorageResult;
use bb8::PooledConnection;
#[cfg(feature = "mysql")]
use std::time::Duration;
#[cfg(feature = "mysql")]
use tokio::time;

#[cfg(feature = "mysql")]
use diesel::MysqlConnection;
#[cfg(feature = "postgres")]
use diesel::PgConnection;

use error_stack::ResultExt;
use masking::PeekInterface;

pub mod api_keys;
pub mod backend;
pub mod consts;
pub mod db;
#[cfg(feature = "mysql")]
pub mod schema;
#[cfg(feature = "postgres")]
pub mod schema_pg;
pub mod types;
pub mod utils;

pub trait State {}

/// Storage State that is to be passed through the application
#[derive(Clone)]
pub struct Storage {
    #[cfg(feature = "postgres")]
    pg_pool: PgPool,
    #[cfg(feature = "mysql")]
    pg_pool: MysqlPool,
}

#[cfg(feature = "postgres")]
pub type PgPooledConn = async_bb8_diesel::ConnectionManager<PgConnection>;
#[cfg(feature = "postgres")]
pub type PgPoolConn = async_bb8_diesel::Connection<diesel::PgConnection>;
#[cfg(feature = "postgres")]
pub type PgPool = bb8::Pool<PgPooledConn>;

#[cfg(feature = "mysql")]
pub type MysqlPooledConn = async_bb8_diesel::ConnectionManager<MysqlConnection>;
#[cfg(feature = "mysql")]
pub type MysqlPoolConn = async_bb8_diesel::Connection<diesel::MysqlConnection>;
#[cfg(feature = "mysql")]
pub type MysqlPool = bb8::Pool<MysqlPooledConn>;

#[cfg(feature = "postgres")]
impl Storage {
    /// Create a new storage interface from configuration
    pub async fn new(
        database: &PgDatabase,
        schema: &str,
    ) -> error_stack::Result<Self, error::StorageError> {
        let pool = diesel_make_pg_pool(database, schema, false).await?;
        Ok(Self { pg_pool: pool })
    }
    pub async fn get_conn(
        &self,
    ) -> StorageResult<PooledConnection<'_, async_bb8_diesel::ConnectionManager<PgConnection>>>
    {
        match self.pg_pool.get().await {
            Ok(conn) => Ok(conn),
            Err(_err) => Err(crate::generics::MeshError::DatabaseConnectionError),
        }
    }
}
#[cfg(feature = "mysql")]
impl Storage {
    /// Create a new storage interface from configuration
    pub async fn new(
        //featire flag
        database: &Database,
        schema: &str,
    ) -> error_stack::Result<Self, error::StorageError> {
        let pool = diesel_make_mysql_pool(database, schema, false).await?;
        Ok(Self { pg_pool: pool })
    }

    /// Get connection from database pool for accessing data
    pub async fn get_conn(
        &self,
    ) -> StorageResult<PooledConnection<'_, async_bb8_diesel::ConnectionManager<MysqlConnection>>>
    {
        let timeout_duration = Duration::from_secs(10);
        match time::timeout(timeout_duration, self.pg_pool.get()).await {
            Ok(Ok(conn)) => {
                logger::debug!(
                    action = "DB_CONNECTION_SUCCESS",
                    "connection to db successful"
                );

                Ok(conn)
            }
            Ok(Err(_err)) => {
                logger::error!(
                    action = "DB_CONNECTION_FAILURE",
                    "Failed to get connection from pool: {:?}",
                    _err
                );
                Err(crate::generics::MeshError::DatabaseConnectionError)
            }
            Err(_elapsed) => {
                logger::error!(
                    action = "DB_CONNECTION_FAILURE",
                    "time exceeded for DB connection: {:?}",
                    _elapsed
                );
                Err(crate::generics::MeshError::DatabaseConnectionError)
            } // timeout occurred
        }
    }
}

/// Connections per pool when the database config sets no size.
const DEFAULT_POOL_MAX_SIZE: u32 = 50;

/// How long a caller waits for a free pooled connection before the query fails.
const POOL_CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The configured pool size, or [`DEFAULT_POOL_MAX_SIZE`]. bb8 refuses a size of zero.
fn pool_max_size(configured: Option<usize>) -> u32 {
    configured
        .and_then(|size| u32::try_from(size).ok())
        .filter(|size| *size > 0)
        .unwrap_or(DEFAULT_POOL_MAX_SIZE)
}

pub(crate) trait TestInterface {
    type Error;
    async fn test(&self) -> Result<(), ContainerError<Self::Error>>;
}

/// Build the `postgres://` connection URL shared by every PostgreSQL pool.
///
/// CockroachDB speaks the PostgreSQL wire protocol, so the same URL, `search_path` option, and
/// Diesel `postgres` backend drive it unchanged. `sslmode`/`sslrootcert` are appended only when
/// configured — a secure CockroachDB cluster (e.g. CockroachDB Cloud) needs `sslmode=verify-full`,
/// while a plain PostgreSQL or insecure local CockroachDB node connects with libpq's default.
#[cfg(feature = "postgres")]
fn pg_database_url(database: &PgDatabase, schema: &str) -> String {
    // Keep the space percent-encoded: libpq 16 rejects a raw one ("unexpected spaces found in ...")
    // and no connection can be opened at all. libpq 14 tolerated it, which is why it went unnoticed.
    let mut url = format!(
        "postgres://{}:{}@{}:{}/{}?application_name={}&options=-c%20search_path%3D{}",
        database.pg_username,
        database.pg_password.peek(),
        database.pg_host,
        database.pg_port,
        database.pg_dbname,
        schema,
        schema
    );
    if let Some(sslmode) = database.pg_sslmode.as_deref().filter(|s| !s.is_empty()) {
        url.push_str("&sslmode=");
        url.push_str(&encode_query_value(sslmode));
    }
    if let Some(cert) = database
        .pg_ssl_root_cert
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        url.push_str("&sslrootcert=");
        url.push_str(&encode_query_value(cert));
    }
    url
}

/// Percent-encode a connection-URL query value (RFC 3986 unreserved set kept, everything else
/// escaped). An sslrootcert path can contain spaces or reserved characters (`&`, `?`, …) that would
/// otherwise break the query string — libpq rejects a raw space outright.
#[cfg(feature = "postgres")]
fn encode_query_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(feature = "postgres")]
pub async fn diesel_make_pg_pool(
    database: &PgDatabase,
    schema: &str,
    _test_transaction: bool,
) -> error_stack::Result<PgPool, error::StorageError> {
    let database_url = pg_database_url(database, schema);
    let manager = async_bb8_diesel::ConnectionManager::<PgConnection>::new(database_url);
    let pool = bb8::Pool::builder()
        .max_size(pool_max_size(database.pg_pool_size))
        .connection_timeout(POOL_CONNECTION_TIMEOUT);

    pool.build(manager)
        .await
        .change_context(error::StorageError::InitializationError)
        .attach_printable("Failed to create PostgreSQL connection pool")
}

#[cfg(feature = "mysql")]
pub async fn diesel_make_mysql_pool(
    database: &Database,
    schema: &str,
    _test_transaction: bool,
) -> error_stack::Result<MysqlPool, error::StorageError> {
    let database_url = format!(
        "mysql://{}:{}@{}:{}/{}?application_name={}&options=-c%20search_path%3D{}",
        database.username,
        database.password.peek(),
        database.host,
        database.port,
        database.dbname,
        schema,
        schema
    );
    let manager = async_bb8_diesel::ConnectionManager::<MysqlConnection>::new(database_url);
    let pool = bb8::Pool::builder()
        .max_size(pool_max_size(database.pool_size))
        .connection_timeout(POOL_CONNECTION_TIMEOUT);

    pool.build(manager)
        .await
        .change_context(error::StorageError::InitializationError)
        .attach_printable("Failed to create MySQL connection pool")
}

#[cfg(test)]
mod tests {
    use super::{pool_max_size, DEFAULT_POOL_MAX_SIZE};

    #[test]
    fn the_configured_pool_size_is_used() {
        assert_eq!(pool_max_size(Some(12)), 12);
    }

    #[test]
    fn an_unset_or_zero_pool_size_falls_back_to_the_default() {
        assert_eq!(pool_max_size(None), DEFAULT_POOL_MAX_SIZE);
        assert_eq!(pool_max_size(Some(0)), DEFAULT_POOL_MAX_SIZE);
    }
}
