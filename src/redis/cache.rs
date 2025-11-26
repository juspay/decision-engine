use super::mem_cache::GLOBAL_CACHE;
use crate::app;
use crate::logger;
use crate::types::service_configuration;
use crate::utils::StringExt;
use serde::Deserialize;

// Converted type synonyms
// Original Haskell type: KVDBName
pub type KVDBName = String;

async fn get_from_memory_cache(prefixed_key: &str) -> Result<String, String> {
    match GLOBAL_CACHE.get::<String>(prefixed_key) {
        Ok(value) => Ok(value),
        Err(e) => Err(format!("Memory cache get failed: {}", e)),
    }
}

async fn set_to_memory_cache(prefixed_key: &str, value: &str, ttl_seconds: Option<u64>) {
    match GLOBAL_CACHE.store(prefixed_key.to_string(), value.to_string(), ttl_seconds) {
        Ok(_) => {}
        Err(e) => {
            crate::logger::warn!(
                tag = "memory_cache_write_failed",
                action = "memory_cache_write_failed",
                "Failed to write cache for key: {}, error: {:?}",
                prefixed_key,
                e
            );
        }
    }
}

// // Converted data types
// // Original Haskell data type: Multi
// #[derive(Debug, Serialize, Deserialize, PartialEq)]
// pub struct Multi {
//     #[serde(rename = "KVDBName")]
//     pub kvdb_name: String,

//     #[serde(rename = "actions")]
//     pub actions: Vec<fn(ByteString) -> L::KVDBTx<R::Queued<Value>>>,
// }

// impl Multi {
//     pub fn get_kvdb_name(&self) -> &String {
//         &self.kvdb_name
//     }

//     pub fn get_multi_action(&self) -> impl Fn(ByteString) -> L::KVDBTx<R::Queued<Vec<Value>>> {
//         let actions = self.actions.clone();
//         move |prefix| {
//             actions.iter().map(|action| action(prefix.clone())).collect::<L::KVDBTx<R::Queued<Vec<Value>>>>()
//         }
//     }

//     pub fn get_multi_actions(&self) -> &Vec<fn(ByteString) -> L::KVDBTx<R::Queued<Value>>> {
//         &self.actions
//     }
// }

// Converted functions
// Original Haskell function: findByNameFromRedis
pub async fn findByNameFromRedis<A>(key: String) -> Option<A>
where
    A: for<'de> Deserialize<'de>,
{
    findByNameFromRedisHelper(key, Some(extractValue)).await
}

// Original Haskell function: findByNameFromRedisWithDecode
pub async fn findByNameFromRedisWithDecode<A>(
    key: String,
    decode_fn: impl Fn(String) -> Option<A>,
) -> Option<A>
where
    A: for<'de> Deserialize<'de>,
{
    findByNameFromRedisHelper(key, Some(decode_fn)).await
}

pub async fn findByNameFromRedisHelper<A>(
    key: String,
    decode_fn: Option<impl Fn(String) -> Option<A>>,
) -> Option<A>
where
    A: for<'de> Deserialize<'de>,
{
    let global_app_state = app::APP_STATE.get().expect("GlobalAppState not set");
    let prefixed_key = global_app_state.global_config.cache_config.add_prefix(&key);

    match get_from_memory_cache(&prefixed_key).await {
        Ok(cache_value) => {
            logger::debug!(
                tag = "memory_cache",
                action = "hit",
                "Cache hit for key: {}",
                key
            );
            match decode_fn {
                Some(func) => func(cache_value),
                None => extractValue(cache_value),
            }
        }
        Err(_) => {
            logger::debug!(
                tag = "memory_cache",
                action = "miss",
                "Cache miss for key: {}, falling back to database",
                key
            );

            let res = service_configuration::find_config_by_name(key.clone()).await;

            match res {
                Ok(Some(service_config)) => match service_config.value {
                    Some(value) => {
                        // Get TTL from global config
                        let ttl_seconds = Some(
                            global_app_state
                                .global_config
                                .cache_config
                                .service_config_ttl as u64,
                        );
                        set_to_memory_cache(&prefixed_key, &value, ttl_seconds).await;

                        match decode_fn {
                            Some(func) => func(value),
                            None => extractValue(value),
                        }
                    }
                    None => None,
                },
                _ => None,
            }
        }
    }
}

// Function to find value from memory cache/DB and return default if not present
pub async fn findByNameFromRedisWithDefault<A>(key: String, default_value: A) -> A
where
    A: for<'de> Deserialize<'de> + serde::Serialize + Clone,
{
    // First try to get the value using the helper function
    let result = findByNameFromRedisHelper(key.clone(), Some(extractValue::<A>)).await;

    match result {
        Some(value) => value,
        None => {
            // Serialize the default value to JSON string
            match serde_json::to_string(&default_value) {
                Ok(default_json) => {
                    // Cache the default value in memory for future use
                    let global_app_state = app::APP_STATE.get().expect("GlobalAppState not set");
                    let prefixed_key = global_app_state.global_config.cache_config.add_prefix(&key);
                    let ttl_seconds = Some(
                        global_app_state
                            .global_config
                            .cache_config
                            .service_config_ttl as u64,
                    );
                    set_to_memory_cache(&prefixed_key, &default_json, ttl_seconds).await;

                    logger::debug!(
                        tag = "memory_cache",
                        action = "default_cached",
                        "Cached default value for key: {}",
                        key
                    );
                }
                Err(e) => {
                    logger::warn!(
                        tag = "memory_cache",
                        action = "serialize_failed",
                        "Failed to serialize default value for key: {}, error: {:?}",
                        key,
                        e
                    );
                }
            }

            default_value
        }
    }
}

pub fn extractValue<A>(value: String) -> Option<A>
where
    A: for<'de> Deserialize<'de>,
{
    value.parse_struct("generic type").ok()
}

// // Original Haskell function: extractValueFromServiceConfig
// pub fn extractValueFromServiceConfig<T>(
//     service_config: ServiceConfig,
//     should_use_new_value: bool,
//     decode_fn: Option<fn(Text) -> Option<T>>,
// ) -> Option<Option<T>> {
//     match (
//         service_config.new_value_status.as_deref() == Some("STAGGERING") && should_use_new_value,
//         &service_config.new_value,
//     ) {
//         (true, Some(new_val)) => decodeSCValuewithFallBack(&service_config.value, Some(new_val)),
//         _ => {
//             if service_config.value.as_deref() == Some("###") {
//                 None
//             } else {
//                 decodeSCValuewithFallBack(&service_config.previous_value, &service_config.value)
//             }
//         }
//     }
// }

// fn decodeSCValuewithFallBack<T>(
//     fallback_val: &Option<Text>,
//     val: Option<&Text>,
// ) -> Option<Option<T>> {
//     match val {
//         Some(val) => extractValueWithDecode(val).and_then(|v| v.or_else(|| usePreviousValue(fallback_val))),
//         None => usePreviousValue(fallback_val),
//     }
// }

// fn usePreviousValue<T>(fallback_val: &Option<Text>) -> Option<Option<T>> {
// logger::error!(
//     tag = "SERVICE_CONFIG_DECODE_FAIL",
//     "{} : DECODE FAIL FOR VALUE, FALLING BACK TO PREVIOUS_VALUE",
//     service_config.name
// );
//     incrementConfigDecodeFailureCount(&service_config.name);
//     match fallback_val {
//         Some(fallback_val) => extractValueWithDecode(fallback_val),
//         None => None,
//     }
// }

// fn extractValueWithDecode<T>(val: &Text) -> Option<Option<T>> {
//     match decode_fn {
//         Some(fn) => Some(fn(val)),
//         None => extractValue(val),
//     }
// }

// fn extractValue<T>(val: &Text) -> Option<Option<T>> {
//     let e_val: Result<Value, _> = parseValue(val);
//     match e_val {
//         Ok(value) => match fromJSON(value) {
//             Ok(res) => Some(Some(res)),
//             Err(_) => Some(None),
//         },
//         Err(_) => Some(None),
//     }
// }

// pub trait ServiceConfigValue {
//     fn parse_value(proxy: PhantomData<Self>, text: &str) -> Result<Value, String>;
// }

// impl ServiceConfigValue for String {
//     fn parse_value(_: PhantomData<Self>, text: &str) -> Result<Value, String> {
//         Ok(Value::String(text.to_string()))
//     }
// }

// impl ServiceConfigValue for Value {
//     fn parse_value(_: PhantomData<Self>, text: &str) -> Result<Value, String> {
//         from_str::<Value>(text).map_err(|e| e.to_string())
//     }
// }

// // Original Haskell function: cacheValueByField
// pub fn cacheValueByField<Val, Field>(
//     model: Val,
//     field_value: Field,
// ) -> Option<()>
// where
//     Field: BinaryStore,
//     Val: HasCacheKey,
// {
//     let cache_key = mkCacheKeyField(modelCacheKey::<Val>(), Binary::encode(field_value));
//     redisSetex(&cache_key, Binary::encode(model), domainCacheTtl);
//     Some(())
// }

// // Original Haskell function: fetchValueByField
// pub fn fetchValueByField<Val>(
//     key: ByteString,
//     field_value: ByteString,
// ) -> Option<Val>
// where
//     Val: BinaryStore,
// {
//     let cache_key = mkCacheKeyField(&key, &field_value);
//     let result = redisGet(&cache_key);
//     result.and_then(|data| Binary::decode(&data).ok())
// }

// // Original Haskell function: fetchCachedValue
// pub fn fetchCachedValue<Val, Field, M>(field_value: Field)
// where
//     Field: BinaryStore,
//     Val: HasCacheKey,
//     M: MonadFlow,
// {
//     let key = modelCacheKey::<Val>();
//     give(RiskyShowSecrets, || {
//         fetchValueByField::<Val, M>(key, Binary::encode(field_value));
//     });
// }

// // Original Haskell function: mkCacheKeyField
// pub fn mkCacheKeyField(key: ByteString, field_value: ByteString) -> ByteString {
//     domainCachePrefix + &key + ":" + &field_value
// }

// // Original Haskell function: newMulti
// pub fn newMulti(db_name: String) -> Multi {
//     Multi {
//         db_name,
//         ..Default::default()
//     }
// }

// // Original Haskell function: multiThen
// pub fn multiThen<A: ToJSON>(
//     multi: Multi,
//     action: impl Fn(ByteString) -> L::KVDBTx<R::Queued<A>>,
// ) -> Multi {
//     let Multi { name, actions } = multi;
//     Multi {
//         name,
//         actions: {
//             let mut new_actions = actions.clone();
//             new_actions.push(Box::new(move |prefix| {
//                 Box::new(action(prefix).map(|res| A::toJSON(res)))
//             }));
//             new_actions
//         },
//     }
// }

// // Original Haskell function: execMulti
// pub fn execMulti(
//     name: String,
//     opts: Vec<fn(&[u8]) -> L::KVDBTx<R::Queued<Value>>>,
// ) -> Result<Vec<Value>, String> {
//     let result = RC::multiExec(&name, |prefix| {
//         opts.iter()
//             .map(|opt| opt(prefix))
//             .collect::<Result<Vec<_>, _>>()
//     });

//     match result {
//         Err(reply) => Err(format!("{}", reply)),
//         Ok(T::TxSuccess(x)) => Ok(x),
//         Ok(T::TxAborted) => Err("aborted".to_string()),
//         Ok(T::TxError(e)) => Err(e),
//     }
// }

// // Original Haskell function: setExInMulti
// pub fn setExInMulti(
//     key: String,
//     value: String,
//     ttl: i64,
//     multi: Multi,
// ) -> Multi {
//     let action = |prefix| RC.setexTx(textToKey(&key), ttl, TE.encodeUtf8(&value), prefix);
//     multi.multiThen(action)
// }

// // Original Haskell function: incrInMulti
// pub fn incrInMulti(
//     key: String,
//     multi: Multi,
// ) -> Multi {
//     let action = |prefix| RC::incrTx(textToKey(&key), prefix);
//     multi.multiThen(action)
// }

// // Original Haskell function: decrInMulti
// pub fn decrInMulti(
//     key: String,
//     multi: Multi,
// ) -> Multi {
//     let action = |prefix| RC.decrTx(textToKey(&key), prefix);
//     multi.multiThen(action)
// }

// // Original Haskell function: delInMulti
// pub fn delInMulti(
//     key: String,
//     multi: Multi,
// ) -> Multi {
//     let action = |prefix| RC.delTx(vec![textToKey(&key)], prefix);
//     multi.multiThen(action)
// }

// // Original Haskell function: setCacheWithOptsInMulti
// pub fn setCacheWithOptsInMulti(
//     key: String,
//     value: String,
//     ttl_seconds_m: Option<i64>,
//     set_key_opts: T.KVDBSetConditionOption,
//     multi: Multi,
// ) -> Multi {
//     let ttl = ttl_seconds_m.map_or(T::NoTTL, T::Seconds);
//     let action = |prefix| RC.setOptsTx(textToKey(&key), TE.encodeUtf8(&value), ttl, set_key_opts, prefix);
//     multi.multiThen(action)
// }

// // Original Haskell function: keyExistsCache
// pub fn keyExistsCache(
//     db_name: String,
//     key: String,
// ) -> Result<bool, T::KVDBReply> {
//     let result = RC.exists(&db_name, &TE::encodeUtf8(&key));
//     match result {
//         Ok(x) => Ok(x),
//         Err(err) => {
// logger::error!(
//     tag = "Redis exists",
//     "{}",
//     err.to_string()
// );

//             Err(err)
//         }
//     }
// }

// // Original Haskell function: setCacheWithOpts
// pub fn setCacheWithOpts(
//     db_name: String,
//     key: String,
//     value: T::KVDBValue,
//     ttl: T::KVDBSetTTLOption,
//     opts: T::KVDBSetConditionOption,
// ) -> Result<bool, T::KVDBReply> {
//     let result = RC::setOpts(&db_name, &textToKey(&key), value, ttl, opts);
//     match result {
//         Ok(x) => Ok(x),
//         Err(err) => {
// logger::error!(
//     tag = "Redis exists",
//     "{:?}",
//     err
// );
//             Err(err)
//         }
//     }
// }

// // Original Haskell function: incr
// pub fn incr(db_name: String, key: String) -> Result<i32, String> {
//     let result = RC.incr(&db_name, &key.as_bytes());
//     match result {
//         Ok(x) => Ok(x as i32),
//         Err(err) => {
// logger::error!(
//     tag = "Redis incr",
//     "{}",
//     err.to_string()
// );
//             Err(err.to_string())
//         }
//     }
// }

// // Original Haskell function: decr
// pub fn decr(db_name: String, key: String) -> Result<i32, String> {
//     let result = RC::decr(&db_name, key.as_bytes());
//     match result {
//         Ok(x) => Ok(x as i32),
//         Err(err) => {
// logger::error!(
//     tag = "Redis decr",
//     "{:?}",
//     err
// );
//             Err(err.to_string())
//         }
//     }
// }

// // Original Haskell function: getKVDBName
// pub fn getKVDBName(multi: Multi) -> KVDBName {
//     match multi {
//         Multi { name, .. } => name,
//     }
// }

// // Original Haskell function: addToStream
// pub fn addToStream(
//     db_name: String,
//     stream: impl RedisKey,
//     entry_id: T::KVDBStreamEntryIDInput,
//     items: Vec<T::KVDBStreamItem>,
// ) -> Option<T::KVDBStreamEntryID> {
//     let result = RC::xadd(&db_name, &stream, &entry_id, &items);
//     match result {
//         Ok(res) => Some(res),
//         Err(_) => None,
//     }
// }

// // Original Haskell function: getFromStream
// pub fn getFromStream<K: RedisKey>(
//     db_name: String,
//     group_name: T.KVDBGroupName,
//     consumer_name: K,
//     streams_and_ids: Vec<(T.KVDBStream, T.RecordID)>,
//     m_block: Option<i64>,
//     m_count: Option<i64>,
//     noack: bool,
// ) -> Option<Vec<T.KVDBStreamReadResponse>> {
//     match RC.xreadGroup(
//         db_name,
//         group_name,
//         consumer_name,
//         streams_and_ids,
//         m_block,
//         m_count,
//         noack,
//     ) {
//         Ok(res) => Some(res),
//         Err(err) => {
// logger::error!(
//     tag = "getFromStream",
//     "{}",
//     err
// );
//             None
//         }
//     }
// }

// // Original Haskell function: delFromStream
// pub fn delFromStream(
//     db_name: String,
//     stream: impl RedisKey,
//     ids: Vec<T.KVDBStreamEntryID>,
// ) -> i64 {
//     let result = RC::xdel(&db_name, &stream, &ids);
//     match result {
//         Ok(res) => res,
//         Err(err) => {
// logger::error!(
//     tag = "delFromStream",
//     "{}",
//     err
// );
//             0
//         }
//     }
// }

// // Original Haskell function: createStreamGroup
// pub fn createStreamGroup(
//     db_name: String,
//     stream: impl RedisKey,
//     group_name: T.KVDBGroupName,
//     start_id: T.RecordID,
// ) -> bool {
//     let result = RC.xgroupCreate(&db_name, &stream, &group_name, &start_id);
//     match result {
//         Ok(R::Ok) | Ok(R::Pong) => true,
//         Ok(R::Status(status)) => {
// logger::error!(
//     tag = "createStreamGroup",
//     "{}",
//     Text::decode_utf8(&status)
// );
//             false
//         }
//         Err(err) => {
// logger::error!(
//     tag = "createStreamGroup",
//     "{:?}",
//     err
// );
//             false
//         }
//     }
// }

// // Original Haskell function: getStreamLength
// pub fn getStreamLength(
//     db_name: String,
//     stream: impl RedisKey,
// ) -> Integer {
//     let result = RC.xlen(&db_name, &stream);
//     match result {
//         Ok(res) => res,
//         Err(err) => {
// logger::error!(
//     tag = "getStreamLength",
//     "{}",
//     err.to_string()
// );
//             0
//         }
//     }
// }

// // Original Haskell function: pingRequestRedis
// pub fn pingRequestRedis(db_name: String) -> Result<String, Error> {
//     let result = RC::pingRequest(&db_name);
//     match result {
//         Ok(R::Pong) => Ok("True".to_string()),
//         _ => Err(Errors::throwExceptionV2(
//             REDIS_PING_REQUEST_FAILED,
//             ET::ErrorResponse {
//                 code: 500,
//                 response: ET::ErrorPayload {
//                     error_message: "Internal Server Error".to_string(),
//                     user_message: format!("{}", result),
//                     error: true,
//                     userMessage: None,
//                     error_info: Errors::mkUnifiedError(
//                         "INTERNAL_SERVER_ERROR",
//                         "Internal server error.",
//                         "Redis ping error.",
//                         None,
//                     ),
//                 },
//             },
//         )),
//     }
// }

// // Original Haskell function: getMultiAction
// pub fn getMultiAction(
//     multi: Multi,
// ) -> impl Fn(ByteString) -> L::KVDBTx<R::Queued<Vec<Value>>> {
//     move |prefix: ByteString| {
//         let actions = multi.actions;
//         let results: Vec<_> = actions.into_iter().map(|action| action(prefix.clone())).collect();
//         results.into_iter().collect::<Result<Vec<_>, _>>().map(|v| v.into_iter().collect())
//     }
// }

// // Original Haskell function: getMultiActions
// pub fn getMultiActions(multi: Multi) -> Vec<Box<dyn Fn(ByteString) -> KVDBTx<R::Queued<Value>>>>
// {
//     multi.actions
// }

// // Original Haskell function: textToKey
// pub fn textToKey(k: String) -> T::KVDBKey {
//     k.into_bytes()
// }
