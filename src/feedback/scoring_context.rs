//! Owner-checked scoring context shared by both feedback APIs.
//!
//! The Redis key and lock formats deliberately stay compatible with existing writers. Contexts
//! still share the payment-id namespace; an overwritten context must fail closed instead of
//! allowing one merchant's outcome to update another merchant's scoring dimensions.

use crate::auth::AuthorizedMerchant;
use crate::decider::gatewaydecider::types::{ErrorResponse, GatewayScoringData, UnifiedError};

/// The existing on-wire key format, shared by every context writer and reader.
pub fn key(payment_id: &str) -> String {
    format!("{}{}", super::constants::GATEWAY_SCORING_DATA, payment_id)
}

/// A snapshot loaded once and checked against the request's authorized merchant. Passing this
/// through the worker prevents a later Redis read from consuming a replaced, foreign context.
pub struct FeedbackContext {
    merchant: AuthorizedMerchant,
    payment_id: String,
    pub(super) data: Option<GatewayScoringData>,
}

impl FeedbackContext {
    fn from_record(
        merchant: AuthorizedMerchant,
        payment_id: &str,
        data: Option<GatewayScoringData>,
    ) -> Result<Self, ErrorResponse> {
        if data
            .as_ref()
            .is_some_and(|record| record.merchantId != merchant.as_str())
        {
            return Err(owner_mismatch());
        }
        Ok(Self {
            merchant,
            payment_id: payment_id.to_string(),
            data,
        })
    }

    pub(super) fn ensure_request(
        &self,
        merchant_id: &str,
        payment_id: &str,
    ) -> Result<(), ErrorResponse> {
        self.ensure_merchant(merchant_id)?;
        if payment_id != self.payment_id {
            return Err(super::gateway_scoring_service::invalid_request_error(
                "scoring context",
                &"Feedback payment does not match the loaded context",
            ));
        }
        Ok(())
    }

    fn ensure_merchant(&self, merchant_id: &str) -> Result<(), ErrorResponse> {
        if merchant_id != self.merchant.as_str() {
            return Err(owner_mismatch());
        }
        Ok(())
    }
}

fn owner_mismatch() -> ErrorResponse {
    ErrorResponse {
        status: "403".to_string(),
        error_code: "SCORING_CONTEXT_MERCHANT_MISMATCH".to_string(),
        error_message: "Scoring context does not belong to the authorized merchant".to_string(),
        priority_logic_tag: None,
        routing_approach: None,
        filter_wise_gateways: None,
        error_info: UnifiedError {
            code: "SCORING_CONTEXT_MERCHANT_MISMATCH".to_string(),
            user_message: "Scoring context does not belong to the authorized merchant".to_string(),
            developer_message: "Feedback and stored context must have the same merchant"
                .to_string(),
        },
        priority_logic_output: None,
        is_dynamic_mga_enabled: false,
    }
}

pub async fn load(
    merchant: AuthorizedMerchant,
    payment_id: &str,
) -> Result<FeedbackContext, ErrorResponse> {
    let state = crate::app::get_tenant_app_state().await;
    let redis_key = key(payment_id);
    let record = match state
        .redis_conn
        .get_key(&redis_key, "GatewayScoringData")
        .await
    {
        Ok(data) => Some(data),
        Err(error) => {
            // Compression may report a missing key as GetFailed. A decode/read failure is not
            // a missing context: confirm absence before preserving the no-context skip path.
            let absent = match error.current_context() {
                redis_interface::errors::RedisError::NotFound => true,
                redis_interface::errors::RedisError::GetFailed => {
                    matches!(state.redis_conn.key_exists(&redis_key).await, Ok(false))
                }
                _ => false,
            };
            if !absent {
                return Err(super::gateway_scoring_service::invalid_request_error(
                    "scoring context",
                    &error,
                ));
            }
            None
        }
    };
    FeedbackContext::from_record(merchant, payment_id, record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(merchant: &str) -> GatewayScoringData {
        serde_json::from_value(serde_json::json!({
            "merchantId": merchant, "paymentMethod": "card", "paymentMethodType": "credit",
            "orderType": "UNKNOWN", "isPaymentSourceEnabledForSrRouting": false,
            "isAuthLevelEnabledForSrRouting": false, "isBankLevelEnabledForSrRouting": false,
            "isGriEnabledForElimination": false, "isGriEnabledForSrRouting": false,
            "dateCreated": [2026, 287, 0, 0, 0, 0, 0, 0, 0], "eliminationEnabled": false,
            "currency": "USD", "is_legacy_decider_flow": false
        }))
        .expect("valid scoring context fixture")
    }

    #[test]
    fn a_payment_id_collision_cannot_use_another_merchants_context() {
        let result = FeedbackContext::from_record(
            AuthorizedMerchant::internal("merchant_a"),
            "payment_123",
            Some(record("merchant_b")),
        );
        assert!(matches!(result, Err(error) if error.status == "403"
            && error.error_code == "SCORING_CONTEXT_MERCHANT_MISMATCH"));
    }

    #[test]
    fn the_validated_snapshot_keeps_its_owner_when_redis_is_replaced() {
        let mut stored = record("merchant_a");
        let context = FeedbackContext::from_record(
            AuthorizedMerchant::internal("merchant_a"),
            "payment_123",
            Some(stored.clone()),
        );
        stored.merchantId = "merchant_b".to_string();
        assert_eq!(stored.merchantId, "merchant_b");
        assert!(context.is_ok_and(|context| {
            context.ensure_merchant("merchant_a").is_ok()
                && context.ensure_merchant("merchant_b").is_err()
                && context
                    .data
                    .is_some_and(|record| record.merchantId == "merchant_a")
        }));
    }

    #[test]
    fn a_missing_context_still_binds_feedback_to_the_authorized_merchant() {
        let context = FeedbackContext::from_record(
            AuthorizedMerchant::internal("merchant_a"),
            "payment_123",
            None,
        );
        assert!(context.is_ok_and(|context| context.data.is_none()
            && context.ensure_merchant("merchant_a").is_ok()
            && context.ensure_merchant("merchant_b").is_err()));
    }

    #[test]
    fn existing_context_keys_remain_readable_during_rollout() {
        assert_eq!(key("payment_123"), "gateway_scoring_data_payment_123");
    }

    fn feedback(merchant: &str) -> super::super::types::UpdateScorePayload {
        super::super::types::UpdateScorePayload {
            merchant_id: merchant.to_string(),
            gateway: "stripe".to_string(),
            gateway_reference_id: None,
            status: crate::types::txn_details::types::TxnStatus::Charged,
            payment_id: "payment_123".to_string(),
            enforce_dynamic_routing_failure: None,
            txn_latency: None,
            error_info: None,
            is_smart_retry: None,
        }
    }

    #[tokio::test]
    async fn feedback_without_context_preserves_the_skip_path() {
        let result = match FeedbackContext::from_record(
            AuthorizedMerchant::internal("merchant_a"),
            "payment_123",
            None,
        ) {
            Ok(context) => {
                super::super::gateway_scoring_service::check_and_update_gateway_score_(
                    feedback("merchant_a"),
                    context,
                )
                .await
            }
            Err(error) => Err(error),
        };
        assert!(matches!(result, Ok(outcome)
            if outcome == super::super::gateway_scoring_service::SCORE_UPDATE_SKIPPED));
    }

    #[tokio::test]
    async fn a_different_payload_merchant_is_rejected_before_gsm_or_redis_scoring() {
        let result = match FeedbackContext::from_record(
            AuthorizedMerchant::internal("merchant_a"),
            "payment_123",
            None,
        ) {
            Ok(context) => {
                super::super::gateway_scoring_service::check_and_update_gateway_score_(
                    feedback("merchant_b"),
                    context,
                )
                .await
            }
            Err(error) => Err(error),
        };
        assert!(matches!(result, Err(error)
            if error.error_code == "SCORING_CONTEXT_MERCHANT_MISMATCH"));
    }

    #[tokio::test]
    async fn a_different_payment_is_rejected_before_the_missing_context_skip() {
        let mut payload = feedback("merchant_a");
        payload.payment_id = "payment_456".to_string();
        let result = match FeedbackContext::from_record(
            AuthorizedMerchant::internal("merchant_a"),
            "payment_123",
            None,
        ) {
            Ok(context) => {
                super::super::gateway_scoring_service::check_and_update_gateway_score_(
                    payload, context,
                )
                .await
            }
            Err(error) => Err(error),
        };
        assert!(matches!(result, Err(error) if error.error_code == "INVALID_REQUEST"));
    }
}
