use crate::app::APP_STATE;
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::redis::types::{FeatureConf, MerchantFeature as FeatureMerchant};
use crate::types::merchant as ETM;
use crate::types::service_configuration;
use crate::{error, logger};
use axum::{extract::Path, http::HeaderMap, Json};
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantAccountCreateResponse {
    pub message: String,
    pub merchant_id: String,
    pub gateway_success_rate_based_decider_input: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantAccountDeleteResponse {
    pub message: String,
    pub merchant_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebitRoutingRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebitRoutingResponse {
    pub merchant_id: String,
    pub debit_routing_enabled: bool,
}

fn debit_routing_config_name(merchant_id: &str) -> String {
    format!("DEBIT_ROUTING_ENABLED_{}", merchant_id)
}

/// All merchant-level feature flags that can be toggled from the dashboard.
/// Adding a new feature here is the only change needed on the backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownFeature {
    GsmScoringFilter,
    ExploreExploitSrv3,
    AbTestRealPayments,
}

impl KnownFeature {
    fn all() -> &'static [Self] {
        &[
            Self::GsmScoringFilter,
            Self::ExploreExploitSrv3,
            Self::AbTestRealPayments,
        ]
    }

    fn from_slug(s: &str) -> Option<Self> {
        match s {
            "gsm-scoring-filter" => Some(Self::GsmScoringFilter),
            "explore-exploit-srv3" => Some(Self::ExploreExploitSrv3),
            "ab-test-real-payments" => Some(Self::AbTestRealPayments),
            _ => None,
        }
    }

    /// The service_configuration key that holds the FeatureConf for this feature.
    /// This is the single source of truth — no per-merchant keys are used.
    fn feature_conf_key(&self) -> &'static str {
        match self {
            Self::GsmScoringFilter => "gsm_based_scoring_filter_enabled_merchant",
            Self::ExploreExploitSrv3 => "ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3_CARD",
            Self::AbTestRealPayments => "ab_test_real_payments_enabled",
        }
    }

    /// Reads the FeatureConf directly from service_configuration (bypasses Redis/memory
    /// cache) so the dashboard always shows the current persisted state. Falls back to
    /// the Redis-cached path for features whose FeatureConf lives only in Redis.
    async fn read_effective(&self, merchant_id: &str) -> bool {
        let key = self.feature_conf_key();

        let conf = service_configuration::find_config_by_name(key.to_string())
            .await
            .unwrap_or(None)
            .and_then(|c| c.value)
            .and_then(|v| serde_json::from_str::<FeatureConf>(&v).ok());

        if let Some(conf) = conf {
            return crate::redis::feature::check_merchant_enabled(
                Some(conf),
                merchant_id.to_string(),
                key.to_string(),
            );
        }

        // FeatureConf not in service_configuration — may exist only in Redis
        // (e.g. explore-exploit configs that were never migrated to DB).
        crate::redis::feature::is_feature_enabled(
            key.to_string(),
            merchant_id.to_string(),
            crate::feedback::constants::kvRedis(),
        )
        .await
    }

    /// Updates the FeatureConf row in service_configuration by adding or removing
    /// the merchant. This is the only write path — no per-merchant keys involved.
    async fn update_conf(
        &self,
        merchant_id: &str,
        enabled: bool,
    ) -> error_stack::Result<(), crate::generics::MeshError> {
        let key = self.feature_conf_key().to_string();

        let existing = service_configuration::find_config_by_name(key.clone())
            .await
            .unwrap_or(None);

        let exists = existing.is_some();

        let mut conf: FeatureConf = existing
            .and_then(|c| c.value)
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or(FeatureConf {
                enableAll: false,
                enableAllRollout: None,
                disableAny: None,
                merchants: Some(vec![]),
            });

        let mut merchants = conf.merchants.take().unwrap_or_default();
        merchants.retain(|m| m.merchantId.to_lowercase() != merchant_id.to_lowercase());
        if enabled {
            merchants.push(FeatureMerchant {
                merchantId: merchant_id.to_string(),
                rollout: 100,
            });
        }
        conf.merchants = Some(merchants);

        let serialized = serde_json::to_string(&conf)
            .map_err(|e| error_stack::report!(e))
            .change_context(crate::generics::MeshError::Others)?;

        if exists {
            service_configuration::update_config(key, Some(serialized)).await
        } else {
            service_configuration::insert_config(key, Some(serialized)).await
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantFeatureEntry {
    pub feature: KnownFeature,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantFeaturesResponse {
    pub merchant_id: String,
    pub features: Vec<MerchantFeatureEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFeatureRequest {
    pub enabled: bool,
}

#[axum::debug_handler]
pub async fn get_debit_routing(
    Path(merchant_id): Path<String>,
) -> Result<
    Json<DebitRoutingResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_debit_routing_get"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_debit_routing_get"])
        .start_timer();

    logger::debug!(
        "Received request to get debit routing flag for merchant {}",
        merchant_id
    );

    let response = async {
        ETM::merchant_account::load_merchant_by_merchant_id(merchant_id.clone())
            .await
            .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

        let config_name = debit_routing_config_name(&merchant_id);
        let debit_routing_enabled = service_configuration::find_config_by_name(config_name)
            .await
            .change_context(error::MerchantAccountConfigurationError::StorageError)?
            .and_then(|config| config.value)
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(false);

        Ok(Json(DebitRoutingResponse {
            merchant_id,
            debit_routing_enabled,
        }))
    }
    .await;

    match &response {
        Ok(_) => API_REQUEST_COUNTER
            .with_label_values(&["merchant_debit_routing_get", "success"])
            .inc(),
        Err(_) => API_REQUEST_COUNTER
            .with_label_values(&["merchant_debit_routing_get", "failure"])
            .inc(),
    }

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn get_merchant_config(
    Path(merchant_id): Path<String>,
) -> Result<
    Json<ETM::merchant_account::MerchantAccountResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    // Record total request count and start timer
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_account_get"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_account_get"])
        .start_timer();

    logger::debug!(
        "Received request to get merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::load_merchant_by_merchant_id(merchant_id)
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound);

    let response = match result {
        Ok(merchant_account) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_get", "success"])
                .inc();
            Ok(Json(merchant_account.into()))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_get", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn create_merchant_config(
    headers: HeaderMap,
    Json(payload): Json<ETM::merchant_account::MerchantAccountCreateRequest>,
) -> Result<
    Json<MerchantAccountCreateResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(error::MerchantAccountConfigurationError::StorageError)?;

    let provided = headers
        .get("x-admin-secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != global_config.admin_secret.secret {
        return Err(error::MerchantAccountConfigurationError::Unauthorized.into());
    }
    // Record total request count and start timer
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_account_create"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_account_create"])
        .start_timer();

    logger::debug!(
        "Received request to create merchant account configuration: {:?}",
        payload
    );

    let merchant_id = payload.merchant_id.clone();
    let gateway_success_rate_based_decider_input =
        payload.gateway_success_rate_based_decider_input.clone();

    let merchant_account =
        ETM::merchant_account::load_merchant_by_merchant_id(payload.merchant_id.clone()).await;

    if merchant_account.is_some() {
        API_REQUEST_COUNTER
            .with_label_values(&["merchant_account_create", "failure"])
            .inc();
        timer.observe_duration();
        return Err(error::MerchantAccountConfigurationError::MerchantAlreadyExists.into());
    }

    let result = ETM::merchant_account::insert_merchant_account(payload)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantInsertionFailed);

    let response = match result {
        Ok(_) => {
            logger::debug!("Merchant account configuration created successfully");
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_create", "success"])
                .inc();
            let api_key = crate::routes::api_key::insert_api_key_for_merchant(
                &merchant_id,
                Some("Default API key".to_string()),
            )
            .await;
            Ok(Json(MerchantAccountCreateResponse {
                message: "Merchant account created successfully".to_string(),
                merchant_id,
                gateway_success_rate_based_decider_input,
                api_key,
            }))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_create", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn update_debit_routing(
    Path(merchant_id): Path<String>,
    Json(payload): Json<DebitRoutingRequest>,
) -> Result<
    Json<DebitRoutingResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_debit_routing_update"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_debit_routing_update"])
        .start_timer();

    logger::debug!(
        "Received request to update debit routing for merchant {}: enabled={}",
        merchant_id,
        payload.enabled
    );

    // Verify merchant exists
    ETM::merchant_account::load_merchant_by_merchant_id(merchant_id.clone())
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

    let config_name = debit_routing_config_name(&merchant_id);
    let config_value = payload.enabled.to_string();

    // Check if config already exists
    let existing_config = service_configuration::find_config_by_name(config_name.clone())
        .await
        .change_context(error::MerchantAccountConfigurationError::StorageError)?;

    let result = if existing_config.is_some() {
        // Update existing config
        service_configuration::update_config(config_name, Some(config_value))
            .await
            .change_context(error::MerchantAccountConfigurationError::StorageError)
    } else {
        // Insert new config
        service_configuration::insert_config(config_name, Some(config_value))
            .await
            .change_context(error::MerchantAccountConfigurationError::StorageError)
    };

    let response = match result {
        Ok(_) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_debit_routing_update", "success"])
                .inc();
            Ok(Json(DebitRoutingResponse {
                merchant_id: merchant_id.clone(),
                debit_routing_enabled: payload.enabled,
            }))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_debit_routing_update", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn get_merchant_features(
    Path(merchant_id): Path<String>,
) -> Result<
    Json<MerchantFeaturesResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_features_get"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_features_get"])
        .start_timer();

    let response = async {
        ETM::merchant_account::load_merchant_by_merchant_id(merchant_id.clone())
            .await
            .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

        let mut features = Vec::new();
        for feature in KnownFeature::all() {
            let enabled = feature.read_effective(&merchant_id).await;
            features.push(MerchantFeatureEntry {
                feature: feature.clone(),
                enabled,
            });
        }

        Ok(Json(MerchantFeaturesResponse {
            merchant_id,
            features,
        }))
    }
    .await;

    match &response {
        Ok(_) => API_REQUEST_COUNTER
            .with_label_values(&["merchant_features_get", "success"])
            .inc(),
        Err(_) => API_REQUEST_COUNTER
            .with_label_values(&["merchant_features_get", "failure"])
            .inc(),
    }

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn update_merchant_feature(
    Path((merchant_id, feature_slug)): Path<(String, String)>,
    Json(payload): Json<UpdateFeatureRequest>,
) -> Result<
    Json<MerchantFeaturesResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_feature_update"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_feature_update"])
        .start_timer();

    let feature = KnownFeature::from_slug(&feature_slug)
        .ok_or(error::MerchantAccountConfigurationError::InvalidConfiguration)?;

    ETM::merchant_account::load_merchant_by_merchant_id(merchant_id.clone())
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

    let result = feature
        .update_conf(&merchant_id, payload.enabled)
        .await
        .change_context(error::MerchantAccountConfigurationError::StorageError);

    let response = match result {
        Ok(_) => {
            let mut features = Vec::new();
            for f in KnownFeature::all() {
                features.push(MerchantFeatureEntry {
                    feature: f.clone(),
                    enabled: f.read_effective(&merchant_id).await,
                });
            }
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_feature_update", "success"])
                .inc();
            Ok(Json(MerchantFeaturesResponse {
                merchant_id,
                features,
            }))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_feature_update", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn delete_merchant_config(
    Path(merchant_id): Path<String>,
) -> Result<
    Json<MerchantAccountDeleteResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    // Record total request count and start timer
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_account_delete"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_account_delete"])
        .start_timer();

    logger::debug!(
        "Received request to delete merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::delete_merchant_account(merchant_id.clone())
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantDeletionFailed);

    let response = match result {
        Ok(_) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_delete", "success"])
                .inc();
            Ok(Json(MerchantAccountDeleteResponse {
                message: "Merchant account deleted successfully".to_string(),
                merchant_id,
            }))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_delete", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}
