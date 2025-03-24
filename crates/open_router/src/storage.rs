use crate::{
    config::Database,
    error::{self, ContainerError},
};

use std::sync::Arc;

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
    pg_pool: Arc<Pool<AsyncMysqlConnection>>,
}

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

        let config =
            pooled_connection::AsyncDieselConnectionManager::<AsyncMysqlConnection>::new(database_url);
        let pool = Pool::builder(config);

        let pool = match database.pool_size {
            Some(value) => pool.max_size(value),
            None => pool,
        };

        let pool = pool
            .build()
            .change_context(error::StorageError::DBPoolError)?;
        Ok(Self {
            pg_pool: Arc::new(pool),
        })
    }

    /// Get connection from database pool for accessing data
    pub async fn get_conn(&self) -> Result<DeadPoolConnType, ContainerError<error::StorageError>> {
        Ok(self
            .pg_pool
            .get()
            .await
            .change_context(error::StorageError::PoolClientFailure)?)
    }
}

pub(crate) trait TestInterface {
    type Error;
    async fn test(&self) -> Result<(), ContainerError<Self::Error>>;
}
