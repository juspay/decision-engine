use diesel::connection;
use diesel_async::RunQueryDsl;
use error_stack::ResultExt;
use error_stack::{report, Result};
use fred::prelude::RedisKey;
use fred::{
    interfaces::{HashesInterface, KeysInterface, ListInterface, SetsInterface, StreamsInterface},
    prelude::{LuaInterface, RedisErrorKind},
    types::{
        Expiration, FromRedis, MultipleIDs, MultipleKeys, MultipleOrderedPairs, MultipleStrings,
        MultipleValues, RedisMap, RedisValue, ScanType, Scanner, SetOptions, XCap, XReadResponse,
    },
};
use redis_interface::{types::DelReply, errors, RedisConnectionPool};
use std::fmt::Debug;

pub struct RedisConnectionWrapper {
    pub conn: RedisConnectionPool,
}

impl RedisConnectionWrapper {
    pub fn new(redis_conn: RedisConnectionPool) -> Self {
        Self { conn: redis_conn }
    }

    pub async fn set_key<V>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), errors::RedisError>
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
        self.conn.serialize_and_set_key_with_expiry(key, value, ttl).await
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

    pub async fn delete_key(&self, key: &str) -> Result<DelReply, errors::RedisError> {
        self.conn.pool
            .del(key)
            .await
            .change_context(errors::RedisError::DeleteFailed)
    }
}
