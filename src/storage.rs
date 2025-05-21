use crate::{
    config::Database,
    config::PgDatabase,
    error::{self, ContainerError},
};

use crate::generics::StorageResult;
use bb8::PooledConnection;

#[cfg(feature = "db_migration")]
use diesel::PgConnection;
#[cfg(feature = "db_migration")]
use diesel_async::{
    AsyncPgConnection,
    pooled_connection::{
        self,
        deadpool::{Object, Pool},
    },
};
#[cfg(not(feature = "db_migration"))]
use diesel::MysqlConnection;
#[cfg(not(feature = "db_migration"))]
use diesel_async::{
    AsyncMysqlConnection,
    pooled_connection::{
        self,
        deadpool::{Object, Pool},
    },
};

use error_stack::ResultExt;
use masking::PeekInterface;

pub mod consts;
pub mod db;
#[cfg(not(feature = "db_migration"))]
pub mod schema;
#[cfg(feature = "db_migration")]
pub mod schema_pg;
pub mod types;
pub mod utils;

pub trait State {}

/// Storage State that is to be passed through the application
#[derive(Clone)]
pub struct Storage {
    #[cfg(feature = "db_migration")]
    pg_pool: PgPool,
    #[cfg(not(feature = "db_migration"))]
    pg_pool: MysqlPool,
}

#[cfg(feature = "db_migration")]
pub type PgPooledConn = async_bb8_diesel::ConnectionManager<PgConnection>;
#[cfg(feature = "db_migration")]
pub type PgPoolConn = async_bb8_diesel::Connection<diesel::PgConnection>;
#[cfg(feature = "db_migration")]
pub type PgPool = bb8::Pool<PgPooledConn>;

#[cfg(not(feature = "db_migration"))]
pub type MysqlPooledConn = async_bb8_diesel::ConnectionManager<MysqlConnection>;
#[cfg(not(feature = "db_migration"))]
pub type MysqlPoolConn = async_bb8_diesel::Connection<diesel::MysqlConnection>;
#[cfg(not(feature = "db_migration"))]
pub type MysqlPool = bb8::Pool<MysqlPooledConn>;

#[cfg(feature = "db_migration")]
type DeadPoolConnType = Object<AsyncPgConnection>;

#[cfg(not(feature = "db_migration"))]
type DeadPoolConnType = Object<AsyncMysqlConnection>;

#[cfg(feature = "db_migration")]
impl Storage {
    /// Create a new storage interface from configuration
    pub async fn new(
        database: &PgDatabase,
        schema: &str,
    ) -> error_stack::Result<Self, error::StorageError> {
            let database_url = format!(
                "postgres://{}:{}@{}:{}/{}?application_name={}&options=-c search_path%3D{}",
                database.pg_username,
                database.pg_password.peek(),
                database.pg_host,
                database.pg_port,
                database.pg_dbname,
                schema,
                schema
            );

            let config =
                pooled_connection::AsyncDieselConnectionManager::<AsyncPgConnection>::new(
                    database_url,
                );
            let pool = Pool::builder(config);

            let pool = match database.pg_pool_size {
                Some(value) => pool.max_size(value),
                None => pool,
            };

            let pool = diesel_make_pg_pool(database, schema, false).await?;
            return Ok(Self { pg_pool: pool });

    }
    pub async fn get_conn(
        &self,
    ) -> StorageResult<
        PooledConnection<'_, async_bb8_diesel::ConnectionManager<
            PgConnection,
        >>,
    > {
        match self.pg_pool.get().await {
            Ok(conn) => Ok(conn),
            Err(err) => Err(crate::generics::MeshError::DatabaseConnectionError),
        }
    }
}
#[cfg(not(feature = "db_migration"))]
impl Storage {
        /// Create a new storage interface from configuration
    pub async fn new(
            //featire flag
        database: &Database,
        schema: &str,
    ) -> error_stack::Result<Self, error::StorageError> {
        
        let database_url = format!(
            "mysql://{}:{}@{}:{}/{}?application_name={}&options=-c search_path%3D{}",
            database.username,                    
            database.password.peek(),
            database.host,
            database.port,
            database.dbname,
            schema,
            schema
        );
    
        let config = pooled_connection::AsyncDieselConnectionManager::<AsyncMysqlConnection>::new(
                        database_url,
        );
        let pool = Pool::builder(config);
    
        let pool = match database.pool_size {
                Some(value) => pool.max_size(value),
                None => pool,
        };
    
        let pool = diesel_make_mysql_pool(database, schema, false).await?;
        return Ok(Self { pg_pool: pool });
    }

    /// Get connection from database pool for accessing data
    pub async fn get_conn(
        &self,
    ) -> StorageResult<
        PooledConnection<'_, async_bb8_diesel::ConnectionManager<
            MysqlConnection,
        >>,
    > {
        match self.pg_pool.get().await {
            Ok(conn) => Ok(conn),
            Err(err) => Err(crate::generics::MeshError::DatabaseConnectionError),
        }
    }
}

    
    pub(crate) trait TestInterface {
    type Error;
    async fn test(&self) -> Result<(), ContainerError<Self::Error>>;
}

#[cfg(feature = "db_migration")]
pub async fn diesel_make_pg_pool(
    database: &PgDatabase,
    schema: &str,
    test_transaction: bool,
) -> error_stack::Result<PgPool, error::StorageError> {
    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}?application_name={}&options=-c search_path%3D{}",
        database.pg_username,
        database.pg_password.peek(),
        database.pg_host,
        database.pg_port,
        database.pg_dbname,
        schema,
        schema
    );
    let manager = async_bb8_diesel::ConnectionManager::<PgConnection>::new(database_url);
    let pool = bb8::Pool::builder()
        .max_size(50)
        .connection_timeout(std::time::Duration::from_secs(60));

    pool.build(manager)
        .await
        .change_context(error::StorageError::InitializationError)
        .attach_printable("Failed to create PostgreSQL connection pool")
}

#[cfg(not(feature = "db_migration"))]
pub async fn diesel_make_mysql_pool(
    database: &Database,
    schema: &str,
    test_transaction: bool,
) -> error_stack::Result<MysqlPool, error::StorageError> {
    let database_url = format!(
        "mysql://{}:{}@{}:{}/{}?application_name={}&options=-c search_path%3D{}",
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
        .max_size(50)
        .connection_timeout(std::time::Duration::from_secs(60));

    pool.build(manager)
        .await
        .change_context(error::StorageError::InitializationError)
        .attach_printable("Failed to create MySQL connection pool")
}
