//! A/B experiment execution: deciding which arm a payment belongs to, running that arm, and
//! joining the routing decision to the outcome that comes back later.
//!
//! The shapes these operate on live in [`crate::types::ab_test`].

pub mod config;
pub mod evaluator;
pub mod interceptor;
pub mod outcome;
pub mod preview;

pub use config::is_intercepting;
pub use interceptor::{intercept, AbTestIntercept};
pub use outcome::{
    emit_if_in_flight, is_rule_only_arm_inflight, record_cost_outcome, store_inflight,
};
