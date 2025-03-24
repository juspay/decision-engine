use serde_json as A;
use std::str::FromStr;
use std::string::String;
use std::vec::Vec;
use std::option::Option;
use std::env;


pub fn resolve_env<T, F>(key: String, default: F) -> T
where
    F: FnOnce() -> T,
    T: FromStrExt,
{
    let envVar = env::var(key);
    match envVar {
        Ok(val) => T::from_str(&val).unwrap_or_else(|_| default()),
        Err(_) => default(),
    }
}

// impl<T: FromStr> FromStr for Option<T> {
//     type Err = A::Error;

//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Ok(Some(s.to_string()))
//     }
// }

trait FromStrExt {
    type Error;
    fn from_str(s: &str) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

// impl<T: FromStr> FromStrExt for T 
// where
//     <T as FromStr>::Err: std::fmt::Debug,
// {
//     type Error = <T as FromStr>::Err;
//     fn from_str(s: &str) -> Result<Self, Self::Error> {
//         Ok(s.parse().unwrap())
//     }
// }

impl<T: FromStr> FromStrExt for Option<T> 
where
    <T as FromStr>::Err: std::fmt::Debug,
{
    type Error = <T as FromStr>::Err;
    fn from_str(s: &str) -> Result<Self, Self::Error> {
        Ok(Some(s.parse().unwrap()))
    }
}

// TODO: When will it be options ?
struct VecFromStr(Vec<String>);

impl FromStrExt for VecFromStr {
    type Error = String;
    fn from_str(s: &str) -> Result<Self, Self::Error>
    {
        let a: Vec<String> = s.split(",").map(|s| s.to_string()).collect();
        Ok(VecFromStr(a))
    }
}



impl FromStrExt for String {
    type Error = A::Error;

    fn from_str(s: &str) -> Result<Self, Self::Error> {
        Ok(s.to_string())
    }
}

impl FromStrExt for i32 {
    type Error = A::Error;

    fn from_str(s: &str) -> Result<Self, Self::Error> {
        Ok(s.parse().unwrap())
    }
}

pub fn euler_endpoint() -> String {
    resolve_env("EULER_PROD_INTERNAL_ENDPOINT".to_string(), euler_prod_internal_endpoint)
}

pub fn euler_prod_internal_endpoint() -> String {
    resolve_env("PROD_EULER_INTERNAL_ENDPOINT".to_string(), || "http://euler.prod.internal.mum.juspay.net".to_string())
}

pub fn is_passed_gpm() -> Option<String> {
    resolve_env("IS_PASSED_GATEWAY_PRIORITY_LIST".to_string(), || None)
}

pub fn number_of_streams_for_routing_metrics() -> i32 {
    resolve_env("COUNT_STREAM_SHARDS_ROUTING_METRICS".to_string(), || 1024)
}

pub fn merchant_disabled_for_sodexo_duplicate_check() -> Vec<String> {
    resolve_env("MERC_DISABLED_FOR_SODEXO_DUPLICATE_CARD_CHECK".to_string(), || VecFromStr(Vec::new())).0
}
