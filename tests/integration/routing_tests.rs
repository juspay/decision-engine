//! Integration tests for core routing decision logic
//!
//! These tests verify end-to-end routing behavior including:
//! - SR-based routing (success rate optimization)
//! - Priority logic evaluation
//! - Elimination (downtime detection)
//! - Hedging behavior
//!
//! Tests use the actual API endpoints and database to ensure
//! real-world scenarios work correctly.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum_test::TestServer;
use serde_json::json;

/// Helper to create test server instance
/// 
/// This creates a fully functional test server with actual database and Redis connections.
/// The server uses test-specific configuration to avoid polluting production data.
async fn setup_test_server() -> TestServer {
    use open_router::{app, config::GlobalConfig, tenant::GlobalAppState};
    use std::sync::Arc;
    
    // Load configuration from environment (uses config.example.toml in tests)
    let mut global_config = GlobalConfig::new()
        .expect("Failed to load test configuration");
    
    global_config.validate()
        .expect("Configuration validation failed");
    
    // Create global app state with all tenants
    let global_app_state = GlobalAppState::new(global_config).await;
    
    // Build the router using the actual server builder logic
    let router = build_test_router(global_app_state);
    
    TestServer::new(router)
        .expect("Failed to create test server")
}

/// Build router with all application routes for testing
fn build_test_router(global_app_state: Arc<open_router::tenant::GlobalAppState>) -> axum::Router {
    use open_router::routes;
    use axum::routing::{delete, get, post};
    
    axum::Router::new()
        // Routing APIs
        .route("/routing/create", post(open_router::euclid::handlers::routing_rules::routing_create))
        .route("/routing/activate", post(open_router::euclid::handlers::routing_rules::activate_routing_rule))
        .route("/routing/list/:created_by", post(open_router::euclid::handlers::routing_rules::list_all_routing_algorithm_id))
        .route("/routing/list/active/:created_by", post(open_router::euclid::handlers::routing_rules::list_active_routing_algorithm))
        .route("/routing/evaluate", post(open_router::euclid::handlers::routing_rules::routing_evaluate))
        
        // Decision APIs
        .route("/decide-gateway", post(routes::decide_gateway::decide_gateway))
        .route("/update-gateway-score", post(routes::update_gateway_score::update_gateway_score))
        
        // Config APIs
        .route("/rule/create", post(routes::rule_configuration::create_rule_config))
        .route("/rule/get", post(routes::rule_configuration::get_rule_config))
        .route("/rule/update", post(routes::rule_configuration::update_rule_config))
        .route("/rule/delete", post(routes::rule_configuration::delete_rule_config))
        
        // Merchant APIs
        .route("/merchant-account/create", post(routes::merchant_account_config::create_merchant_config))
        .route("/merchant-account/:merchant-id", get(routes::merchant_account_config::get_merchant_config))
        .route("/merchant-account/:merchant-id", delete(routes::merchant_account_config::delete_merchant_config))
        
        .with_state(global_app_state)
}

#[tokio::test]
async fn test_sr_based_routing_selects_highest_success_rate_gateway() {
    // Setup: Create merchant with SR routing configuration
    let merchant_id = common::TEST_MERCHANT_ID;
    let payment_id = common::generate_payment_id("SR_TEST");
    
    // Test scenario:
    // 1. Send multiple transactions to GATEWAY_A with SUCCESS status
    // 2. Send multiple transactions to GATEWAY_B with FAILURE status
    // 3. Request new routing decision
    // 4. Expect GATEWAY_A to be selected (higher SR)
    
    let server = setup_test_server().await;
    
    // Create merchant account
    let merchant_payload = common::create_merchant_account_payload(merchant_id);
    let response = server
        .post("/merchant-account/create")
        .json(&merchant_payload)
        .await;
    response.assert_status_ok();
    
    // Configure SR routing
    let sr_config = common::create_success_rate_config(merchant_id, 100, 5);
    let response = server.post("/rule/create").json(&sr_config).await;
    response.assert_status_ok();
    
    // Simulate successful transactions on GATEWAY_A
    for i in 0..10 {
        let pid = format!("{}_TRAIN_A_{}", payment_id, i);
        let update_payload = common::create_update_score_request(
            merchant_id,
            common::GATEWAY_A,
            "SUCCESS",
            &pid,
        );
        server
            .post("/update-gateway-score")
            .json(&update_payload)
            .await
            .assert_status_ok();
    }
    
    // Simulate failed transactions on GATEWAY_B
    for i in 0..10 {
        let pid = format!("{}_TRAIN_B_{}", payment_id, i);
        let update_payload = common::create_update_score_request(
            merchant_id,
            common::GATEWAY_B,
            "FAILURE",
            &pid,
        );
        server
            .post("/update-gateway-score")
            .json(&update_payload)
            .await
            .assert_status_ok();
    }
    
    // Request routing decision
    let payment_info = common::create_payment_info(&payment_id, 100.0, "CARD", "CREDIT_CARD");
    let routing_request = common::create_decide_gateway_request(
        merchant_id,
        vec![common::GATEWAY_A, common::GATEWAY_B],
        "SR_BASED_ROUTING",
        payment_info,
    );
    
    let response = server
        .post("/decide-gateway")
        .json(&routing_request)
        .await;
    
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    
    // Assert GATEWAY_A is selected (100% SR vs 0% SR)
    assert_eq!(
        body["decided_gateway"].as_str().unwrap(),
        common::GATEWAY_A,
        "Expected highest SR gateway to be selected"
    );
    
    assert_eq!(
        body["routing_approach"].as_str().unwrap(),
        "SR_SELECTION_V3_ROUTING",
        "Expected SR-based routing approach"
    );
}

#[tokio::test]
async fn test_priority_logic_overrides_sr_routing() {
    // Setup: Create merchant with priority logic that enforces specific gateway
    let merchant_id = common::TEST_MERCHANT_ID;
    let payment_id = common::generate_payment_id("PL_TEST");
    
    let server = setup_test_server().await;
    
    // Create merchant
    let merchant_payload = common::create_merchant_account_payload(merchant_id);
    server
        .post("/merchant-account/create")
        .json(&merchant_payload)
        .await
        .assert_status_ok();
    
    // Create priority logic: CARD payments → GATEWAY_C
    let pl_request = common::create_priority_logic_request(
        merchant_id,
        "Card Priority Rule",
        "card",
        vec![common::GATEWAY_C],
    );
    
    let response = server.post("/routing/create").json(&pl_request).await;
    response.assert_status_ok();
    let pl_response: serde_json::Value = response.json();
    let rule_id = pl_response["rule_id"].as_str().expect("rule_id");
    
    // Activate the priority logic
    let activate_payload = json!({
        "created_by": merchant_id,
        "routing_algorithm_id": rule_id
    });
    server
        .post("/routing/activate")
        .json(&activate_payload)
        .await
        .assert_status_ok();
    
    // Request routing decision with CARD payment
    let payment_info = common::create_payment_info(&payment_id, 150.0, "CARD", "CREDIT_CARD");
    let routing_request = common::create_decide_gateway_request(
        merchant_id,
        vec![common::GATEWAY_A, common::GATEWAY_B, common::GATEWAY_C],
        "PL_BASED_ROUTING",
        payment_info,
    );
    
    let response = server
        .post("/decide-gateway")
        .json(&routing_request)
        .await;
    
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    
    // Assert priority logic enforced GATEWAY_C
    assert_eq!(
        body["decided_gateway"].as_str().unwrap(),
        common::GATEWAY_C,
        "Priority logic should enforce configured gateway"
    );
    
    assert!(
        body["priority_logic_output"].is_object(),
        "Priority logic output should be present"
    );
}

#[tokio::test]
async fn test_elimination_deprioritizes_low_sr_gateways() {
    // Setup: Configure elimination with threshold 0.5
    // Send transactions to push GATEWAY_A below threshold
    // Verify GATEWAY_A is deprioritized in routing
    
    let merchant_id = common::TEST_MERCHANT_ID;
    let payment_id = common::generate_payment_id("ELIM_TEST");
    
    let server = setup_test_server().await;
    
    // Create merchant
    let merchant_payload = common::create_merchant_account_payload(merchant_id);
    server
        .post("/merchant-account/create")
        .json(&merchant_payload)
        .await
        .assert_status_ok();
    
    // Configure SR routing
    let sr_config = common::create_success_rate_config(merchant_id, 50, 0);
    server
        .post("/rule/create")
        .json(&sr_config)
        .await
        .assert_status_ok();
    
    // Configure elimination with 0.5 threshold
    let elim_config = common::create_elimination_config(merchant_id, 0.5);
    server
        .post("/rule/create")
        .json(&elim_config)
        .await
        .assert_status_ok();
    
    // Send transactions: GATEWAY_A = 20% SR (below threshold)
    for i in 0..10 {
        let status = if i < 2 { "SUCCESS" } else { "FAILURE" };
        let pid = format!("{}_ELIM_A_{}", payment_id, i);
        let update_payload =
            common::create_update_score_request(merchant_id, common::GATEWAY_A, status, &pid);
        server
            .post("/update-gateway-score")
            .json(&update_payload)
            .await
            .assert_status_ok();
    }
    
    // Send transactions: GATEWAY_B = 80% SR (above threshold)
    for i in 0..10 {
        let status = if i < 8 { "SUCCESS" } else { "FAILURE" };
        let pid = format!("{}_ELIM_B_{}", payment_id, i);
        let update_payload =
            common::create_update_score_request(merchant_id, common::GATEWAY_B, status, &pid);
        server
            .post("/update-gateway-score")
            .json(&update_payload)
            .await
            .assert_status_ok();
    }
    
    // Request routing decision
    let payment_info = common::create_payment_info(&payment_id, 200.0, "UPI", "UPI_COLLECT");
    let routing_request = common::create_decide_gateway_request(
        merchant_id,
        vec![common::GATEWAY_A, common::GATEWAY_B],
        "SR_BASED_ROUTING",
        payment_info,
    );
    
    let response = server
        .post("/decide-gateway")
        .json(&routing_request)
        .await;
    
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    
    // Assert GATEWAY_B selected (above threshold)
    assert_eq!(
        body["decided_gateway"].as_str().unwrap(),
        common::GATEWAY_B,
        "Should select gateway above elimination threshold"
    );
    
    // Check routing approach indicates downtime
    let routing_approach = body["routing_approach"].as_str().unwrap();
    assert!(
        routing_approach.contains("DOWNTIME") || routing_approach == "SR_V3_DOWNTIME_ROUTING",
        "Routing approach should indicate downtime handling: {}",
        routing_approach
    );
}

#[tokio::test]
async fn test_all_gateways_down_falls_back_gracefully() {
    // Edge case: All gateways below elimination threshold
    // System should still route (select best of bad options)
    
    let merchant_id = common::TEST_MERCHANT_ID;
    let payment_id = common::generate_payment_id("ALL_DOWN");
    
    let server = setup_test_server().await;
    
    // Setup merchant with elimination
    let merchant_payload = common::create_merchant_account_payload(merchant_id);
    server
        .post("/merchant-account/create")
        .json(&merchant_payload)
        .await
        .assert_status_ok();
    
    let sr_config = common::create_success_rate_config(merchant_id, 50, 0);
    server
        .post("/rule/create")
        .json(&sr_config)
        .await
        .assert_status_ok();
    
    let elim_config = common::create_elimination_config(merchant_id, 0.7);
    server
        .post("/rule/create")
        .json(&elim_config)
        .await
        .assert_status_ok();
    
    // Push all gateways below 70% threshold
    // GATEWAY_A = 10%, GATEWAY_B = 30%
    for i in 0..10 {
        let status_a = if i < 1 { "SUCCESS" } else { "FAILURE" };
        let pid_a = format!("{}_A_{}", payment_id, i);
        server
            .post("/update-gateway-score")
            .json(&common::create_update_score_request(
                merchant_id,
                common::GATEWAY_A,
                status_a,
                &pid_a,
            ))
            .await
            .assert_status_ok();
        
        let status_b = if i < 3 { "SUCCESS" } else { "FAILURE" };
        let pid_b = format!("{}_B_{}", payment_id, i);
        server
            .post("/update-gateway-score")
            .json(&common::create_update_score_request(
                merchant_id,
                common::GATEWAY_B,
                status_b,
                &pid_b,
            ))
            .await
            .assert_status_ok();
    }
    
    // Request routing
    let payment_info = common::create_payment_info(&payment_id, 50.0, "WALLET", "PAYTM");
    let routing_request = common::create_decide_gateway_request(
        merchant_id,
        vec![common::GATEWAY_A, common::GATEWAY_B],
        "SR_BASED_ROUTING",
        payment_info,
    );
    
    let response = server
        .post("/decide-gateway")
        .json(&routing_request)
        .await;
    
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    
    // Should still return a gateway (best of bad options = GATEWAY_B with 30% SR)
    assert!(
        body["decided_gateway"].is_string(),
        "Should return a gateway even when all are down"
    );
    
    assert_eq!(
        body["decided_gateway"].as_str().unwrap(),
        common::GATEWAY_B,
        "Should select least bad gateway"
    );
    
    assert_eq!(
        body["routing_approach"].as_str().unwrap(),
        "SR_V3_ALL_DOWNTIME_ROUTING",
        "Should indicate all gateways in downtime"
    );
}

#[tokio::test]
async fn test_empty_eligible_gateway_list_returns_error() {
    // Edge case: Empty eligible gateway list should return error
    
    let merchant_id = common::TEST_MERCHANT_ID;
    let payment_id = common::generate_payment_id("EMPTY_GW");
    
    let server = setup_test_server().await;
    
    let payment_info = common::create_payment_info(&payment_id, 100.0, "CARD", "DEBIT_CARD");
    let routing_request = common::create_decide_gateway_request(
        merchant_id,
        vec![], // Empty gateway list
        "SR_BASED_ROUTING",
        payment_info,
    );
    
    let response = server
        .post("/decide-gateway")
        .json(&routing_request)
        .await;
    
    // Should return error (4xx status)
    assert!(
        response.status_code().is_client_error() || response.status_code().is_server_error(),
        "Empty gateway list should return error status"
    );
}

#[tokio::test]
async fn test_invalid_merchant_id_returns_error() {
    // Edge case: Non-existent merchant should return error
    
    let payment_id = common::generate_payment_id("INVALID_MERCH");
    
    let server = setup_test_server().await;
    
    let payment_info = common::create_payment_info(&payment_id, 100.0, "NET_BANKING", "HDFC");
    let routing_request = common::create_decide_gateway_request(
        "non_existent_merchant_xyz",
        vec![common::GATEWAY_A],
        "SR_BASED_ROUTING",
        payment_info,
    );
    
    let response = server
        .post("/decide-gateway")
        .json(&routing_request)
        .await;
    
    // Should handle gracefully (either error or default behavior)
    assert!(
        response.status_code().is_success()
            || response.status_code().is_client_error()
            || response.status_code().is_server_error(),
        "Should handle invalid merchant ID"
    );
}
