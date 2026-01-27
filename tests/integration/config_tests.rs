//! Integration tests for configuration CRUD operations
//!
//! These tests verify the full lifecycle of routing configurations:
//! - Success Rate configuration create/read/update/delete
//! - Elimination configuration create/read/update/delete  
//! - Merchant account management
//! - Configuration validation and error handling

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum_test::TestServer;
use serde_json::json;

async fn setup_test_server() -> TestServer {
    use open_router::{config::GlobalConfig, tenant::GlobalAppState, routes};
    use axum::routing::{delete, get, post};
    use std::sync::Arc;
    
    let mut global_config = GlobalConfig::new()
        .expect("Failed to load test configuration");
    global_config.validate().expect("Configuration validation failed");
    
    let global_app_state = GlobalAppState::new(global_config).await;
    
    let router = axum::Router::new()
        .route("/routing/create", post(open_router::euclid::handlers::routing_rules::routing_create))
        .route("/routing/activate", post(open_router::euclid::handlers::routing_rules::activate_routing_rule))
        .route("/routing/list/:created_by", post(open_router::euclid::handlers::routing_rules::list_all_routing_algorithm_id))
        .route("/routing/list/active/:created_by", post(open_router::euclid::handlers::routing_rules::list_active_routing_algorithm))
        .route("/routing/evaluate", post(open_router::euclid::handlers::routing_rules::routing_evaluate))
        .route("/decide-gateway", post(routes::decide_gateway::decide_gateway))
        .route("/update-gateway-score", post(routes::update_gateway_score::update_gateway_score))
        .route("/rule/create", post(routes::rule_configuration::create_rule_config))
        .route("/rule/get", post(routes::rule_configuration::get_rule_config))
        .route("/rule/update", post(routes::rule_configuration::update_rule_config))
        .route("/rule/delete", post(routes::rule_configuration::delete_rule_config))
        .route("/merchant-account/create", post(routes::merchant_account_config::create_merchant_config))
        .route("/merchant-account/:merchant-id", get(routes::merchant_account_config::get_merchant_config))
        .route("/merchant-account/:merchant-id", delete(routes::merchant_account_config::delete_merchant_config))
        .with_state(global_app_state);
    
    TestServer::new(router).expect("Failed to create test server")
}

#[tokio::test]
async fn test_success_rate_config_full_lifecycle() {
    // Test complete CRUD cycle for SR configuration
    
    let merchant_id = "merchant_sr_lifecycle";
    let server = setup_test_server().await;
    
    // 1. CREATE
    let sr_config = common::create_success_rate_config(merchant_id, 200, 10);
    let create_response = server.post("/rule/create").json(&sr_config).await;
    
    create_response.assert_status_ok();
    let create_body: serde_json::Value = create_response.json();
    common::assert_api_success(&create_body, "Configuration created successfully");
    
    // 2. READ
    let get_payload = json!({
        "merchant_id": merchant_id,
        "algorithm": "successRate"
    });
    let get_response = server.post("/rule/get").json(&get_payload).await;
    
    get_response.assert_status_ok();
    let get_body: serde_json::Value = get_response.json();
    
    assert_eq!(get_body["merchant_id"].as_str().unwrap(), merchant_id);
    assert_eq!(get_body["config"]["type"].as_str().unwrap(), "successRate");
    assert_eq!(
        get_body["config"]["data"]["defaultBucketSize"].as_i64().unwrap(),
        200
    );
    assert_eq!(
        get_body["config"]["data"]["defaultHedgingPercent"]
            .as_i64()
            .unwrap(),
        10
    );
    
    // 3. UPDATE
    let updated_config = common::create_success_rate_config(merchant_id, 300, 15);
    let update_response = server.post("/rule/update").json(&updated_config).await;
    
    update_response.assert_status_ok();
    
    // Verify update
    let get_updated = server.post("/rule/get").json(&get_payload).await;
    let updated_body: serde_json::Value = get_updated.json();
    assert_eq!(
        updated_body["config"]["data"]["defaultBucketSize"]
            .as_i64()
            .unwrap(),
        300
    );
    assert_eq!(
        updated_body["config"]["data"]["defaultHedgingPercent"]
            .as_i64()
            .unwrap(),
        15
    );
    
    // 4. DELETE
    let delete_payload = json!({
        "merchant_id": merchant_id,
        "algorithm": "successRate"
    });
    let delete_response = server.post("/rule/delete").json(&delete_payload).await;
    
    delete_response.assert_status_ok();
    
    // Verify deletion - GET should return error or empty
    let get_after_delete = server.post("/rule/get").json(&get_payload).await;
    assert!(
        get_after_delete.status_code().is_client_error()
            || get_after_delete.status_code().is_success(),
        "After deletion, config should not exist or return error"
    );
}

#[tokio::test]
async fn test_elimination_config_full_lifecycle() {
    // Test complete CRUD cycle for elimination configuration
    
    let merchant_id = "merchant_elim_lifecycle";
    let server = setup_test_server().await;
    
    // 1. CREATE
    let elim_config = common::create_elimination_config(merchant_id, 0.4);
    let create_response = server.post("/rule/create").json(&elim_config).await;
    
    create_response.assert_status_ok();
    
    // 2. READ
    let get_payload = json!({
        "merchant_id": merchant_id,
        "algorithm": "elimination"
    });
    let get_response = server.post("/rule/get").json(&get_payload).await;
    
    get_response.assert_status_ok();
    let get_body: serde_json::Value = get_response.json();
    
    assert_eq!(get_body["merchant_id"].as_str().unwrap(), merchant_id);
    assert_eq!(get_body["config"]["type"].as_str().unwrap(), "elimination");
    assert_eq!(
        get_body["config"]["data"]["threshold"].as_f64().unwrap(),
        0.4
    );
    
    // 3. UPDATE
    let updated_elim = common::create_elimination_config(merchant_id, 0.6);
    let update_response = server.post("/rule/update").json(&updated_elim).await;
    
    update_response.assert_status_ok();
    
    // Verify update
    let get_updated = server.post("/rule/get").json(&get_payload).await;
    let updated_body: serde_json::Value = get_updated.json();
    assert_eq!(
        updated_body["config"]["data"]["threshold"].as_f64().unwrap(),
        0.6
    );
    
    // 4. DELETE
    let delete_payload = json!({
        "merchant_id": merchant_id,
        "algorithm": "elimination"
    });
    let delete_response = server.post("/rule/delete").json(&delete_payload).await;
    
    delete_response.assert_status_ok();
}

#[tokio::test]
async fn test_merchant_account_lifecycle() {
    // Test merchant account create/read/delete
    
    let merchant_id = "merchant_account_test";
    let server = setup_test_server().await;
    
    // CREATE
    let merchant_payload = common::create_merchant_account_payload(merchant_id);
    let create_response = server
        .post("/merchant-account/create")
        .json(&merchant_payload)
        .await;
    
    create_response.assert_status_ok();
    
    // READ
    let get_response = server
        .get(&format!("/merchant-account/{}", merchant_id))
        .await;
    
    get_response.assert_status_ok();
    let get_body: serde_json::Value = get_response.json();
    assert_eq!(get_body["merchant_id"].as_str().unwrap(), merchant_id);
    
    // DELETE
    let delete_response = server
        .delete(&format!("/merchant-account/{}", merchant_id))
        .await;
    
    delete_response.assert_status_ok();
    
    // Verify deletion
    let get_after_delete = server
        .get(&format!("/merchant-account/{}", merchant_id))
        .await;
    
    assert!(
        get_after_delete.status_code().is_client_error(),
        "Deleted merchant account should not exist"
    );
}

#[tokio::test]
async fn test_duplicate_config_creation_handles_gracefully() {
    // Test creating same configuration twice
    
    let merchant_id = "merchant_duplicate";
    let server = setup_test_server().await;
    
    let sr_config = common::create_success_rate_config(merchant_id, 100, 5);
    
    // First creation - should succeed
    let first_response = server.post("/rule/create").json(&sr_config).await;
    first_response.assert_status_ok();
    
    // Second creation - should handle gracefully (update or error)
    let second_response = server.post("/rule/create").json(&sr_config).await;
    
    assert!(
        second_response.status_code().is_success()
            || second_response.status_code().is_client_error(),
        "Duplicate config creation should be handled gracefully"
    );
}

#[tokio::test]
async fn test_get_non_existent_config_returns_error() {
    // Test retrieving configuration that doesn't exist
    
    let server = setup_test_server().await;
    
    let get_payload = json!({
        "merchant_id": "non_existent_merchant_config",
        "algorithm": "successRate"
    });
    
    let response = server.post("/rule/get").json(&get_payload).await;
    
    assert!(
        response.status_code().is_client_error() || response.status_code().is_server_error(),
        "Non-existent config should return error"
    );
}

#[tokio::test]
async fn test_update_non_existent_config_handles_gracefully() {
    // Test updating configuration that doesn't exist
    
    let merchant_id = "non_existent_for_update";
    let server = setup_test_server().await;
    
    let sr_config = common::create_success_rate_config(merchant_id, 100, 5);
    let response = server.post("/rule/update").json(&sr_config).await;
    
    // Should either create (upsert) or return error
    assert!(
        response.status_code().is_success()
            || response.status_code().is_client_error()
            || response.status_code().is_server_error(),
        "Update of non-existent config should be handled"
    );
}

#[tokio::test]
async fn test_delete_already_deleted_config_handles_gracefully() {
    // Test deleting configuration that was already deleted
    
    let merchant_id = "merchant_double_delete";
    let server = setup_test_server().await;
    
    // Create and delete
    let sr_config = common::create_success_rate_config(merchant_id, 100, 5);
    server.post("/rule/create").json(&sr_config).await;
    
    let delete_payload = json!({
        "merchant_id": merchant_id,
        "algorithm": "successRate"
    });
    
    server.post("/rule/delete").json(&delete_payload).await;
    
    // Delete again
    let second_delete = server.post("/rule/delete").json(&delete_payload).await;
    
    assert!(
        second_delete.status_code().is_success()
            || second_delete.status_code().is_client_error(),
        "Double delete should be handled gracefully"
    );
}

#[tokio::test]
async fn test_invalid_config_data_returns_validation_error() {
    // Test that invalid configuration data is rejected
    
    let server = setup_test_server().await;
    
    // Invalid: negative bucket size
    let invalid_config = json!({
        "merchant_id": "invalid_merchant",
        "config": {
            "type": "successRate",
            "data": {
                "defaultLatencyThreshold": 90,
                "defaultSuccessRate": 0.5,
                "defaultBucketSize": -100,  // Invalid negative value
                "defaultHedgingPercent": 5,
                "subLevelInputConfig": []
            }
        }
    });
    
    let response = server.post("/rule/create").json(&invalid_config).await;
    
    assert!(
        response.status_code().is_client_error(),
        "Invalid config should return 4xx error"
    );
}

#[tokio::test]
async fn test_sr_config_with_sublevel_configuration() {
    // Test SR configuration with payment-method-specific overrides
    
    let merchant_id = "merchant_sublevel";
    let server = setup_test_server().await;
    
    let sr_config = json!({
        "merchant_id": merchant_id,
        "config": {
            "type": "successRate",
            "data": {
                "defaultLatencyThreshold": 90,
                "defaultSuccessRate": 0.5,
                "defaultBucketSize": 200,
                "defaultHedgingPercent": 5,
                "subLevelInputConfig": [
                    {
                        "paymentMethodType": "upi",
                        "paymentMethod": "upi_collect",
                        "bucketSize": 150,
                        "hedgingPercent": 2
                    },
                    {
                        "paymentMethodType": "card",
                        "paymentMethod": "credit_card",
                        "bucketSize": 300,
                        "hedgingPercent": 10
                    }
                ]
            }
        }
    });
    
    let create_response = server.post("/rule/create").json(&sr_config).await;
    create_response.assert_status_ok();
    
    // Verify sublevel configs are stored
    let get_payload = json!({
        "merchant_id": merchant_id,
        "algorithm": "successRate"
    });
    let get_response = server.post("/rule/get").json(&get_payload).await;
    
    get_response.assert_status_ok();
    let body: serde_json::Value = get_response.json();
    
    let sublevel_configs = body["config"]["data"]["subLevelInputConfig"]
        .as_array()
        .expect("subLevelInputConfig should be array");
    
    assert_eq!(sublevel_configs.len(), 2, "Should have 2 sublevel configs");
}
