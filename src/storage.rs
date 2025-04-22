use crate::{
    config::Database,
    error::{self, ContainerError},
};

use crate::generics::StorageResult;
use bb8::PooledConnection;
use diesel::MysqlConnection;
use diesel_async::{
    pooled_connection::{
        self,
        deadpool::{Object, Pool},
    },
    AsyncMysqlConnection,
};
use error_stack::ResultExt;
use masking::PeekInterface;

pub mod consts;
pub mod db;
pub mod schema;
pub mod types;
pub mod utils;

pub trait State {}

/// Storage State that is to be passed though the application
#[derive(Clone)]
pub struct Storage {
    pg_pool: MysqlPool,
}

pub type MysqlPooledConn = async_bb8_diesel::ConnectionManager<MysqlConnection>;
pub type MysqlPoolConn = async_bb8_diesel::Connection<diesel::MysqlConnection>;

pub type MysqlPool = bb8::Pool<MysqlPooledConn>;

type DeadPoolConnType = Object<AsyncMysqlConnection>;

impl Storage {
    /// Create a new storage interface from configuration
    pub async fn new(
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
        Ok(Self { pg_pool: pool })
    }

    /// Get connection from database pool for accessing data
    pub async fn get_conn(
        &self,
    ) -> StorageResult<PooledConnection<'_, async_bb8_diesel::ConnectionManager<MysqlConnection>>>
    {
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
        // .min_idle(database.min_idle)
        // .queue_strategy(database.queue_strategy.into())
        .connection_timeout(std::time::Duration::from_secs(60));
    // .max_lifetime(database.max_lifetime.map(std::time::Duration::from_secs));

    // if test_transaction {
    //     pool = pool.connection_customizer(Box::new(TestTransaction));
    // }

    pool.build(manager)
        .await
        .change_context(error::StorageError::InitializationError)
        .attach_printable("Failed to create PostgreSQL connection pool")
}
