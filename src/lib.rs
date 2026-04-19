#![allow(non_snake_case)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::type_complexity)]
#![allow(clippy::as_conversions)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::module_inception)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::match_single_binding)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::map_identity)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::manual_filter_map)]
#![allow(clippy::single_match)]
#![allow(clippy::manual_ok_err)]
pub mod analytics;
pub mod api_client;
pub mod app;
pub mod config;
pub mod crypto;
pub mod custom_extractors;
pub mod decider;
pub mod error;
pub mod euclid;
pub mod feedback;
pub mod generics;
pub mod logger;
pub mod merchant_config_util;
pub mod metrics;
#[cfg(feature = "middleware")]
pub mod middleware;
pub mod redis;
pub mod routes;
pub mod storage;
pub mod tenant;
pub mod types;
pub mod utils;
pub mod validations;
