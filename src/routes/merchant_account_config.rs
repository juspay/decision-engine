use crate::auth::AuthorizedMerchant;
use crate::custom_extractors::Json;
use crate::custom_extractors::{MerchantFromPath, ScopedJson};
use crate::redis::types::{FeatureConf, MerchantFeature as FeatureMerchant};
use crate::types::merchant as ETM;
use crate::types::service_configuration;
use crate::{error, logger};
use axum::extract::Path;
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum::EnumIter)]
#[serde(rename_all = "kebab-case")]
pub enum KnownFeature {
    GsmScoringFilter,
    ExploreExploitSrv3,
    AbTestRealPayments,
    CostSavings,
    Elimination,
    Autopilot,
    /// Volume-commitment steering — a secondary objective that runs alongside cost savings.
    VolumeContracts,
    SrRouting,
    /// Outcomes of hybrid payments decided by their rule output also update SR scores.
    SrScoresFromRuleRouting,
}

impl KnownFeature {
    /// The service_configuration key that holds the FeatureConf for this feature.
    /// This is the single source of truth — no per-merchant keys are used.
    fn feature_conf_key(&self) -> &'static str {
        match self {
            Self::GsmScoringFilter => "gsm_based_scoring_filter_enabled_merchant",
            Self::ExploreExploitSrv3 => "ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3_CARD",
            Self::AbTestRealPayments => "ab_test_real_payments_enabled",
            Self::CostSavings => "cost_savings_enabled",
            Self::Elimination => "enable_gateway_level_sr_elimination",
            Self::Autopilot => "autopilot_enabled",
            // The same key `flow_new` checks, so this toggle drives the routing gate directly.
            Self::VolumeContracts => {
                crate::decider::gatewaydecider::volume_commitment::FEATURE_FLAG
            }
            Self::SrRouting => crate::routes::hybrid_routing::SR_ROUTING_FEATURE_FLAG,
            Self::SrScoresFromRuleRouting => {
                crate::routes::hybrid_routing::SR_SCORES_FROM_RULE_ROUTING_FEATURE_FLAG
            }
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
    ///
    /// Every merchant shares the one row, so the write only lands if the row still holds what was
    /// read; otherwise another merchant's toggle got in first and the update is redone on top of
    /// it. A blind write would drop that toggle and silently flip the feature for its merchant.
    /// Toggles on this instance queue on a lock rather than retrying against each other, so the
    /// retries only absorb the occasional race with another instance.
    async fn update_conf(
        &self,
        merchant_id: &str,
        enabled: bool,
    ) -> error_stack::Result<(), crate::generics::MeshError> {
        let key = self.feature_conf_key().to_string();
        let _queued = FEATURE_CONF_WRITE_LOCK.lock().await;

        for attempt in 0..FEATURE_CONF_UPDATE_ATTEMPTS {
            if attempt > 0 {
                // Let the other instance's write land before re-reading.
                tokio::time::sleep(std::time::Duration::from_millis(
                    10 * attempt as u64 + rand::random::<u64>() % 20,
                ))
                .await;
            }
            let existing = service_configuration::find_config_by_name(key.clone())
                .await
                .map_err(|e| error_stack::report!(e))?;
            let Some(existing) = existing else {
                let serialized = with_merchant(None, merchant_id, enabled)?;
                match service_configuration::insert_config(key.clone(), Some(serialized)).await {
                    Ok(()) => return Ok(()),
                    // The name is unique: when another instance created the row first, this
                    // insert is refused and the next attempt merges into the row it created.
                    Err(error) => {
                        let created = service_configuration::find_config_by_name(key.clone())
                            .await
                            .ok()
                            .flatten()
                            .is_some();
                        if !created {
                            return Err(error);
                        }
                        continue;
                    }
                }
            };

            let serialized = with_merchant(existing.value.as_deref(), merchant_id, enabled)?;
            if service_configuration::compare_and_update_config(
                key.clone(),
                existing.value,
                Some(serialized),
            )
            .await?
            {
                return Ok(());
            }
        }

        Err(error_stack::report!(crate::generics::MeshError::Others)).attach_printable(format!(
            "{key} kept changing; gave up after {FEATURE_CONF_UPDATE_ATTEMPTS} attempts"
        ))
    }
}

/// How many times a feature toggle is redone after losing a race with another instance's toggle.
const FEATURE_CONF_UPDATE_ATTEMPTS: usize = 10;

/// Serializes this instance's FeatureConf writes; see `KnownFeature::update_conf`.
static FEATURE_CONF_WRITE_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

/// The FeatureConf JSON `current` becomes with `merchant_id` enabled or disabled. A missing or
/// unparsable `current` starts from an empty roster.
fn with_merchant(
    current: Option<&str>,
    merchant_id: &str,
    enabled: bool,
) -> error_stack::Result<String, crate::generics::MeshError> {
    let mut conf: FeatureConf = current
        .and_then(|v| serde_json::from_str(v).ok())
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

    serde_json::to_string(&conf)
        .map_err(|e| error_stack::report!(e))
        .change_context(crate::generics::MeshError::Others)
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
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<
    Json<DebitRoutingResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    let merchant_id = merchant.as_str().to_string();

    logger::debug!(
        "Received request to get debit routing flag for merchant {}",
        merchant_id
    );

    let response = async {
        ETM::merchant_account::load_authorized(&merchant)
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

    response
}

#[axum::debug_handler]
pub async fn get_merchant_config(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<
    Json<ETM::merchant_account::MerchantAccountResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    let merchant_id = merchant.as_str().to_string();

    logger::debug!(
        "Received request to get merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::load_authorized(&merchant)
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound);

    result
        .map(|merchant_account| Json(merchant_account.into()))
        .map_err(Into::into)
}

#[axum::debug_handler]
pub async fn create_merchant_config(
    ScopedJson {
        merchant, payload, ..
    }: ScopedJson<ETM::merchant_account::MerchantAccountCreateRequest>,
) -> Result<
    Json<MerchantAccountCreateResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    logger::debug!(
        "Received request to create merchant account configuration: {:?}",
        payload
    );

    let merchant_id = payload.merchant_id.clone();
    let gateway_success_rate_based_decider_input =
        payload.gateway_success_rate_based_decider_input.clone();

    if ETM::merchant_account::load_authorized(&merchant)
        .await
        .is_some()
    {
        return Err(error::MerchantAccountConfigurationError::MerchantAlreadyExists.into());
    }

    let result = ETM::merchant_account::insert_merchant_account(payload)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantInsertionFailed);

    match result {
        Ok(_) => {
            logger::debug!("Merchant account configuration created successfully");
            let api_key = crate::routes::api_key::mint_api_key(
                &merchant,
                Some("Default API key".to_string()),
            )
            .await
            .map(|created| created.api_key);
            Ok(Json(MerchantAccountCreateResponse {
                message: "Merchant account created successfully".to_string(),
                merchant_id,
                gateway_success_rate_based_decider_input,
                api_key,
            }))
        }
        Err(e) => Err(e.into()),
    }
}

#[axum::debug_handler]
pub async fn update_debit_routing(
    MerchantFromPath(merchant): MerchantFromPath,
    Json(payload): Json<DebitRoutingRequest>,
) -> Result<
    Json<DebitRoutingResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    let merchant_id = merchant.as_str().to_string();

    logger::debug!(
        "Received request to update debit routing for merchant {}: enabled={}",
        merchant_id,
        payload.enabled
    );

    ETM::merchant_account::load_authorized(&merchant)
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

    let config_name = debit_routing_config_name(&merchant_id);
    let config_value = payload.enabled.to_string();

    let result = service_configuration::upsert_config(config_name, Some(config_value))
        .await
        .change_context(error::MerchantAccountConfigurationError::StorageError);

    result
        .map(|_| {
            Json(DebitRoutingResponse {
                merchant_id: merchant_id.clone(),
                debit_routing_enabled: payload.enabled,
            })
        })
        .map_err(Into::into)
}

/// Every known feature and whether it is on for the merchant, as the dashboard lists them.
async fn features_of(merchant: &AuthorizedMerchant) -> MerchantFeaturesResponse {
    let mut features = Vec::new();
    for feature in KnownFeature::iter() {
        let enabled = feature.read_effective(merchant.as_str()).await;
        features.push(MerchantFeatureEntry { feature, enabled });
    }
    MerchantFeaturesResponse {
        merchant_id: merchant.as_str().to_string(),
        features,
    }
}

/// Feature flags are kept by merchant id whether or not the merchant exists, so an admin caller
/// naming a merchant that was never created is told so rather than flagging a ghost.
async fn ensure_merchant_exists(
    merchant: &AuthorizedMerchant,
) -> Result<(), error::ContainerError<error::MerchantAccountConfigurationError>> {
    ETM::merchant_account::load_authorized(merchant)
        .await
        .map(|_| ())
        .ok_or_else(|| error::MerchantAccountConfigurationError::MerchantNotFound.into())
}

#[axum::debug_handler]
pub async fn get_merchant_features(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<
    Json<MerchantFeaturesResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    ensure_merchant_exists(&merchant).await?;
    Ok(Json(features_of(&merchant).await))
}

#[axum::debug_handler]
pub async fn update_merchant_feature(
    MerchantFromPath(merchant): MerchantFromPath,
    Path((_, feature)): Path<(String, KnownFeature)>,
    Json(payload): Json<UpdateFeatureRequest>,
) -> Result<
    Json<MerchantFeaturesResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    ensure_merchant_exists(&merchant).await?;
    feature
        .update_conf(merchant.as_str(), payload.enabled)
        .await
        .change_context(error::MerchantAccountConfigurationError::StorageError)?;
    Ok(Json(features_of(&merchant).await))
}

#[axum::debug_handler]
pub async fn delete_merchant_config(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<
    Json<MerchantAccountDeleteResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    let merchant_id = merchant.as_str().to_string();

    logger::debug!(
        "Received request to delete merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::delete_merchant_account(merchant_id.clone())
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantDeletionFailed);

    result
        .map(|_| {
            Json(MerchantAccountDeleteResponse {
                message: "Merchant account deleted successfully".to_string(),
                merchant_id,
            })
        })
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::with_merchant;
    use crate::redis::types::FeatureConf;

    fn merchants(json: &str) -> Vec<String> {
        let conf: FeatureConf = serde_json::from_str(json).unwrap();
        conf.merchants
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.merchantId)
            .collect()
    }

    #[test]
    fn enabling_keeps_every_other_merchant_and_setting() {
        let current = r#"{"enableAll":true,"enableAllRollout":20,"disableAny":["m_x"],
            "merchants":[{"merchantId":"m_a","rollout":100},{"merchantId":"m_b","rollout":50}]}"#;
        let next = with_merchant(Some(current), "m_c", true).unwrap();
        assert_eq!(merchants(&next), ["m_a", "m_b", "m_c"]);

        let conf: FeatureConf = serde_json::from_str(&next).unwrap();
        assert!(conf.enableAll);
        assert_eq!(conf.enableAllRollout, Some(20));
        assert_eq!(conf.disableAny, Some(vec!["m_x".to_string()]));
    }

    #[test]
    fn disabling_removes_only_that_merchant_ignoring_case() {
        let current = r#"{"enableAll":false,"merchants":[
            {"merchantId":"M_A","rollout":100},{"merchantId":"m_b","rollout":100}]}"#;
        let next = with_merchant(Some(current), "m_a", false).unwrap();
        assert_eq!(merchants(&next), ["m_b"]);
    }

    #[test]
    fn re_enabling_does_not_duplicate() {
        let current = r#"{"enableAll":false,"merchants":[{"merchantId":"m_a","rollout":100}]}"#;
        let next = with_merchant(Some(current), "m_a", true).unwrap();
        assert_eq!(merchants(&next), ["m_a"]);
    }

    #[test]
    fn missing_or_unparsable_value_starts_empty() {
        assert_eq!(
            merchants(&with_merchant(None, "m_a", true).unwrap()),
            ["m_a"]
        );
        assert_eq!(
            merchants(&with_merchant(Some("not json"), "m_a", true).unwrap()),
            ["m_a"]
        );
    }
}
