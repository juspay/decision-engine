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
    types::{Expiration, FromRedis, MultipleValues},
};
use redis_interface::{errors, types::DelReply, RedisConnectionPool};
use std::fmt::Debug;

use crate::config::GlobalConfig;
use crate::redis::feature;
use crate::redis::feature::{RedisCompressionConfig, RedisDataStruct};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Cursor, Read};
use std::str;
#[cfg(feature = "zstd")]
use zstd::dict::DecoderDictionary;
#[cfg(feature = "zstd")]
use zstd::stream::encode_all;
#[cfg(feature = "zstd")]
use zstd::stream::read::Decoder;

// use crate::redis::cache::{findByNameFromRedis};
// use crate::decider::gatewaydecider::constants as C;
// use std::collections::HashMap;
// use serde::Serialize;
// use serde::Deserialize;
pub struct RedisConnectionWrapper {
    pub conn: RedisConnectionPool,
    pub config: GlobalConfig,
}

impl RedisConnectionWrapper {
    #[cfg(feature = "zstd")]
    pub async fn setx_with_compression<V>(
        &self,
        key: &str,
        value: V,
        ttl: Option<i64>,
        option: Option<SetOptions>,
        redis_compression_config: Option<HashMap<String, RedisCompressionConfig>>,
        redis_type: RedisDataStruct,
    ) -> Result<bool, errors::RedisError>
    where
        V: serde::Serialize + Debug,
    {
        let json = serde_json::to_vec(&value).map_err(|_| errors::RedisError::SetHashFailed)?;

        // redisCompresionEligibleLength equivalent - default 200, configurable via env
        let redis_compression_eligible_length = env::var("REDIS_COMPRESSION_ELIGIBLE_LENGTH")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(200);

        // Convert RedisDataStruct enum to string key for HashMap lookup
        let redis_type_key = redis_type.as_str();

        // Replicate: mbCompConf <- (HM.lookup redisType =<<) <$> getOptionLocalIO (R._optionsLocal flowRt) RedisCompressionConf
        let final_value = match redis_compression_config
            .as_ref()
            .and_then(|config| config.get(redis_type_key))
        {
            Some(comp_conf) => {
                // case mbCompConf of Just compConf -> if length v > redisCompresionEligibleLength && compConf.compEnabled
                if json.len() > redis_compression_eligible_length && comp_conf.compEnabled {
                    // compress' compConf
                    self.compress_with_dict(&json, comp_conf)
                } else {
                    // pure v (return uncompressed)
                    json
                }
            }
            None => {
                // Nothing -> pure v (return uncompressed)
                json
            }
        };

        let result = self
            .conn
            .pool
            .set(key, final_value, ttl.map(Expiration::EX), option, false)
            .await
            .change_context(errors::RedisError::SetHashFailed)?;
        Ok(result)
    }

    #[cfg(feature = "zstd")]
    fn compress_with_dict(&self, json: &[u8], comp_conf: &RedisCompressionConfig) -> Vec<u8> {
        // mbDictConf <- (HM.lookup compConf.dictId =<<) <$> getOptionLocalIO (R._options flowRt) RedisZstdDictConf
        let dict_file_path = format!(
            "{}/{}.dict",
            self.config
                .compression_filepath
                .zstd_compression_filepath
                .clone(),
            comp_conf.dictId
        );

        match File::open(&dict_file_path) {
            Ok(mut dict_file) => {
                let mut dict_bytes = Vec::new();
                if dict_file.read_to_end(&mut dict_bytes).is_ok() {
                    // case mbDictConf of Just d -> pure $ ZSTD.compressUsingDict d (fromMaybe 3 compConf.compLevel) v
                    let compression_level = comp_conf
                        .compLevel
                        .as_ref()
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(3); // fromMaybe 3 compConf.compLevel

                    match encode_all(Cursor::new(json), compression_level) {
                        Ok(compressed) => compressed,
                        Err(e) => {
                            logger::error!("REDIS_ZSTD_COMPRESS - REDIS_COMPRESSION - Compression failed: {:?}", e);
                            json.to_vec()
                        }
                    }
                } else {
                    logger::error!("REDIS_ZSTD_COMPRESS - REDIS_COMPRESSION - Failed to read dictionary file for dictId: {}", comp_conf.dictId);
                    json.to_vec()
                }
            }
            Err(_) => {
                // Nothing -> logHelper ... "Dict not found while compressing redis value for dictId: " <> compConf.dictId
                logger::error!("REDIS_ZSTD_COMPRESS - REDIS_COMPRESSION - Dict not found while compressing redis value for dictId: {}", comp_conf.dictId);
                json.to_vec() // pure v (return uncompressed)
            }
        }
    }

    pub fn new(redis_conn: RedisConnectionPool, config: GlobalConfig) -> Self {
        Self {
            conn: redis_conn,
            config,
        }
    }

    #[cfg(not(feature = "zstd"))]
    pub async fn set_key<V>(
        &self,
        key: &str,
        value: V,
        redis_compression_config: Option<HashMap<String, RedisCompressionConfig>>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError>
    where
        V: serde::Serialize + Debug,
    {
        self.conn.serialize_and_set_key(key, value).await
    }

    #[cfg(feature = "zstd")]
    pub async fn set_key<V>(
        &self,
        key: &str,
        value: V,
        redis_compression_config: Option<HashMap<String, RedisCompressionConfig>>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError>
    where
        V: serde::Serialize + Debug,
    {
        self.setx_with_compression(key, value, None, None, redis_compression_config, redis_type)
            .await
            .map(|_| ())
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

    fn is_zstd_compressed(data: &[u8]) -> bool {
        data.len() >= 4 && &data[..4] == [0x28, 0xB5, 0x2F, 0xFD]
    }

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

    fn decode_dict_id(bytes: &[u8]) -> String {
        if bytes.len() == 1 {
            let b = bytes[0];

            // Haskell treat ASCII printable bytes as characters
            if b.is_ascii_graphic() || b == b' ' {
                return (b as char).to_string();
            }

            // Otherwise: return decimal (NOT hex)
            return format!("{}", b);
        }

        // multi-byte: return decimal big-endian number (Haskell behavior)
        let mut val: u64 = 0;
        for &b in bytes {
            val = (val << 8) | (b as u64);
        }
        format!("{}", val)
    }

    #[cfg(feature = "zstd")]
    pub async fn get_key<T>(
        &self,
        key: &str,
        type_name: &'static str,
    ) -> Result<T, errors::RedisError>
    where
        T: DeserializeOwned,
    {
        let raw_bytes: Vec<u8> = self.conn.get_key(key).await?;
        logger::error!(
            "REDIS_ZSTD_DECOMPRESS - REDIS_COMPRESSION - Retrieved raw bytes length: {}",
            raw_bytes.len()
        );

        if Self::is_zstd_compressed(&raw_bytes) {
            let dict_id = Self::extract_dict_id_from_cdata(&raw_bytes)
                .ok_or(errors::RedisError::OnMessageError)?;

            logger::error!(
                "REDIS_ZSTD_DECOMPRESS - REDIS_COMPRESSION - Extracted dict_id: {}",
                dict_id
            );

            let dict_file_path = format!(
                "{}/{}.dict",
                self.config
                    .compression_filepath
                    .zstd_compression_filepath
                    .clone(),
                dict_id
            );

            let mut dict_file =
                File::open(dict_file_path).map_err(|_| errors::RedisError::UnknownResult)?;

            logger::error!("Redis get failed");

            let mut dict_bytes = Vec::new();
            dict_file
                .read_to_end(&mut dict_bytes)
                .map_err(|_| errors::RedisError::GetFailed)?;
            let _dict = DecoderDictionary::copy(&dict_bytes);

            let mut decoder = Decoder::with_dictionary(Cursor::new(&raw_bytes), &dict_bytes)
                .map_err(|_| errors::RedisError::GetFailed)?;
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|_| errors::RedisError::GetFailed)?;

            let value: T =
                serde_json::from_slice(&decompressed).map_err(|_| errors::RedisError::GetFailed)?;
            Ok(value)
        } else {
            let value: T =
                serde_json::from_slice(&raw_bytes).change_context(errors::RedisError::GetFailed)?;
            Ok(value)
        }
    }

    #[cfg(not(feature = "zstd"))]
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

    #[cfg(feature = "zstd")]
    pub async fn get_key_string(&self, key: &str) -> Result<String, errors::RedisError> {
        self.get_key::<String>(key, "").await
    }

    #[cfg(not(feature = "zstd"))]
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

    #[cfg(not(feature = "zstd"))]
    pub async fn setXWithOption(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        option: SetOptions,
        redis_compression_config: Option<HashMap<String, RedisCompressionConfig>>,
        redis_type: RedisDataStruct,
    ) -> Result<bool, errors::RedisError> {
        // implement the redis query to set if it doesn't exist
        self.conn
            .pool
            .set(key, value, Some(Expiration::EX(ttl)), Some(option), false)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }
    #[cfg(feature = "zstd")]
    pub async fn setXWithOption(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        option: SetOptions,
        redis_compression_config: Option<HashMap<String, RedisCompressionConfig>>,
        redis_type: RedisDataStruct,
    ) -> Result<bool, errors::RedisError> {
        self.setx_with_compression(
            key,
            value,
            Some(ttl),
            Some(option),
            redis_compression_config,
            redis_type,
        )
        .await
    }
    pub async fn exists(&self, key: &str) -> Result<bool, errors::RedisError> {
        self.conn
            .pool
            .exists(key)
            .await
            .change_context(errors::RedisError::GetFailed)
    }
    #[cfg(not(feature = "zstd"))]
    pub async fn setx(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        redis_compression_config: Option<HashMap<String, RedisCompressionConfig>>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError> {
        self.conn
            .pool
            .set(key, value, Some(Expiration::EX(ttl)), None, false)
            .await
            .change_context(errors::RedisError::SetHashFailed)
    }
    #[cfg(feature = "zstd")]
    pub async fn setx(
        &self,
        key: &str,
        value: &str,
        ttl: i64,
        redis_compression_config: Option<HashMap<String, RedisCompressionConfig>>,
        redis_type: RedisDataStruct,
    ) -> Result<(), errors::RedisError> {
        self.setx_with_compression(
            key,
            value,
            Some(ttl),
            None,
            redis_compression_config,
            redis_type,
        )
        .await
        .map(|_| ())
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
