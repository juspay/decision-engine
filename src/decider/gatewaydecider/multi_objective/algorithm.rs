use std::collections::HashMap;

use super::cluster_key::derive_cluster_key;
use super::hypersense_client;
use super::hypersense_client::PspCost;
use super::{MultiObjectiveInfo, MultiObjectiveOutcome, PspSummary};
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::txn_details::types::TxnDetail;

/// Bump given to the cost-winner's score. Tiny so we don't pollute the score
/// magnitude (downstream observability still sees ~the original SR score).
const PROMOTION_EPSILON: f64 = 1e-6;

pub struct ReorderOutcome {
    pub head_moved: bool,
    pub info: MultiObjectiveInfo,
}

pub async fn try_apply_multi_objective_post_step(
    score_map: &mut HashMap<String, f64>,
    merchant_id: &str,
    txn_detail: &TxnDetail,
    txn_card_info: &TxnCardInfo,
    tolerance_pp: f64,
) -> MultiObjectiveInfo {
    if score_map.len() < 2 {
        let sr_head = current_head(score_map);
        return auth_won(
            sr_head,
            tolerance_pp,
            &HashMap::new(),
            score_map.len(),
            "Only one PSP available; nothing to reorder.".to_string(),
        )
        .info;
    }
    let cluster_key = derive_cluster_key(txn_detail, txn_card_info);
    let psps: Vec<String> = score_map.keys().cloned().collect();
    let costs = hypersense_client::lookup_costs(merchant_id, &cluster_key, &psps).await;
    reorder_for_cost(score_map, tolerance_pp, &costs).info
}

pub fn reorder_for_cost(
    score_map: &mut HashMap<String, f64>,
    tolerance_pp: f64,
    costs: &HashMap<String, PspCost>,
) -> ReorderOutcome {
    let sr_head = current_head(score_map);

    let best_auth = match sr_head.as_ref().map(|(_, a)| *a) {
        Some(a) if a.is_finite() => a,
        _ => {
            return auth_won(
                sr_head,
                tolerance_pp,
                costs,
                score_map.len(),
                "SR head has no finite score; cannot evaluate cost.".to_string(),
            );
        }
    };

    let cutoff = best_auth - tolerance_pp;
    let qualified: Vec<String> = score_map
        .iter()
        .filter(|(_, score)| **score >= cutoff)
        .map(|(gw, _)| gw.clone())
        .collect();

    // Need at least 2 qualified PSPs to make a cost-based tradeoff, otherwise SR head wins by default
    if qualified.len() < 2 {
        return auth_won(
            sr_head,
            tolerance_pp,
            costs,
            qualified.len(),
            format!(
                "Only {} PSP qualified under the {:.2} pp band; no alternative to consider.",
                qualified.len(),
                tolerance_pp
            ),
        );
    }

    let key_for = |gw: &str| -> f64 {
        match costs.get(gw) {
            Some(c) if c.available => c.effective_cost_bps,
            _ => f64::INFINITY,
        }
    };

    let cheapest_psp = qualified
        .iter()
        .min_by(|a, b| {
            key_for(a)
                .partial_cmp(&key_for(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
        .expect("qualified has >= 2 elements by prior guard");

    let cheapest_cost = key_for(&cheapest_psp);
    if !cheapest_cost.is_finite() {
        return auth_won(
            sr_head,
            tolerance_pp,
            costs,
            qualified.len(),
            format!(
                "No cost data available for any of the {} qualified PSPs.",
                qualified.len()
            ),
        );
    }

    let sr_head_psp = score_map
        .iter()
        .filter(|(_, s)| (**s - best_auth).abs() < f64::EPSILON)
        .map(|(gw, _)| gw.clone())
        .min();

    let sr_head_cost = sr_head_psp.as_deref().map(key_for);

    if sr_head_psp.as_deref() == Some(cheapest_psp.as_str()) {
        return auth_won(
            sr_head,
            tolerance_pp,
            costs,
            qualified.len(),
            format!(
                "SR head '{}' was already the cheapest among {} qualifiers.",
                cheapest_psp,
                qualified.len()
            ),
        );
    }

    let cheapest_auth = *score_map.get(&cheapest_psp).unwrap_or(&best_auth);
    let sr_head_summary = sr_head_psp.as_ref().map(|psp| PspSummary {
        psp: psp.clone(),
        auth_rate: best_auth,
        cost_bps: sr_head_cost.filter(|c| c.is_finite()),
    });
    let chosen_summary = PspSummary {
        psp: cheapest_psp.clone(),
        auth_rate: cheapest_auth,
        cost_bps: Some(cheapest_cost),
    };
    let cost_saved_bps = sr_head_cost
        .filter(|c| c.is_finite())
        .map(|sr| sr - cheapest_cost);

    if let Some(entry) = score_map.get_mut(&cheapest_psp) {
        *entry = best_auth + PROMOTION_EPSILON;
    }

    let reason = match cost_saved_bps {
        Some(saved) => format!(
            "Promoted '{}' over '{}' — saves {:.2} bps within the {:.2} pp band.",
            cheapest_psp,
            sr_head_psp.unwrap_or_default(),
            saved,
            tolerance_pp
        ),
        None => format!(
            "Promoted '{}' over SR head (cheaper within the {:.2} pp band).",
            cheapest_psp, tolerance_pp
        ),
    };

    ReorderOutcome {
        head_moved: true,
        info: MultiObjectiveInfo {
            outcome: MultiObjectiveOutcome::CostWon,
            reason,
            tolerance_pp,
            sr_head: sr_head_summary,
            chosen: Some(chosen_summary),
            cost_saved_bps,
            qualified_count: qualified.len(),
        },
    }
}

fn current_head(score_map: &HashMap<String, f64>) -> Option<(String, f64)> {
    score_map
        .iter()
        .fold(None, |acc: Option<(String, f64)>, (psp, score)| match acc {
            None => Some((psp.clone(), *score)),
            Some((_, best)) if *score > best => Some((psp.clone(), *score)),
            Some(_) => acc,
        })
}

fn auth_won(
    sr_head: Option<(String, f64)>,
    tolerance_pp: f64,
    costs: &HashMap<String, PspCost>,
    qualified_count: usize,
    reason: String,
) -> ReorderOutcome {
    let summary = sr_head.map(|(psp, auth_rate)| {
        let cost_bps = costs.get(&psp).and_then(|c| {
            if c.available {
                Some(c.effective_cost_bps)
            } else {
                None
            }
        });
        PspSummary {
            psp,
            auth_rate,
            cost_bps,
        }
    });
    ReorderOutcome {
        head_moved: false,
        info: MultiObjectiveInfo {
            outcome: MultiObjectiveOutcome::AuthWon,
            reason,
            tolerance_pp,
            sr_head: summary.clone(),
            chosen: summary,
            cost_saved_bps: None,
            qualified_count,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MultiObjectiveOutcome, PspSummary};
    use super::*;

    fn cost(bps: f64) -> PspCost {
        PspCost {
            available: true,
            effective_cost_bps: bps,
        }
    }

    fn unavailable() -> PspCost {
        PspCost {
            available: false,
            effective_cost_bps: 0.0,
        }
    }

    fn map(entries: &[(&str, f64)]) -> HashMap<String, f64> {
        entries
            .iter()
            .map(|(g, s)| ((*g).to_string(), *s))
            .collect()
    }

    fn costs(entries: &[(&str, PspCost)]) -> HashMap<String, PspCost> {
        entries
            .iter()
            .map(|(g, c)| ((*g).to_string(), c.clone()))
            .collect()
    }

    #[test]
    fn no_qualifiers_is_noop() {
        let mut m = map(&[("a", 0.9), ("b", 0.7)]);
        let c = costs(&[("a", cost(20.0)), ("b", cost(10.0))]);
        let out = reorder_for_cost(&mut m, 5.0, &c);
        assert!(!out.head_moved);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::AuthWon);
        assert!(out.info.reason.contains("Only 1 PSP qualified"));
    }

    #[test]
    fn cheaper_in_band_psp_promoted() {
        let mut m = map(&[("a", 0.874), ("b", 0.869)]);
        let c = costs(&[("a", cost(20.0)), ("b", cost(16.0))]);
        let out = reorder_for_cost(&mut m, 0.8, &c);
        assert!(out.head_moved);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::CostWon);
        assert_eq!(out.info.cost_saved_bps, Some(4.0));
        let chosen: PspSummary = out.info.chosen.unwrap();
        assert_eq!(chosen.psp, "b");
        let sr_head: PspSummary = out.info.sr_head.unwrap();
        assert_eq!(sr_head.psp, "a");
        let head = m
            .iter()
            .max_by(|x, y| x.1.partial_cmp(y.1).unwrap())
            .unwrap();
        assert_eq!(head.0, "b");
    }

    #[test]
    fn auth_wins_when_runner_up_out_of_band() {
        let mut m = map(&[("a", 0.912), ("b", 0.881)]);
        let c = costs(&[("a", cost(20.0)), ("b", cost(16.0))]);
        let out = reorder_for_cost(&mut m, 0.8, &c);
        assert!(!out.head_moved);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::AuthWon);
        assert!(out.info.reason.contains("Only 1 PSP qualified"));
    }

    #[test]
    fn missing_cost_psp_sorts_last() {
        let mut m = map(&[("a", 0.9), ("b", 0.895)]);
        let c = costs(&[("b", cost(10.0))]);
        let out = reorder_for_cost(&mut m, 5.0, &c);
        assert!(out.head_moved);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::CostWon);
    }

    #[test]
    fn unavailable_psp_sorts_last() {
        let mut m = map(&[("a", 0.9), ("b", 0.895)]);
        let c = costs(&[("a", unavailable()), ("b", cost(10.0))]);
        let out = reorder_for_cost(&mut m, 5.0, &c);
        assert!(out.head_moved);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::CostWon);
    }

    #[test]
    fn all_qualifiers_missing_cost_is_noop() {
        let mut m = map(&[("a", 0.9), ("b", 0.895)]);
        let out = reorder_for_cost(&mut m, 5.0, &HashMap::new());
        assert!(!out.head_moved);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::AuthWon);
        assert!(out.info.reason.contains("No cost data"));
    }

    #[test]
    fn sr_head_already_cheapest_is_noop() {
        let mut m = map(&[("a", 0.9), ("b", 0.895)]);
        let c = costs(&[("a", cost(10.0)), ("b", cost(20.0))]);
        let out = reorder_for_cost(&mut m, 5.0, &c);
        assert!(!out.head_moved);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::AuthWon);
        assert!(out.info.reason.contains("already the cheapest"));
    }
}
