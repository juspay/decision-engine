
use std::fmt::Debug;

use async_bb8_diesel::AsyncRunQueryDsl;
use diesel::{
    associations::HasTable, debug_query, dsl::{Find, Limit}, helper_types::{Filter, IntoBoxed}, insertable::CanInsertInSingleQuery, mysql::Mysql, query_builder::{
        AsChangeset, AsQuery, DeleteStatement, InsertStatement, IntoUpdateTarget, QueryFragment,
        QueryId, UpdateStatement,
    }, query_dsl::{
        methods::{BoxedDsl, FilterDsl, FindDsl, LimitDsl, OffsetDsl, OrderDsl},
        LoadQuery, RunQueryDsl,
    }, result::Error as DieselError, Expression, Insertable, MysqlConnection, QueryDsl, QuerySource, Table
};
use diesel_async::{AsyncMysqlConnection};
use error_stack::{report, ResultExt};
use crate::logger;
use crate::storage::Storage;
use bb8::PooledConnection;
use crate::storage::MysqlPoolConn;

// use crate::{errors, MysqlPooledConn, StorageResult};

pub type StorageResult<T> = Result<T,MeshError>;

#[derive(Copy, Clone, Debug, thiserror::Error)]
pub enum MeshError {
    #[error("An error occurred when obtaining database connection")]
    DatabaseConnectionError,
    #[error("The requested resource was not found in the database")]
    NotFound,
    #[error("A unique constraint violation occurred")]
    UniqueViolation,
    #[error("No fields were provided to be updated")]
    NoFieldsToUpdate,
    #[error("An error occurred when generating typed SQL query")]
    QueryGenerationFailed,
    // InsertFailed,
    #[error("An unknown error occurred")]
    Others,
}

pub enum DatabaseOperation {
    FindOne,
    Filter,
    Update,
    Insert,
    Delete,
    DeleteWithResult,
    UpdateWithResults,
    UpdateOne,
    Count,
}

pub async fn generic_find_all<T, P, R>(storage: &Storage, predicate: P) -> StorageResult<Vec<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err)            => Err(MeshError::Others)
    }?;
    generic_filter::<T, _, _>(&conn, predicate).await
}

async fn generic_filter<T, P, R>(conn: &MysqlPoolConn, predicate: P) -> StorageResult<Vec<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let query = T::table().filter(predicate);
    // query = query.filter(predicate);

    // if let Some(limit) = limit {
    //     query = query.limit(limit);
    // }

    // if let Some(offset) = offset {
    //     query = query.offset(offset);
    // }

    // if let Some(order) = order {
    //     query = query.order(order);
    // }

    logger::debug!(query = %debug_query::<Mysql, _>(&query).to_string());

    track_database_call::<T, _, _>(query.get_results_async(conn), DatabaseOperation::Filter)
        .await
        .map_err(|err| match err {
            DieselError::NotFound => MeshError::NotFound,
            _ => MeshError::Others,
        }) 
}

async fn generic_find_one_core<T, P, R>(conn: &MysqlPoolConn, predicate: P) -> StorageResult<R>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let query = <T as HasTable>::table().filter(predicate);
    logger::debug!(query = %debug_query::<Mysql, _>(&query).to_string());

    track_database_call::<T, _, _>(query.get_result_async(conn), DatabaseOperation::FindOne)
        .await
        .map_err(|err| match err {
            DieselError::NotFound => MeshError::NotFound,
            _ => MeshError::Others, 
        })
}

pub async fn generic_find_one<T, P, R>(
    storage: &Storage,
    predicate: P,
) -> StorageResult<R>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err)            => Err(MeshError::Others)
    }?;
    generic_find_one_core::<T, _, _>(&conn, predicate).await
}

pub async fn generic_find_one_optional<T, P, R>(
    storage: &Storage,
    predicate: P,
) -> StorageResult<Option<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err)            => Err(MeshError::Others)
    }?;
    to_optional(generic_find_one_core::<T, _, _>(&conn, predicate).await)
}

pub async fn generic_find_by_id_optional<T, Pk, R>(
    storage : &Storage,
    id: Pk,
) -> StorageResult<Option<R>>
where
    T: FindDsl<Pk> + HasTable<Table = T> + LimitDsl + Table + 'static,
    <T as HasTable>::Table: FindDsl<Pk>,
    Find<T, Pk>: LimitDsl + QueryFragment<Mysql> + RunQueryDsl<MysqlConnection> + Send + 'static,
    Limit<Find<T, Pk>>: LoadQuery<'static, MysqlConnection, R>,
    Pk: Clone + Debug,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err)            => Err(MeshError::Others)
    }?;
    to_optional(generic_find_by_id_core::<T, _, _>(&conn, id).await)
}

pub async fn track_database_call<T, Fut, U>(future: Fut, operation: DatabaseOperation) -> U
    where
        Fut: std::future::Future<Output = U>,
    {
        let start = std::time::Instant::now();
        let output = future.await;
        output
    }

async fn generic_find_by_id_core<T, Pk, R>(conn : &MysqlPoolConn, id: Pk) -> StorageResult<R>
where
    T: FindDsl<Pk> + HasTable<Table = T> + LimitDsl + Table + 'static,
    Find<T, Pk>: LimitDsl + QueryFragment<Mysql> + RunQueryDsl<MysqlConnection> + Send + 'static,
    Limit<Find<T, Pk>>: LoadQuery<'static, MysqlConnection, R>,
    Pk: Clone + Debug,
    R: Send + 'static,
{
    let query = <T as HasTable>::table().find(id.to_owned());
    logger::debug!(query = %debug_query::<Mysql, _>(&query).to_string());
    


    match track_database_call::<T, _, _>(query.first_async(conn), DatabaseOperation::FindOne).await
    {
        Ok(value) => Ok(value),
        Err(err) => match err {
            DieselError::NotFound => {
                Err(MeshError::NotFound)
            }
            _ => Err(MeshError::Others),
        },
    }
}

fn to_optional<T>(arg: StorageResult<T>) -> StorageResult<Option<T>> {
    match arg {
        Ok(value) => Ok(Some(value)),
        Err(err) => match err {
            MeshError::NotFound => Ok(None),
            _ => Err(err),
        },
    }
}


