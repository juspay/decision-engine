use crate::{
    app::{get_tenant_app_state, APP_STATE},
    error::ContainerError,
    euclid::{
        ast::{ConnectorInfo, ValueType},
        errors::EuclidErrors,
        types::RoutingRequest,
    },
};
use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    Json,
};
use error_stack::ResultExt;
use fred::interfaces::KeysInterface;
use masking::PeekInterface;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
pub struct ConnectorInput {
    connector_name: String,
    merchant_connector_id: String,
    profile_id: String,
    connector_type: String,
    #[serde(default)]
    disabled: Option<bool>,
    status: Option<String>,
    payment_methods_enabled: Option<Vec<PaymentMethodInput>>,
}

#[derive(Debug, Deserialize)]
struct PaymentMethodInput {
    payment_method: String,
    payment_method_types: Option<Vec<PaymentMethodTypeInput>>,
}

#[derive(Debug, Deserialize)]
struct PaymentMethodTypeInput {
    payment_method_type: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ConnectorEligibility {
    connector_name: String,
    merchant_connector_id: String,
    payment_methods: HashMap<String, HashSet<String>>,
}

fn redis_key(profile_id: &str) -> String {
    format!("profile_connector_eligibility:{profile_id}")
}

fn compact_snapshot(
    profile_id: &str,
    inputs: Vec<ConnectorInput>,
) -> Result<Vec<ConnectorEligibility>, String> {
    let valid = !profile_id.trim().is_empty()
        && inputs.iter().all(|input| {
            input.profile_id == profile_id
                && !input.connector_name.trim().is_empty()
                && !input.merchant_connector_id.trim().is_empty()
        });
    let mut identifiers = HashSet::new();
    let unique = inputs
        .iter()
        .all(|input| identifiers.insert(input.merchant_connector_id.clone()));
    if valid && unique {
        Ok(inputs
            .into_iter()
            .filter(|input| {
                input.connector_type == "payment_processor"
                    && input.disabled != Some(true)
                    && input
                        .status
                        .as_deref()
                        .is_none_or(|status| status == "active")
            })
            .map(|input| {
                let mut payment_methods: HashMap<String, HashSet<String>> = HashMap::new();
                for method in input.payment_methods_enabled.unwrap_or_default() {
                    let types = payment_methods
                        .entry(method.payment_method.trim().to_ascii_lowercase())
                        .or_default();
                    for method_type in method.payment_method_types.unwrap_or_default() {
                        types.insert(method_type.payment_method_type.trim().to_ascii_lowercase());
                    }
                }
                ConnectorEligibility {
                    connector_name: input.connector_name,
                    merchant_connector_id: input.merchant_connector_id,
                    payment_methods,
                }
            })
            .collect())
    } else {
        Err("Every connector must belong to the path profile and have nonempty, unique MCA identifiers".to_string())
    }
}

async fn store_snapshot(
    redis: &redis_interface::RedisConnectionPool,
    profile_id: &str,
    snapshot: &[ConnectorEligibility],
) -> error_stack::Result<(), redis_interface::errors::RedisError> {
    let bytes = serde_json::to_vec(snapshot)
        .change_context(redis_interface::errors::RedisError::JsonSerializationFailed)?;
    redis
        .pool
        .set::<(), _, _>(
            redis.add_prefix(&redis_key(profile_id)),
            bytes.as_slice(),
            None,
            None,
            false,
        )
        .await
        .change_context(redis_interface::errors::RedisError::SetFailed)
}

fn admin_authorized(headers: &HeaderMap, expected: &str) -> bool {
    !expected.is_empty()
        && headers
            .get("x-admin-secret")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == expected)
}

pub async fn replace_profile_connectors(
    headers: HeaderMap,
    Path(profile_id): Path<String>,
    Json(inputs): Json<Vec<ConnectorInput>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let config = APP_STATE.get().map(|state| &state.global_config).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Application state unavailable".to_string(),
    ))?;
    let expected = config.admin_secret.secret.peek();
    let authorized = admin_authorized(&headers, expected);
    if authorized {
        let snapshot = compact_snapshot(&profile_id, inputs)
            .map_err(|message| (StatusCode::BAD_REQUEST, message))?;
        let state = get_tenant_app_state().await;
        store_snapshot(&state.redis_conn.conn, &profile_id, &snapshot)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to store connector eligibility".to_string(),
                )
            })?;
        Ok(Json(
            serde_json::json!({"profile_id": profile_id, "connector_count": snapshot.len()}),
        ))
    } else {
        Err((StatusCode::UNAUTHORIZED, "Invalid admin secret".to_string()))
    }
}

fn parameter<'a>(request: &'a RoutingRequest, name: &str) -> Option<&'a str> {
    match request.parameters.get(name).and_then(Option::as_ref) {
        Some(ValueType::EnumVariant(value) | ValueType::StrValue(value)) => Some(value.as_str()),
        _ => None,
    }
}

fn eligible_fallback(
    request: &RoutingRequest,
    snapshot: &[ConnectorEligibility],
) -> Vec<ConnectorInfo> {
    let pm = parameter(request, "payment_method").map(|value| value.trim().to_ascii_lowercase());
    let pmt =
        parameter(request, "payment_method_type").map(|value| value.trim().to_ascii_lowercase());
    let mut seen = HashSet::new();
    request
        .fallback_output
        .iter()
        .flatten()
        .flat_map(|fallback| {
            snapshot
                .iter()
                .filter(|mca| {
                    fallback
                        .gateway_name
                        .eq_ignore_ascii_case(&mca.connector_name)
                        && fallback
                            .gateway_id
                            .as_ref()
                            .is_none_or(|id| id == &mca.merchant_connector_id)
                        && mca.payment_methods.iter().any(|(method, types)| {
                            pm.as_ref().is_none_or(|value| value == method)
                                && pmt.as_ref().is_none_or(|value| types.contains(value))
                        })
                })
                .map(|mca| ConnectorInfo {
                    gateway_name: mca.connector_name.clone(),
                    gateway_id: Some(mca.merchant_connector_id.clone()),
                })
        })
        .filter(|connector| {
            seen.insert((connector.gateway_name.clone(), connector.gateway_id.clone()))
        })
        .collect()
}

pub async fn transform_fallback(
    request: &mut RoutingRequest,
) -> Result<(), ContainerError<EuclidErrors>> {
    if request.fallback_output.is_some() {
        let state = get_tenant_app_state().await;
        let bytes: Vec<u8> = state
            .redis_conn
            .conn
            .get_key(&redis_key(&request.created_by))
            .await
            .map_err(|_| EuclidErrors::StorageError)?;
        if !bytes.is_empty() {
            let snapshot: Vec<ConnectorEligibility> =
                serde_json::from_slice(&bytes).map_err(|_| EuclidErrors::StorageError)?;
            let fallback = eligible_fallback(request, &snapshot);
            if fallback.is_empty() {
                Err(EuclidErrors::InvalidRequest(
                    "No fallback connector supports the payment method and type for this profile"
                        .to_string(),
                )
                .into())
            } else {
                request.fallback_output = Some(fallback);
                Ok(())
            }
        } else {
            Ok(())
        }
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: &str, pm: &str, pmt: &str, disabled: bool) -> ConnectorInput {
        ConnectorInput {
            connector_name: "processor".to_string(),
            merchant_connector_id: id.to_string(),
            profile_id: "profile".to_string(),
            connector_type: "payment_processor".to_string(),
            disabled: Some(disabled),
            status: Some("active".to_string()),
            payment_methods_enabled: Some(vec![PaymentMethodInput {
                payment_method: pm.to_string(),
                payment_method_types: Some(vec![PaymentMethodTypeInput {
                    payment_method_type: pmt.to_string(),
                }]),
            }]),
        }
    }

    fn request(id: Option<&str>) -> RoutingRequest {
        RoutingRequest {
            payment_id: None,
            created_by: "profile".to_string(),
            fallback_output: Some(vec![ConnectorInfo {
                gateway_name: "processor".to_string(),
                gateway_id: id.map(str::to_string),
            }]),
            parameters: HashMap::from([
                (
                    "payment_method".to_string(),
                    Some(ValueType::EnumVariant("card".to_string())),
                ),
                (
                    "payment_method_type".to_string(),
                    Some(ValueType::StrValue("credit".to_string())),
                ),
            ]),
            algorithm_for: None,
        }
    }

    #[test]
    fn filters_per_mca_and_expands_only_matching_accounts() {
        let snapshot = compact_snapshot(
            "profile",
            vec![
                input("credit", "card", "credit", false),
                input("wallet", "wallet", "apple_pay", false),
                input("disabled", "card", "credit", true),
                input("debit", "card", "debit", false),
            ],
        )
        .expect("valid connector fixtures must compact successfully");
        assert_eq!(
            eligible_fallback(&request(None), &snapshot),
            vec![ConnectorInfo {
                gateway_name: "processor".to_string(),
                gateway_id: Some("credit".to_string())
            }]
        );
        assert!(eligible_fallback(&request(Some("wallet")), &snapshot).is_empty());
        assert!(eligible_fallback(&request(Some("unknown")), &snapshot).is_empty());
        assert!(eligible_fallback(&request(None), &[]).is_empty());
    }

    #[test]
    fn rejects_wrong_profile_and_duplicate_mca() {
        assert!(compact_snapshot("other", vec![input("credit", "card", "credit", false)]).is_err());
        assert!(compact_snapshot(
            "profile",
            vec![
                input("credit", "card", "credit", false),
                input("credit", "card", "debit", false)
            ]
        )
        .is_err());
        assert_eq!(compact_snapshot("profile", vec![]), Ok(vec![]));
    }

    #[test]
    fn missing_type_matches_method_only_and_missing_method_matches_type() {
        let snapshot = compact_snapshot(
            "profile",
            vec![
                input("card", "card", "credit", false),
                input("wallet", "wallet", "apple_pay", false),
            ],
        )
        .expect("valid connector fixtures must compact successfully");
        let mut request = request(None);
        request.parameters.remove("payment_method_type");
        assert_eq!(eligible_fallback(&request, &snapshot).len(), 1);
        request.parameters.remove("payment_method");
        request.parameters.insert(
            "payment_method_type".to_string(),
            Some(ValueType::EnumVariant("apple_pay".to_string())),
        );
        assert_eq!(
            eligible_fallback(&request, &snapshot)[0]
                .gateway_id
                .as_deref(),
            Some("wallet")
        );
    }

    #[test]
    fn admin_secret_is_required_even_with_other_credentials() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", axum::http::HeaderValue::from_static("secret"));
        headers.insert(
            "authorization",
            axum::http::HeaderValue::from_static("Bearer secret"),
        );
        assert!(!admin_authorized(&headers, "secret"));
        headers.insert(
            "x-admin-secret",
            axum::http::HeaderValue::from_static("wrong"),
        );
        assert!(!admin_authorized(&headers, "secret"));
        headers.insert(
            "x-admin-secret",
            axum::http::HeaderValue::from_static("secret"),
        );
        assert!(admin_authorized(&headers, "secret"));
        assert!(!admin_authorized(&headers, ""));
    }

    #[test]
    fn does_not_match_type_from_a_different_method_entry() {
        let mut connector = input("mixed", "card", "debit", false);
        if let Some(methods) = connector.payment_methods_enabled.as_mut() {
            methods.push(PaymentMethodInput {
                payment_method: "wallet".to_string(),
                payment_method_types: Some(vec![PaymentMethodTypeInput {
                    payment_method_type: "credit".to_string(),
                }]),
            });
        }
        let snapshot = compact_snapshot("profile", vec![connector])
            .expect("valid connector fixtures must compact successfully");
        assert!(eligible_fallback(&request(None), &snapshot).is_empty());
    }

    #[test]
    fn preserves_fallback_order_and_deduplicates_expanded_accounts() {
        let snapshot = compact_snapshot(
            "profile",
            vec![
                input("one", "card", "credit", false),
                input("two", "card", "credit", false),
            ],
        )
        .expect("valid connector fixtures must compact successfully");
        let mut request = request(Some("two"));
        if let Some(fallback) = request.fallback_output.as_mut() {
            fallback.push(ConnectorInfo {
                gateway_name: "processor".to_string(),
                gateway_id: None,
            });
        }
        let result = eligible_fallback(&request, &snapshot);
        assert_eq!(
            result
                .iter()
                .filter_map(|connector| connector.gateway_id.as_deref())
                .collect::<Vec<_>>(),
            vec!["two", "one"]
        );
    }

    #[tokio::test]
    #[ignore = "requires Redis at 127.0.0.1:6379"]
    async fn snapshot_replacement_removes_ttl_and_survives_reads() -> Result<(), String> {
        let settings = redis_interface::RedisSettings {
            host: "127.0.0.1".to_string(),
            port: 6379,
            default_ttl: 1,
            ..Default::default()
        };
        let redis = redis_interface::RedisConnectionPool::new(&settings)
            .await
            .map_err(|error| error.to_string())?;
        let profile = format!("eligibility_test_{}", uuid::Uuid::new_v4());
        let key = redis_key(&profile);
        let snapshot = compact_snapshot("profile", vec![input("card", "card", "credit", false)])?;
        redis
            .serialize_and_set_key(&key, &snapshot)
            .await
            .map_err(|error| error.to_string())?;
        let old_ttl: i64 = redis
            .pool
            .ttl(redis.add_prefix(&key))
            .await
            .map_err(|error| error.to_string())?;
        store_snapshot(&redis, &profile, &snapshot)
            .await
            .map_err(|error| error.to_string())?;
        let ttl: i64 = redis
            .pool
            .ttl(redis.add_prefix(&key))
            .await
            .map_err(|error| error.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        let first: Vec<u8> = redis
            .get_key(&key)
            .await
            .map_err(|error| error.to_string())?;
        let second: Vec<u8> = redis
            .get_key(&key)
            .await
            .map_err(|error| error.to_string())?;
        store_snapshot(&redis, &profile, &[])
            .await
            .map_err(|error| error.to_string())?;
        let replacement: Vec<u8> = redis
            .get_key(&key)
            .await
            .map_err(|error| error.to_string())?;
        let replacement_ttl: i64 = redis
            .pool
            .ttl(redis.add_prefix(&key))
            .await
            .map_err(|error| error.to_string())?;
        redis
            .delete_key(&key)
            .await
            .map_err(|error| error.to_string())?;
        assert!((0..=1).contains(&old_ttl));
        assert_eq!(ttl, -1);
        assert_eq!(
            first,
            serde_json::to_vec(&snapshot).map_err(|error| error.to_string())?
        );
        assert_eq!(second, first);
        assert_eq!(replacement, b"[]");
        assert_eq!(replacement_ttl, -1);
        Ok(())
    }
}
