use std::future::Future;
use std::pin::Pin;

use crate::logger;
use error_stack::Result;
use error_stack::ResultExt;
use fred::prelude::RedisKey;
use fred::types::SetOptions;
use fred::{
    clients::Transaction,
    interfaces::{KeysInterface, ListInterface, TransactionInterface},
    types::{Expiration, FromRedis, MultipleValues, RedisValue},
};
use redis_interface::{errors, types::DelReply, RedisConnectionPool};
use std::fmt::Debug;

use crate::config::CompressionFilepath;
use crate::config::GlobalConfig;
use crate::redis::feature;
use crate::redis::feature::{
    RedisCompressionConfig, RedisCompressionConfigCombined, RedisDataStruct,
};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Cursor, Read};
use std::str;
#[cfg(feature = "redis_compression")]
use zstd::{
    bulk::Compressor,
    dict::{DecoderDictionary, EncoderDictionary},
    stream::{read::Decoder, write::Encoder},
};

pub struct RedisConnectionWrapper {
    pub conn: RedisConnectionPool,
    pub compression_file_path: Option<CompressionFilepath>,
}

const ZSTD_MAGIC_BYTES: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];

impl RedisConnectionWrapper {
    pub fn new(
        redis_conn: RedisConnectionPool,
        compression_file_path: Option<CompressionFilepath>,
    ) -> Self {
        Self {
            conn: redis_conn,
            compression_file_path,
        }
    }

    #[cfg(feature = "redis_compression")]
    fn compress_string_with_config(
        &self,
        value: &str,
        key: &str,
        redis_compression_config: Option<&RedisCompressionConfigCombined>,
        redis_type: RedisDataStruct,
    ) -> Vec<u8> {
        logger::debug!(
            "REDIS_ZSTD_COMPRESS - compress_string_with_config called with key: {}, value length: {}",
            key,
            value.len()
        );

        let json = value.as_bytes().to_vec();

        let redis_compression_eligible_length = env::var("REDIS_COMPRESSION_ELIGIBLE_LENGTH")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(200);

        let redis_type_key = redis_type.as_str();

        logger::debug!(
            "REDIS_ZSTD_COMPRESS - Redis Type Key: {}, JSON Length: {}, redis_compression_config: {:?}, key: {} , eligible_length: {}",
            redis_type_key,
            json.len(),
            redis_compression_config,
            key,
            redis_compression_eligible_length
        );

        let final_value = match redis_compression_config {
            Some(config_combined) if config_combined.isRedisCompEnabled => {
                match config_combined
                    .redisCompressionConfig
                    .as_ref()
                    .and_then(|config| config.get(redis_type_key))
                {
                    Some(comp_conf)
                        if json.len() > redis_compression_eligible_length
                            && comp_conf.compEnabled =>
                    {
                        logger::debug!(
                            "REDIS_ZSTD_COMPRESS - Compressing data for key: {}, dictId: {}",
                            key,
                            comp_conf.dictId
                        );
                        self.compress_with_dict(&json, comp_conf, key)
                    }
                    _ => {
                        logger::debug!(
                            "REDIS_ZSTD_COMPRESS - Skipping compression for key: {} (length or compEnabled check failed)",
                            key
                        );
                        json
                    }
                }
            }
            _ => {
                logger::debug!(
                    "REDIS_ZSTD_COMPRESS - Skipping compression for key: {} (redis compression not enabled or config not present)",
                    key
                );
                json
            }
        };

        logger::debug!(
            "REDIS_ZSTD_COMPRESS - Compressed/processed key: {}, final value length: {}, is_compressed: {}",
            key,
            final_value.len(),
            Self::is_zstd_compressed(&final_value)
        );

        final_value
    }

    #[cfg(feature = "redis_compression")]
    fn compress_with_dict(
        &self,
        json: &[u8],
        comp_conf: &RedisCompressionConfig,
        key: &str,
    ) -> Vec<u8> {
        let dict_file_path = match &self.compression_file_path {
            Some(compression_config) => format!(
                "{}/{}.dict",
                compression_config.zstd_compression_filepath, comp_conf.dictId
            ),
            None => {
                logger::debug!(
                    "REDIS_ZSTD_COMPRESS - Compression filepath not configured for key: {}",
                    key
                );
                return json.to_vec();
            }
        };

        match std::fs::read(&dict_file_path) {
            Ok(dict_bytes) => {
                let compression_level = comp_conf
                    .compLevel
                    .as_ref()
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(3);

                match Compressor::with_dictionary(compression_level, &dict_bytes) {
                    Ok(mut compressor) => match compressor.compress(json) {
                        Ok(compressed) => {
                            logger::debug!(
                                    "REDIS_ZSTD_COMPRESS - Successfully compressed data for key: {}, original length: {}, compressed length: {}",
                                    key,
                                    json.len(),
                                    compressed.len()
                                );
                            return compressed;
                        }
                        Err(e) => {
                            logger::error!("Compression failed for key {}: {:?}", key, e);
                        }
                    },
                    Err(e) => {
                        logger::error!("Failed to create compressor for key {}: {:?}", key, e);
                    }
                }
            }
            Err(e) => {
                logger::error!(
                    "Failed to read dictionary file {} for key {}: {:?}",
                    dict_file_path,
                    key,
                    e
                );
            }
        }

        json.to_vec()
    }

    #[cfg(not(feature = "redis_compression"))]
    pub async fn set_key<V>(
        &self,
        key: &str,
        value: V,
        redis_compression_config: Option<RedisCompressionConfigCombined>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError>
    where
        V: serde::Serialize + Debug,
    {
        self.conn.serialize_and_set_key(key, value).await
    }

    #[cfg(feature = "redis_compression")]
    pub async fn set_key(
        &self,
        key: &str,
        value: &str,
        redis_compression_config: Option<RedisCompressionConfigCombined>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError> {
        let final_value = self.compress_string_with_config(
            value,
            key,
            redis_compression_config.as_ref(),
            redis_type,
        );

        // Convert Vec<u8> to RedisValue::Bytes for proper serialization
        let redis_value = RedisValue::Bytes(final_value.into());

        self.conn
            .pool
            .set(key, redis_value, None, None, false)
            .await
            .change_context(errors::RedisError::SetHashFailed)?;

        logger::debug!(
            "REDIS_ZSTD_COMPRESS - REDIS_COMPRESSION - Successfully set key: {} (string type)",
            key
        );

        Ok(())
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

    #[cfg(feature = "redis_compression")]
    fn is_zstd_compressed(data: &[u8]) -> bool {
        data.len() >= 4 && &data[..4] == [0x28, 0xB5, 0x2F, 0xFD]
    }
    #[cfg(feature = "redis_compression")]
    fn extract_dict_id_from_cdata(cdata: &[u8]) -> Option<String> {
        // Haskell magic: "(\181/\253"
        let magic_number: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

        if cdata.len() > 9 && &cdata[0..4] == magic_number {
            let cdata_wo_magic = &cdata[4..];

            let frame_header_w = cdata_wo_magic[0];

            // getDictionaryLengthFromDictID (bit1, bit0)
            let bit1 = ((frame_header_w >> 1) & 1) == 1;
            let bit0 = (frame_header_w & 1) == 1;

            let dict_id_size = match (bit1, bit0) {
                (false, false) => 1,
                (false, true) => 1,
                (true, false) => 2,
                (true, true) => 4,
            };

            // singleSegmentFlag = bit 5
            let single_segment_flag = ((frame_header_w >> 5) & 1) == 1;

            let start_offset = if single_segment_flag { 1 } else { 2 };

            if cdata_wo_magic.len() < start_offset + dict_id_size {
                return None;
            }

            let dict_id_bytes = &cdata_wo_magic[start_offset..start_offset + dict_id_size];

            // decodeDictId → convert bytes to hex string
            let decoded = Self::decode_dict_id(dict_id_bytes);

            Some(decoded)
        } else {
            None
        }
    }

    #[cfg(feature = "redis_compression")]
    fn decode_dict_id(bytes: &[u8]) -> String {
        let val = match bytes.len() {
            1 => bytes[0] as u32,
            2 => {
                let s8 = bytes[0] as u32;
                let f8 = bytes[1] as u32;
                (f8 << 8) | s8
            }
            3 => {
                let a8 = bytes[0] as u32;
                let b8 = bytes[1] as u32;
                let c8 = bytes[2] as u32;
                (c8 << 16) | (b8 << 8) | a8
            }
            4 => {
                let a8 = bytes[0] as u32;
                let b8 = bytes[1] as u32;
                let c8 = bytes[2] as u32;
                let d8 = bytes[3] as u32;
                ((d8 << 8) | c8) << 16 | (b8 << 8) | a8
            }
            _ => {
                logger::error!("Unexpected dict_id size: {}", bytes.len());
                return "0".to_string();
            }
        };

        val.to_string()
    }

    #[cfg(feature = "redis_compression")]
    pub async fn get_key<T>(
        &self,
        key: &str,
        type_name: &'static str,
    ) -> Result<T, errors::RedisError>
    where
        T: DeserializeOwned,
    {
        let raw_bytes: Vec<u8> = self.conn.get_key(key).await?;

        if raw_bytes.is_empty() {
            return Err(errors::RedisError::GetFailed.into());
        }

        match Self::extract_dict_id_from_cdata(&raw_bytes) {
            Some(dict_id) => {
                let dict_file_path = match &self.compression_file_path {
                    Some(compression_config) => format!(
                        "{}/{}.dict",
                        compression_config.zstd_compression_filepath, dict_id
                    ),
                    None => {
                        logger::debug!(
                            "REDIS_ZSTD_COMPRESS - Compression filepath not configured for key: {}",
                            key
                        );
                        return serde_json::from_slice(&raw_bytes)
                            .change_context(errors::RedisError::GetFailed);
                    }
                };

                let mut dict_file = File::open(&dict_file_path).map_err(|e| {
                    logger::error!(
                        "Failed to open dictionary file {} for key {}: {:?}",
                        dict_file_path,
                        key,
                        e
                    );
                    errors::RedisError::UnknownResult
                })?;

                let mut dict_bytes = Vec::new();
                dict_file.read_to_end(&mut dict_bytes).map_err(|e| {
                    logger::error!("Failed to read dictionary file for key {}: {:?}", key, e);
                    errors::RedisError::GetFailed
                })?;

                let mut decoder = Decoder::with_dictionary(Cursor::new(&raw_bytes), &dict_bytes)
                    .map_err(|e| {
                        logger::error!("Failed to create ZSTD decoder for key {}: {:?}", key, e);
                        errors::RedisError::GetFailed
                    })?;

                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed).map_err(|e| {
                    logger::error!("Failed to decompress data for key {}: {:?}", key, e);
                    errors::RedisError::GetFailed
                })?;

                serde_json::from_slice(&decompressed).change_context(errors::RedisError::GetFailed)
            }
            None => {
                serde_json::from_slice(&raw_bytes).change_context(errors::RedisError::GetFailed)
            }
        }
    }

    #[cfg(not(feature = "redis_compression"))]
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

    #[cfg(feature = "redis_compression")]
    pub async fn get_key_string(&self, key: &str) -> Result<String, errors::RedisError> {
        self.get_key::<String>(key, "").await
    }

    #[cfg(not(feature = "redis_compression"))]
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

    #[cfg(not(feature = "redis_compression"))]
    pub async fn setXWithOption(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        option: SetOptions,
        redis_compression_config: Option<RedisCompressionConfigCombined>,
        redis_type: RedisDataStruct,
    ) -> Result<bool, errors::RedisError> {
        // implement the redis query to set if it doesn't exist
        self.conn
            .pool
            .set(key, value, Some(Expiration::EX(ttl)), Some(option), false)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }
    #[cfg(feature = "redis_compression")]
    pub async fn setXWithOption(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        option: SetOptions,
        redis_compression_config: Option<RedisCompressionConfigCombined>,
        redis_type: RedisDataStruct,
    ) -> Result<bool, errors::RedisError> {
        let final_value = self.compress_string_with_config(
            value,
            key,
            redis_compression_config.as_ref(),
            redis_type,
        );

        // Convert Vec<u8> to RedisValue::Bytes for proper serialization
        let redis_value = RedisValue::Bytes(final_value.into());

        let result = self
            .conn
            .pool
            .set(
                key,
                redis_value,
                Some(Expiration::EX(ttl)),
                Some(option),
                false,
            )
            .await
            .change_context(errors::RedisError::SetHashFailed)?;

        logger::debug!(
            "REDIS_ZSTD_COMPRESS - REDIS_COMPRESSION - Successfully set key: {}, result: {}",
            key,
            result
        );

        Ok(result)
    }
    pub async fn exists(&self, key: &str) -> Result<bool, errors::RedisError> {
        self.conn
            .pool
            .exists(key)
            .await
            .change_context(errors::RedisError::GetFailed)
    }
    #[cfg(not(feature = "redis_compression"))]
    pub async fn setx(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        redis_compression_config: Option<RedisCompressionConfigCombined>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError> {
        self.conn
            .pool
            .set(key, value, Some(Expiration::EX(ttl)), None, false)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }
    #[cfg(feature = "redis_compression")]
    pub async fn setx(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        redis_compression_config: Option<RedisCompressionConfigCombined>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError> {
        let final_value = self.compress_string_with_config(
            value,
            key,
            redis_compression_config.as_ref(),
            redis_type,
        );

        // Convert Vec<u8> to RedisValue::Bytes for proper serialization
        let redis_value = RedisValue::Bytes(final_value.into());

        self.conn
            .pool
            .set(key, redis_value, Some(Expiration::EX(ttl)), None, false)
            .await
            .change_context(errors::RedisError::SetHashFailed)?;

        logger::debug!(
            "REDIS_ZSTD_COMPRESS - REDIS_COMPRESSION - Successfully set key: {}",
            key
        );

        Ok(())
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
}
