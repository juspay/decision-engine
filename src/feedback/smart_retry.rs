//! Smart retry: the connectors `/update-gateway-score` suggests for a retryable failure.

use crate::app::get_tenant_app_state;
use crate::decider::gatewaydecider::types::{GatewayScoringData, ScoreKeyType};
use crate::decider::gatewaydecider::utils as GU;
use crate::feedback::gateway_scoring_service::{update_gateway_score_lock, ScoringContextRead};
use crate::feedback::gateway_selection_scoring_v3 as GSSV3;
use crate::feedback::types::UpdateScorePayload;
use crate::feedback::utils::{self as Fbu, GatewayScoringType};
use crate::gsm::GatewayIdentifier;
use futures::stream::{self, StreamExt};

/// `gsm_info.retryConnectors`: eligible connectors minus the failed and already-tried ones, best SR first.
pub struct RetryConnectors<'a> {
    payload: &'a UpdateScorePayload,
}

impl<'a> RetryConnectors<'a> {
    /// For the failure `payload` reports.
    pub fn new(payload: &'a UpdateScorePayload) -> Self {
        Self { payload }
    }

    /// The connectors to retry on, from the payment's scoring `context`; empty without a context,
    /// eligible list, or anything left.
    pub async fn find(&self, context: &ScoringContextRead) -> Vec<String> {
        match context.found() {
            Some(scoring_data) => {
                let candidates = self.untried_eligible_connectors(scoring_data).await;
                if candidates.len() < 2 {
                    candidates
                } else {
                    Self::sort_best_first(
                        self.score_by_current_sr(scoring_data.clone(), candidates)
                            .await,
                    )
                }
            }
            None => Vec::new(),
        }
    }

    /// The eligible connectors other than the failed one and those already tried, in `/decide`'s order.
    async fn untried_eligible_connectors(&self, scoring_data: &GatewayScoringData) -> Vec<String> {
        let mut candidates = Vec::new();
        for gateway in scoring_data.eligibleConnectors.clone().unwrap_or_default() {
            if !Self::is_failed_gateway(&gateway, &self.payload.gateway)
                && !self.is_already_tried(&gateway).await
            {
                candidates.push(gateway);
            }
        }
        candidates
    }

    /// Whether `candidate` (`connector:mca`) is the reported gateway, with or without its mca id.
    fn is_failed_gateway(candidate: &str, reported: &str) -> bool {
        candidate == reported || (!reported.contains(':') && candidate.connector_name() == reported)
    }

    /// Whether this payment was already scored on `gateway`, i.e. its score lock exists.
    async fn is_already_tried(&self, gateway: &str) -> bool {
        let app_state = get_tenant_app_state().await;
        let redis = &app_state.redis_conn;
        // Stops at the first lock found.
        stream::iter(self.score_lock_keys(gateway))
            .any(|lock| async move { matches!(redis.key_exists(&lock).await, Ok(true)) })
            .await
    }

    /// Every score lock an update of this payment on `gateway` may have set, with or without its mca id.
    fn score_lock_keys(&self, gateway: &str) -> Vec<String> {
        let names = std::iter::once(gateway)
            .chain((gateway != gateway.connector_name()).then(|| gateway.connector_name()));
        names
            .flat_map(|name| {
                [
                    GatewayScoringType::Reward,
                    GatewayScoringType::PenaliseSrv3,
                    GatewayScoringType::Penalise,
                ]
                .into_iter()
                .map(move |kind| {
                    update_gateway_score_lock(
                        kind,
                        &self.payload.merchant_id,
                        &self.payload.payment_id,
                        name,
                    )
                })
            })
            .collect()
    }

    /// Each candidate's current SR, from the same SR key and bucket as scoring uses.
    async fn score_by_current_sr(
        &self,
        scoring_data: GatewayScoringData,
        candidates: Vec<String>,
    ) -> Vec<(String, f64)> {
        match Fbu::get_txn_detail_from_api_payload(self.payload.clone(), scoring_data.clone()) {
            Ok(txn_detail) => {
                self.score_with_sr_keys(txn_detail, scoring_data, candidates)
                    .await
            }
            // Unranked, in `/decide`'s order.
            Err(_) => candidates
                .into_iter()
                .map(|gateway| (gateway, 1.0))
                .collect(),
        }
    }

    /// Reads each candidate's SR from its SR v3 key in the merchant's bucket.
    async fn score_with_sr_keys(
        &self,
        txn_detail: crate::types::txn_details::types::TxnDetail,
        scoring_data: GatewayScoringData,
        candidates: Vec<String>,
    ) -> Vec<(String, f64)> {
        let txn_card_info =
            Fbu::get_txn_card_info_from_api_payload(self.payload.clone(), scoring_data.clone());
        let bucket_size = GSSV3::flow::getSrV3MerchantBucketSize(txn_detail, txn_card_info).await;
        let keys = GU::get_unified_key(
            scoring_data,
            None,
            ScoreKeyType::SrV3Key,
            false,
            candidates
                .iter()
                .map(|gateway| (gateway.clone(), None))
                .collect(),
        )
        .await;
        let mut scored = Vec::with_capacity(candidates.len());
        for gateway in candidates {
            let score = match keys.get(&gateway) {
                Some(key) => {
                    crate::decider::gatewaydecider::gw_scoring::get_score_from_redis(
                        bucket_size,
                        key,
                    )
                    .await
                }
                None => 1.0,
            };
            scored.push((gateway, score));
        }
        scored
    }

    /// Best score first; ties keep the order `/decide` listed them in.
    fn sort_best_first(mut scored: Vec<(String, f64)>) -> Vec<String> {
        scored.sort_by(|(_, left), (_, right)| right.total_cmp(left));
        scored.into_iter().map(|(gateway, _)| gateway).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::RetryConnectors;

    // The reported gateway is excluded whether or not it carries the merchant connector id.
    #[test]
    fn the_reported_gateway_is_recognised_with_or_without_its_id() {
        assert!(RetryConnectors::is_failed_gateway(
            "checkout:mca_cko",
            "checkout:mca_cko"
        ));
        assert!(RetryConnectors::is_failed_gateway(
            "checkout:mca_cko",
            "checkout"
        ));
        assert!(!RetryConnectors::is_failed_gateway(
            "checkout:mca_cko",
            "checkout:mca_other"
        ));
        assert!(!RetryConnectors::is_failed_gateway(
            "worldpay:mca_wp",
            "checkout"
        ));
    }

    // Best current score first; equal scores keep the order /decide listed them in.
    #[test]
    fn retry_connectors_are_ranked_best_first() {
        let ranked = RetryConnectors::sort_best_first(vec![
            ("adyen:mca_ad".to_string(), 0.90),
            ("worldpay:mca_wp".to_string(), 0.99),
            ("stripe:mca_st".to_string(), 0.90),
        ]);
        assert_eq!(ranked, ["worldpay:mca_wp", "adyen:mca_ad", "stripe:mca_st"]);
    }
}
