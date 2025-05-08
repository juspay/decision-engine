use diesel::query_builder::AsQuery;
use diesel::QuerySource;
use error_stack::report;
use std::fmt::Debug;

use crate::logger;
use crate::storage::MysqlPoolConn;
use crate::storage::Storage;
use async_bb8_diesel::AsyncRunQueryDsl;
use diesel::query_builder::QueryId;
use diesel::query_dsl::methods::ExecuteDsl;
use diesel::{
    associations::HasTable,
    debug_query,
    dsl::{Find, Limit},
    helper_types::Filter,
    insertable::CanInsertInSingleQuery,
    mysql::Mysql,
    query_builder::DeleteStatement,
    query_builder::QueryFragment,
    query_builder::UpdateStatement,
    query_builder::{InsertStatement, IntoUpdateTarget},
    query_dsl::{
        methods::{FilterDsl, FindDsl, LimitDsl},
        LoadQuery, RunQueryDsl,
    },
    result::Error as DieselError,
    AsChangeset, Insertable, MysqlConnection, Table,
};
use error_stack::ResultExt;

// use crate::{errors, MysqlPooledConn, StorageResult};

pub type StorageResult<T> = Result<T, MeshError>;

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
    #[error("No rows to be updated")]
    NoRowstoUpdate,
    #[error("No rows to be deleted")]
    NoRowstoDelete,
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

pub async fn generic_insert<T, V>(storage: &Storage, values: V) -> StorageResult<usize>
where
    T: HasTable<Table = T> + Table + 'static + Debug,
    V: Debug + Insertable<T>,
    <T as QuerySource>::FromClause: QueryFragment<Mysql> + Debug,
    <V as Insertable<T>>::Values: CanInsertInSingleQuery<Mysql> + QueryFragment<Mysql> + 'static,
    InsertStatement<T, <V as Insertable<T>>::Values>:
        AsQuery + ExecuteDsl<MysqlConnection, Mysql> + Send,
{
    let mut conn = storage.get_conn().await.map_err(|_| MeshError::Others)?;
    generic_insert_core::<T, _>(&mut conn, values).await
}

pub async fn generic_insert_core<T, V>(conn: &MysqlPoolConn, values: V) -> StorageResult<usize>
where
    T: HasTable<Table = T> + Table + 'static + Debug,
    V: Debug + Insertable<T>,
    <T as QuerySource>::FromClause: QueryFragment<Mysql> + Debug,
    <V as Insertable<T>>::Values: CanInsertInSingleQuery<Mysql> + QueryFragment<Mysql> + 'static,
    InsertStatement<T, <V as Insertable<T>>::Values>:
        AsQuery + ExecuteDsl<MysqlConnection, Mysql> + Send,
{
    let debug_values = format!("{values:?}");

    let query = diesel::insert_into(<T as HasTable>::table()).values(values);
    logger::debug!(query = %debug_query::<Mysql, _>(&query).to_string());

    match track_database_call::<T, _, _>(query.execute_async(conn), DatabaseOperation::Insert).await
    {
        Ok(value) => Ok(value),
        Err(err) => {
            print!("Error: {:?}", err);
            Err(MeshError::NotFound)
        }
    }
}

pub async fn generic_update<T, V, P>(
    conn: &MysqlPoolConn,
    predicate: P,
    values: V,
) -> StorageResult<usize>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    V: AsChangeset<Target = <Filter<T, P> as HasTable>::Table> + Debug,
    Filter<T, P>: IntoUpdateTarget,
    UpdateStatement<
        <Filter<T, P> as HasTable>::Table,
        <Filter<T, P> as IntoUpdateTarget>::WhereClause,
        <V as AsChangeset>::Changeset,
    >: AsQuery + QueryFragment<Mysql> + QueryId + Send + 'static,
{
    let debug_values = format!("{values:?}");

    let query = diesel::update(<T as HasTable>::table().filter(predicate)).set(values);
    logger::debug!(query = %debug_query::<Mysql, _>(&query).to_string());

    match track_database_call::<T, _, _>(query.execute_async(conn), DatabaseOperation::Update).await
    {
        Ok(value) => {
            logger::debug!("Updated rows: {:?}", value);
            if value == 0 {
                return Err(crate::generics::MeshError::NoRowstoUpdate);
            }
            Ok(value)
        }
        Err(err) => {
            logger::error!("Error while updating: {:?} {:?}", err, debug_values);
            Err(MeshError::NotFound)
        }
    }
}

pub async fn generic_delete<T, P>(conn: &MysqlPoolConn, predicate: P) -> StorageResult<usize>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: IntoUpdateTarget,
    DeleteStatement<
        <Filter<T, P> as HasTable>::Table,
        <Filter<T, P> as IntoUpdateTarget>::WhereClause,
    >: AsQuery + QueryFragment<Mysql> + QueryId + Send + 'static,
{
    let query = diesel::delete(<T as HasTable>::table().filter(predicate));
    logger::debug!(query = %debug_query::<Mysql, _>(&query).to_string());

    match track_database_call::<T, _, _>(query.execute_async(conn), DatabaseOperation::Delete).await
    {
        Ok(value) => {
            logger::debug!("Deleted rows: {:?}", value);
            if value == 0 {
                return Err(crate::generics::MeshError::NoRowstoDelete);
            }
            Ok(value)
        }
        Err(err) => {
            logger::error!("Error while deleting: {:?}", err);
            Err(MeshError::NotFound)
        }
    }
}

pub async fn generic_find_all<T, P, R>(storage: &Storage, predicate: P) -> StorageResult<Vec<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err) => Err(MeshError::Others),
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
            DieselError::NotFound => {
                logger::debug!("DieseslError: {:?}", err);
                MeshError::NotFound
            }
            _ => {
                logger::debug!("Error: {:?}", err);
                MeshError::Others
            }
        })
}

pub async fn generic_find_one<T, P, R>(storage: &Storage, predicate: P) -> StorageResult<R>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err) => Err(MeshError::Others),
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
        Ok(conn) => {
            logger::debug!("DB connected sccessfuly");
            Ok(conn)
        }
        Err(err) => {
            logger::debug!("Error getting connection: {:?}", err);
            Err(MeshError::Others)
        }
    }?;
    to_optional(generic_find_one_core::<T, _, _>(&conn, predicate).await)
}

pub async fn generic_find_by_id_optional<T, Pk, R>(
    storage: &Storage,
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
        Err(err) => Err(MeshError::Others),
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

async fn generic_find_by_id_core<T, Pk, R>(conn: &MysqlPoolConn, id: Pk) -> StorageResult<R>
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
            DieselError::NotFound => Err(MeshError::NotFound),
            _ => {
                logger::debug!("Error: {:?}", err);
                Err(MeshError::Others)
            }
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

// SELECT merchant_iframe_preferences.id, merchant_iframe_preferences.merchant_id, merchant_iframe_preferences.dynamic_switching_enabled, merchant_iframe_preferences.isin_routing_enabled, merchant_iframe_preferences.issuer_routing_enabled, merchant_iframe_preferences.txn_failure_gateway_penalty, merchant_iframe_preferences.card_brand_routing_enabled FROM merchant_iframe_preferences WHERE (merchant_iframe_preferences.merchant_id = 'azharamin');

// SELECT merchant_iframe_preferences.id, merchant_iframe_preferences.merchant_id, merchant_iframe_preferences.dynamic_switching_enabled, merchant_iframe_preferences.isin_routing_enabled, merchant_iframe_preferences.issuer_routing_enabled, merchant_iframe_preferences.txn_failure_gateway_penality, merchant_iframe_preferences.card_brand_routing_enabled FROM merchant_iframe_preferences WHERE (merchant_iframe_preferences.merchant_id = 'azharamin');
