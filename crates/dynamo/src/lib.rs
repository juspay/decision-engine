//! ## Dynamo: Dynamic Routing
//!
//! This library is a dynamic routing system that can be used to route payments to different
//! processors. The system is designed to be able to route payments based on the success rate of
//! the processors, elimination under performing processors.
//!
//! This file contains the main trait that defines the behavior of the dynamic routing system.

use error_stack::Result;

/// [`DynamicRouting`] is a trait that defines the behavior of a dynamic routing system.
///
/// This system ideally consists of 3 data categories:
/// - `Id`: The unique identifier for the entity this is implemented for.
/// - `Params`: The parameters based on which the routing is done.
/// - `Labels`: The labels is the final deicision made by the routing system.
///
///
///
#[async_trait::async_trait]
pub trait DynamicRouting {
    type UpdateWindowReport;
    type RoutingResponse;

    type Error;

    /// [`perform_routing`] is a function that performs the specified routing of the current
    /// parameters for the specified entity.
    ///
    ///
    async fn perform_routing(
        &self,
        id: &str,
        params: &str,
        labels: Vec<String>,
        config: serde_json::Value,
        tenant_id: &Option<String>,
    ) -> Result<Self::RoutingResponse, Self::Error>;

    /// [`update_window`] is used to update the window stored in memory for the entity.
    ///
    /// # Note
    /// The underlying system should make sure that this function isn't invoked concurrently, but
    /// rather maintain a semaphore to ensure that only one instance of this function is running at
    /// a time.
    ///
    async fn update_window(
        &self,
        id: &str,
        params: &str,
        report: Self::UpdateWindowReport,
        config: serde_json::Value,
        tenant_id: &Option<String>,
    ) -> Result<(), Self::Error>;

    /// [`invalidate_metrics`] is a function that invalidates all the metrics stored for a given ID
    ///
    ///
    async fn invalidate_metrics(
        &self,
        id: &str,
        tenant_id: &Option<String>,
    ) -> Result<(), Self::Error>;
}

pub mod app;
pub mod authentication;
pub mod configs;
pub mod consts;
pub mod contract_routing;
pub mod elimination;
pub mod ephemeral_store;
pub mod error;
pub mod health_check;
pub mod helpers;
pub mod logger;
pub mod metrics;
pub mod proto_build;
pub mod secrets;
pub mod success_rate;
pub mod utils;
