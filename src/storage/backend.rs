//! The one place the database backend is chosen. Everything else is written once against these
//! names, and the `mysql` / `postgres` feature decides what they mean.

#[cfg(feature = "mysql")]
mod imp {
    pub type Db = diesel::mysql::Mysql;
    pub type DbConnection = diesel::MysqlConnection;
    pub type PoolConn = crate::storage::MysqlPoolConn;
    pub use crate::storage::schema;
}

#[cfg(feature = "postgres")]
mod imp {
    pub type Db = diesel::pg::Pg;
    pub type DbConnection = diesel::PgConnection;
    pub type PoolConn = crate::storage::PgPoolConn;
    pub use crate::storage::schema_pg as schema;
}

pub use imp::*;
