//! Common utilities and test helpers for integration tests
//!
//! This module provides shared functionality for setting up test environments,
//! creating test data, and asserting expected behaviors across integration tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;

/// Test merchant ID used across integration tests
pub const TEST_MERCHANT_ID: &str = "test_merchant_integration";

/// Test gateway names
pub const GATEWAY_A: &str = "GATEWAY_A";
pub const GATEWAY_B: &str = "GATEWAY_B";
pub const GATEWAY_C: &str = "GATEWAY_C";

/// Helper function to create a valid payment info payload
pub fn create_payment_info(
    payment_id: &str,
    amount: f64,
    payment_method_type: &str,
    payment_method: &str,
) -> serde_json::Value {
    json!({
        "paymentId": payment_id,
        "amount": amount,
        "currency": "USD",
        "customerId": "CUST_TEST_001",
        "udfs": null,
        "preferredGateway": null,
        "paymentType": "ORDER_PAYMENT",
        "metadata": null,
        "internalMetadata": null,
        "isEmi": false,
        "emiBank": null,
        "emiTenure": null,
        "paymentMethodType": payment_method_type,
        "paymentMethod": payment_method,
        "paymentSource": null,
        "authType": null,
        "cardIssuerBankName": null,
        "cardIsin": null,
        "cardType": null,
        "cardSwitchProvider": null
    })
}

/// Helper function to create a decide-gateway request payload
pub fn create_decide_gateway_request(
    merchant_id: &str,
    eligible_gateways: Vec<&str>,
    ranking_algorithm: &str,
    payment_info: serde_json::Value,
) -> serde_json::Value {
    json!({
        "merchantId": merchant_id,
        "eligibleGatewayList": eligible_gateways,
        "rankingAlgorithm": ranking_algorithm,
        "eliminationEnabled": true,
        "paymentInfo": payment_info
    })
}

/// Helper function to create an update-gateway-score request
pub fn create_update_score_request(
    merchant_id: &str,
    gateway: &str,
    status: &str,
    payment_id: &str,
) -> serde_json::Value {
    json!({
        "merchantId": merchant_id,
        "gateway": gateway,
        "gatewayReferenceId": null,
        "status": status,
        "paymentId": payment_id
    })
}

/// Helper function to create a success rate configuration
pub fn create_success_rate_config(
    merchant_id: &str,
    default_bucket_size: i32,
    default_hedging_percent: i32,
) -> serde_json::Value {
    json!({
        "merchant_id": merchant_id,
        "config": {
            "type": "successRate",
            "data": {
                "defaultLatencyThreshold": 90,
                "defaultSuccessRate": 0.5,
                "defaultBucketSize": default_bucket_size,
                "defaultHedgingPercent": default_hedging_percent,
                "subLevelInputConfig": []
            }
        }
    })
}

/// Helper function to create an elimination configuration
pub fn create_elimination_config(merchant_id: &str, threshold: f64) -> serde_json::Value {
    json!({
        "merchant_id": merchant_id,
        "config": {
            "type": "elimination",
            "data": {
                "threshold": threshold
            }
        }
    })
}

/// Helper function to create a merchant account
pub fn create_merchant_account_payload(merchant_id: &str) -> serde_json::Value {
    json!({
        "merchant_id": merchant_id
    })
}

/// Helper function to create a priority logic routing algorithm
pub fn create_priority_logic_request(
    merchant_id: &str,
    rule_name: &str,
    payment_method: &str,
    gateways: Vec<&str>,
) -> serde_json::Value {
    let priority_gateways: Vec<serde_json::Value> = gateways
        .iter()
        .enumerate()
        .map(|(i, gw)| {
            json!({
                "gateway_name": gw,
                "gateway_id": format!("mca_{}", i + 100)
            })
        })
        .collect();

    json!({
        "name": rule_name,
        "created_by": merchant_id,
        "description": "Integration test priority rule",
        "algorithm_for": "payment",
        "algorithm": {
            "type": "advanced",
            "data": {
                "globals": {},
                "default_selection": {
                    "priority": priority_gateways.clone()
                },
                "rules": [
                    {
                        "name": "Payment Method Rule",
                        "routingType": "priority",
                        "output": {
                            "priority": priority_gateways
                        },
                        "statements": [
                            {
                                "condition": [
                                    {
                                        "lhs": "payment_method",
                                        "comparison": "equal",
                                        "value": {
                                            "type": "enum_variant",
                                            "value": payment_method
                                        },
                                        "metadata": {}
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        },
        "metadata": {}
    })
}

/// Generate a unique test payment ID
pub fn generate_payment_id(prefix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();
    format!("{}_{}", prefix, timestamp)
}

/// Helper to extract field from JSON response
pub fn extract_field<'a>(json: &'a serde_json::Value, field: &str) -> Option<&'a serde_json::Value> {
    json.get(field)
}

/// Helper to assert successful API response
pub fn assert_api_success(response: &serde_json::Value, expected_message: &str) {
    if let Some(msg) = response.as_str() {
        assert!(
            msg.contains(expected_message),
            "Expected message containing '{}', got: {}",
            expected_message,
            msg
        );
    }
}
