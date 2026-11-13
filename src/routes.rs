pub mod analytics;
pub mod api_key;
pub mod body;
pub mod connector_credentials;
pub mod connector_fees;
pub mod cost_clusters;
pub mod cost_coverage;
pub mod decide;
pub mod decide_gateway;
pub mod decision_gateway;
pub mod gateway_score;
pub mod gsm;
pub mod health;
pub mod hierarchy;
pub mod hybrid_routing;
pub mod invoice_upload;
pub mod merchant_account_config;
pub mod profile_connectors;
pub mod report_upload;
pub mod rule_configuration;
pub mod seed_costs;
pub mod settlement_webhook;
pub mod update_gateway_score;
pub mod update_score;
pub mod user_auth;
pub mod volume_commitment;

/// The deployment's global config, for a route that needs settings outside its tenant's — ClickHouse,
/// cost ingestion. It is set at startup, so a request reaching a route without it is a server fault.
pub(crate) fn global_config(
) -> Result<&'static crate::config::GlobalConfig, crate::error::RouteError> {
    crate::app::APP_STATE
        .get()
        .map(|state| &state.global_config)
        .ok_or_else(|| crate::error::RouteError::internal("app state not initialized"))
}
