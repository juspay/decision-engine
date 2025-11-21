use std::future::Future;
use std::pin::Pin;

use error_stack::Result;
use error_stack::ResultExt;
use fred::prelude::RedisKey;
use fred::types::SetOptions;
use fred::{
    clients::Transaction,
    interfaces::{KeysInterface, ListInterface, TransactionInterface},
    types::{Expiration, FromRedis, MultipleValues},
};
use redis_interface::{errors, types::DelReply, RedisConnectionPool};
use std::fmt::Debug;

pub struct RedisConnectionWrapper {
    pub conn: RedisConnectionPool,
}

impl RedisConnectionWrapper {
    pub fn new(redis_conn: RedisConnectionPool) -> Self {
        Self { conn: redis_conn }
    }

    pub async fn set_key<V>(&self, key: &str, value: V) -> Result<(), errors::RedisError>
    where
        V: serde::Serialize + Debug,
    {
        self.conn.serialize_and_set_key(key, value).await
    }

    pub async fn set_key_with_ttl<V>(
        &self,
        key: &str,
        value: V,
        ttl: i64,
    ) -> Result<(), errors::RedisError>
    where
        V: serde::Serialize + Debug,
    {
        self.conn
            .serialize_and_set_key_with_expiry(key, value, ttl)
            .await
    }

    pub async fn get_key<T>(
        &self,
        key: &str,
        type_name: &'static str,
    ) -> Result<T, errors::RedisError>
    where
        T: serde::de::DeserializeOwned,
    {
        self.conn.get_and_deserialize_key(key, type_name).await
    }

    pub async fn get_key_string(&self, key: &str) -> Result<String, errors::RedisError> {
        self.conn
            .get_key(key)
            .await
            .change_context(errors::RedisError::GetFailed)
    }

    pub async fn get_list_length(&self, key: &str) -> Result<usize, errors::RedisError> {
        self.conn
            .pool
            .llen(key)
            .await
            .change_context(errors::RedisError::GetListLengthFailed)
    }

    pub async fn append_to_list_start<V>(
        &self,
        key: &RedisKey,
        elements: V,
    ) -> Result<(), errors::RedisError>
    where
        V: TryInto<MultipleValues> + Debug + Send,
        V::Error: Into<fred::error::RedisError> + Send,
    {
        self.conn
            .pool
            .lpush(key, elements)
            .await
            .change_context(errors::RedisError::AppendElementsToListFailed)
    }

    pub async fn remove_from_list_end(
        &self,
        key: &str,
        count: Option<usize>,
    ) -> Result<Vec<String>, errors::RedisError> {
        self.conn
            .pool
            .rpop(key, count)
            .await
            .change_context(errors::RedisError::PopListElementsFailed)
    }

    pub async fn get_range_from_list(
        &self,
        key: &str,
        start: i64,
        end: i64,
    ) -> Result<Vec<String>, errors::RedisError> {
        self.conn
            .pool
            .lrange(key, start, end)
            .await
            .change_context(errors::RedisError::GetFailed)
    }

    pub async fn delete_key(&self, key: &str) -> Result<DelReply, errors::RedisError> {
        self.conn
            .pool
            .del(key)
            .await
            .change_context(errors::RedisError::DeleteFailed)
    }

    pub async fn increment_key(&self, key: &str) -> Result<i64, errors::RedisError> {
        self.conn
            .pool
            .incr(key)
            .await
            .change_context(errors::RedisError::IncrementHashFieldFailed)
    }

    pub async fn decrement_key(&self, key: &str) -> Result<i64, errors::RedisError> {
        self.conn
            .pool
            .decr(key)
            .await
            .change_context(errors::RedisError::IncrementHashFieldFailed)
    }

    pub async fn expire_key(&self, key: &str, ttl: i64) -> Result<(), errors::RedisError> {
        self.conn
            .pool
            .expire(key, ttl)
            .await
            .change_context(errors::RedisError::IncrementHashFieldFailed)
    }

    pub async fn setXWithOption(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        option: SetOptions,
    ) -> Result<bool, errors::RedisError> {
        // implement the redis query to set if it doesn't exist
        self.conn
            .pool
            .set(key, value, Some(Expiration::EX(ttl)), Some(option), false)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }
    pub async fn exists(&self, key: &str) -> Result<bool, errors::RedisError> {
        self.conn
            .pool
            .exists(key)
            .await
            .change_context(errors::RedisError::GetFailed)
    }
    pub async fn setx(&self, key: &str, value: &str, ttl: i64) -> Result<(), errors::RedisError> {
        self.conn
            .pool
            .set(key, value, Some(Expiration::EX(ttl)), None, false)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }
    pub async fn multi<R, F>(&self, abort_on_error: bool, f: F) -> Result<R, errors::RedisError>
    where
        R: FromRedis,
        F: for<'a> FnOnce(
            &'a Transaction,
        ) -> Pin<
            Box<dyn Future<Output = Result<(), fred::error::RedisError>> + Send + 'a>,
        >,
    {
        let trx = self.conn.pool.next().multi();
        f(&trx)
            .await
            .change_context(errors::RedisError::UnknownResult)?;
        trx.exec::<R>(abort_on_error)
            .await
            .change_context(errors::RedisError::UnknownResult)
    }

    /// Add entry to Redis stream with MAXLEN using raw command
    /// Example: XADD shard_stream_0 MAXLEN 1000 * key value
    pub async fn xadd_with_maxlen(
        &self,
        stream_key: &str,
        maxlen: u64,
        fields: Vec<String>,
    ) -> Result<String, errors::RedisError> {
        use fred::interfaces::ClientLike;
        use fred::types::CustomCommand;

        // Build raw Redis command: XADD stream_key MAXLEN maxlen * field1 value1 field2 value2 ...
        let mut args = vec![stream_key.to_string(), "MAXLEN".to_string(), maxlen.to_string(), "*".to_string()];
        args.extend(fields);
        
        self.conn
            .pool
            .custom(CustomCommand::new("XADD", stream_key, false), args)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }

    /// Add entry to Redis stream with approximate MAXLEN (more efficient)
    /// Example: XADD shard_stream_0 MAXLEN ~ 1000 * key value
    pub async fn xadd_with_approximate_maxlen(
        &self,
        stream_key: &str,
        maxlen: u64,
        fields: Vec<String>,
    ) -> Result<String, errors::RedisError> {
        use fred::interfaces::ClientLike;
        use fred::types::CustomCommand;

        // Build raw Redis command: XADD stream_key MAXLEN ~ maxlen * field1 value1 field2 value2 ...
        let mut args = vec![stream_key.to_string(), "MAXLEN".to_string(), "~".to_string(), maxlen.to_string(), "*".to_string()];
        args.extend(fields);
        
        self.conn
            .pool
            .custom(CustomCommand::new("XADD", stream_key, false), args)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }

    /// Read entries from Redis stream using XRANGE
    /// Example: XRANGE shard_stream_0 1-0+ +
    pub async fn xrange(
        &self,
        stream_key: &str,
        start: &str,
        end: &str,
        count: Option<u64>,
    ) -> Result<Vec<(String, Vec<(String, String)>)>, errors::RedisError> {
        use fred::interfaces::StreamsInterface;

        self.conn
            .pool
            .xrange(stream_key, start, end, count)
            .await
            .change_context(errors::RedisError::GetFailed)
    }

    /// Get stream length using XLEN
    pub async fn xlen(&self, stream_key: &str) -> Result<u64, errors::RedisError> {
        use fred::interfaces::StreamsInterface;

        let len: u64 = self
            .conn
            .pool
            .xlen(stream_key)
            .await
            .change_context(errors::RedisError::GetFailed)?;

        Ok(len)
    }
}
