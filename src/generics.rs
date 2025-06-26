use diesel::query_builder::AsQuery;
use diesel::QuerySource;
use error_stack::report;
use std::fmt::Debug;

use crate::logger;

#[cfg(feature = "mysql")]
use crate::storage::MysqlPoolConn;

#[cfg(feature = "postgres")]
use crate::storage::PgPoolConn;

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
    query_builder::DeleteStatement,
    query_builder::QueryFragment,
    query_builder::UpdateStatement,
    query_builder::{InsertStatement, IntoUpdateTarget},
    query_dsl::{
        methods::{FilterDsl, FindDsl, LimitDsl},
        LoadQuery, RunQueryDsl,
    },
    result::Error as DieselError,
    AsChangeset, Insertable, Table,
};
#[cfg(feature = "mysql")]
use diesel::{mysql::Mysql, MysqlConnection};
#[cfg(feature = "postgres")]
use diesel::{pg::Pg, PgConnection};
use error_stack::Report;
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

#[cfg(feature = "mysql")]
pub async fn generic_insert<T, V>(storage: &Storage, values: V) -> Result<usize, Report<MeshError>>
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

#[cfg(feature = "postgres")]
pub async fn generic_insert<T, V>(storage: &Storage, values: V) -> Result<usize, Report<MeshError>>
where
    T: HasTable<Table = T> + Table + 'static + Debug,
    V: Debug + Insertable<T>,
    <T as QuerySource>::FromClause: QueryFragment<Pg> + Debug,
    <V as Insertable<T>>::Values: CanInsertInSingleQuery<Pg> + QueryFragment<Pg> + 'static,
    InsertStatement<T, <V as Insertable<T>>::Values>: AsQuery + ExecuteDsl<PgConnection, Pg> + Send,
{
    let mut conn = storage.get_conn().await.map_err(|_| MeshError::Others)?;
    generic_insert_core::<T, _>(&mut conn, values).await
}

#[cfg(feature = "mysql")]
pub async fn generic_insert_core<T, V>(
    conn: &MysqlPoolConn,
    values: V,
) -> Result<usize, Report<MeshError>>
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
    logger::debug!(
        action = "generic_insert",
        "Debug query : {:?}",
        debug_query::<Mysql, _>(&query).to_string()
    );

    track_database_call::<T, _, _>(query.execute_async(conn), DatabaseOperation::Insert)
        .await
        .change_context(MeshError::Others)
}

#[cfg(feature = "postgres")]
pub async fn generic_insert_core<T, V>(
    conn: &PgPoolConn,
    values: V,
) -> Result<usize, Report<MeshError>>
where
    T: HasTable<Table = T> + Table + 'static + Debug,
    V: Debug + Insertable<T>,
    <T as QuerySource>::FromClause: QueryFragment<Pg> + Debug,
    <V as Insertable<T>>::Values: CanInsertInSingleQuery<Pg> + QueryFragment<Pg> + 'static,
    InsertStatement<T, <V as Insertable<T>>::Values>: AsQuery + ExecuteDsl<PgConnection, Pg> + Send,
{
    let debug_values = format!("{values:?}");
    let query = diesel::insert_into(<T as HasTable>::table()).values(values);
    logger::debug!(
        action = "generic_insert",
        "Debug query : {:?}",
        debug_query::<Pg, _>(&query).to_string()
    );

    track_database_call::<T, _, _>(query.execute_async(conn), DatabaseOperation::Insert)
        .await
        .change_context(MeshError::Others)
}
// Returns error incase of entry not found in DB or due to other issues
#[cfg(feature = "mysql")]
pub async fn generic_update<T, V, P>(
    conn: &MysqlPoolConn,
    predicate: P,
    values: V,
) -> Result<usize, Report<MeshError>>
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
    generic_update_if_present::<T, _, _>(conn, predicate, values)
        .await
        .and_then(|res| {
            logger::debug!("Updated rows: {:?}", res);
            if res == 0 {
                return Err(report!(crate::generics::MeshError::NoRowstoUpdate));
            }
            Ok(res)
        })
}
#[cfg(feature = "postgres")]
pub async fn generic_update<T, V, P>(
    conn: &PgPoolConn,
    predicate: P,
    values: V,
) -> Result<usize, Report<MeshError>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    V: AsChangeset<Target = <Filter<T, P> as HasTable>::Table> + Debug,
    Filter<T, P>: IntoUpdateTarget,
    UpdateStatement<
        <Filter<T, P> as HasTable>::Table,
        <Filter<T, P> as IntoUpdateTarget>::WhereClause,
        <V as AsChangeset>::Changeset,
    >: AsQuery + QueryFragment<Pg> + QueryId + Send + 'static,
{
    generic_update_if_present::<T, _, _>(conn, predicate, values)
        .await
        .and_then(|res| {
            logger::debug!("Updated rows: {:?}", res);
            if res == 0 {
                return Err(report!(crate::generics::MeshError::NoRowstoUpdate));
            }
            Ok(res)
        })
}
// Returns 0 incase of entry not found in DB and errors due to other issues
#[cfg(feature = "mysql")]
pub async fn generic_update_if_present<T, V, P>(
    conn: &MysqlPoolConn,
    predicate: P,
    values: V,
) -> Result<usize, Report<MeshError>>
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
    let debug_values = format!("Error while updating: {values:?}");

    let query = diesel::update(<T as HasTable>::table().filter(predicate)).set(values);
    logger::debug!(
        action = "generic_update_if_present",
        "Debug Query {:?}",
        debug_query::<Mysql, _>(&query).to_string()
    );

    track_database_call::<T, _, _>(query.execute_async(conn), DatabaseOperation::Update)
        .await
        .change_context(MeshError::Others)
        .attach_printable(debug_values)
}

#[cfg(feature = "postgres")]
pub async fn generic_update_if_present<T, V, P>(
    conn: &PgPoolConn,
    predicate: P,
    values: V,
) -> Result<usize, Report<MeshError>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    V: AsChangeset<Target = <Filter<T, P> as HasTable>::Table> + Debug,
    Filter<T, P>: IntoUpdateTarget,
    UpdateStatement<
        <Filter<T, P> as HasTable>::Table,
        <Filter<T, P> as IntoUpdateTarget>::WhereClause,
        <V as AsChangeset>::Changeset,
    >: AsQuery + QueryFragment<Pg> + QueryId + Send + 'static,
{
    let debug_values = format!("Error while updating: {values:?}");

    let query = diesel::update(<T as HasTable>::table().filter(predicate)).set(values);
    logger::debug!(
        action = "generic_update_if_present",
        "Debug Query {:?}",
        debug_query::<Pg, _>(&query).to_string()
    );

    track_database_call::<T, _, _>(query.execute_async(conn), DatabaseOperation::Update)
        .await
        .change_context(MeshError::Others)
        .attach_printable(debug_values)
}

#[cfg(feature = "mysql")]
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
    logger::debug!(
        action = "generic_delete",
        "Debug Query {:?}",
        debug_query::<Mysql, _>(&query).to_string()
    );

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
#[cfg(feature = "postgres")]
pub async fn generic_delete<T, P>(conn: &PgPoolConn, predicate: P) -> StorageResult<usize>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: IntoUpdateTarget,
    DeleteStatement<
        <Filter<T, P> as HasTable>::Table,
        <Filter<T, P> as IntoUpdateTarget>::WhereClause,
    >: AsQuery + QueryFragment<Pg> + QueryId + Send + 'static,
{
    let query = diesel::delete(<T as HasTable>::table().filter(predicate));
    logger::debug!(
        action = "generic_delete",
        "Debug Query {:?}",
        debug_query::<Pg, _>(&query).to_string()
    );

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
#[cfg(feature = "mysql")]
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
#[cfg(feature = "postgres")]
pub async fn generic_find_all<T, P, R>(storage: &Storage, predicate: P) -> StorageResult<Vec<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, PgConnection, R> + QueryFragment<Pg> + Send + 'static,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err) => Err(MeshError::Others),
    }?;
    generic_filter::<T, _, _>(&conn, predicate).await
}
#[cfg(feature = "mysql")]
async fn generic_filter<T, P, R>(conn: &MysqlPoolConn, predicate: P) -> StorageResult<Vec<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let query = T::table().filter(predicate);
    logger::info!(
        action = "generic_filter",
        "Debug Query {:?}",
        debug_query::<Mysql, _>(&query).to_string()
    );

    track_database_call::<T, _, _>(query.get_results_async(conn), DatabaseOperation::Filter)
        .await
        .map_err(|err| match err {
            DieselError::NotFound => MeshError::NotFound,
            _ => MeshError::Others,
        })
}
#[cfg(feature = "postgres")]
async fn generic_filter<T, P, R>(conn: &PgPoolConn, predicate: P) -> StorageResult<Vec<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, PgConnection, R> + QueryFragment<Pg> + Send + 'static,
    R: Send + 'static,
{
    let query = T::table().filter(predicate);
    logger::debug!(query = %debug_query::<Pg, _>(&query).to_string());

    track_database_call::<T, _, _>(query.get_results_async(conn), DatabaseOperation::Filter)
        .await
        .map_err(|err| match err {
            DieselError::NotFound => MeshError::NotFound,
            _ => MeshError::Others,
        })
}
#[cfg(feature = "mysql")]
async fn generic_find_one_core<T, P, R>(conn: &MysqlPoolConn, predicate: P) -> StorageResult<R>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, MysqlConnection, R> + QueryFragment<Mysql> + Send + 'static,
    R: Send + 'static,
{
    let query = <T as HasTable>::table().filter(predicate);
    logger::debug!(
        action = "generic_find_one_core",
        "Debug Query {:?}",
        debug_query::<Mysql, _>(&query).to_string()
    );

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
#[cfg(feature = "postgres")]
async fn generic_find_one_core<T, P, R>(conn: &PgPoolConn, predicate: P) -> StorageResult<R>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, PgConnection, R> + QueryFragment<Pg> + Send + 'static,
    R: Send + 'static,
{
    let query = <T as HasTable>::table().filter(predicate);
    logger::debug!(query = %debug_query::<Pg, _>(&query).to_string());

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
#[cfg(feature = "mysql")]
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
#[cfg(feature = "postgres")]
pub async fn generic_find_one<T, P, R>(storage: &Storage, predicate: P) -> StorageResult<R>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, PgConnection, R> + QueryFragment<Pg> + Send + 'static,
    R: Send + 'static,
{
    let conn = match storage.get_conn().await {
        Ok(conn) => Ok(conn),
        Err(err) => Err(MeshError::Others),
    }?;
    generic_find_one_core::<T, _, _>(&conn, predicate).await
}
#[cfg(feature = "mysql")]
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
#[cfg(feature = "postgres")]
pub async fn generic_find_one_optional<T, P, R>(
    storage: &Storage,
    predicate: P,
) -> StorageResult<Option<R>>
where
    T: FilterDsl<P> + HasTable<Table = T> + Table + 'static,
    Filter<T, P>: LoadQuery<'static, PgConnection, R> + QueryFragment<Pg> + Send + 'static,
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
#[cfg(feature = "mysql")]
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
#[cfg(feature = "postgres")]
pub async fn generic_find_by_id_optional<T, Pk, R>(
    storage: &Storage,
    id: Pk,
) -> StorageResult<Option<R>>
where
    T: FindDsl<Pk> + HasTable<Table = T> + LimitDsl + Table + 'static,
    <T as HasTable>::Table: FindDsl<Pk>,
    Find<T, Pk>: LimitDsl + QueryFragment<Pg> + RunQueryDsl<PgConnection> + Send + 'static,
    Limit<Find<T, Pk>>: LoadQuery<'static, PgConnection, R>,
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

#[cfg(feature = "mysql")]
async fn generic_find_by_id_core<T, Pk, R>(conn: &MysqlPoolConn, id: Pk) -> StorageResult<R>
where
    T: FindDsl<Pk> + HasTable<Table = T> + LimitDsl + Table + 'static,
    Find<T, Pk>: LimitDsl + QueryFragment<Mysql> + RunQueryDsl<MysqlConnection> + Send + 'static,
    Limit<Find<T, Pk>>: LoadQuery<'static, MysqlConnection, R>,
    Pk: Clone + Debug,
    R: Send + 'static,
{
    let query = <T as HasTable>::table().find(id.to_owned());
    logger::debug!(
        action = "generic_find_by_id_core",
        "Debug Query {:?}",
        debug_query::<Mysql, _>(&query).to_string()
    );

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

#[cfg(feature = "postgres")]
async fn generic_find_by_id_core<T, Pk, R>(conn: &PgPoolConn, id: Pk) -> StorageResult<R>
where
    T: FindDsl<Pk> + HasTable<Table = T> + LimitDsl + Table + 'static,
    Find<T, Pk>: LimitDsl + QueryFragment<Pg> + RunQueryDsl<PgConnection> + Send + 'static,
    Limit<Find<T, Pk>>: LoadQuery<'static, PgConnection, R>,
    Pk: Clone + Debug,
    R: Send + 'static,
{
    let query = <T as HasTable>::table().find(id.to_owned());
    logger::debug!(
        action = "generic_find_by_id_core",
        "Debug Query {:?}",
        debug_query::<Pg, _>(&query).to_string()
    );

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
