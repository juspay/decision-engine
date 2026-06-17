use std::collections::HashMap;

use super::cluster_key::derive_cluster_key;
use super::hypersense_client;
use super::hypersense_client::PspCost;
use super::{MultiObjectiveInfo, MultiObjectiveOutcome, PspSummary};
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::txn_details::types::TxnDetail;

pub struct CostDecision {
    pub chosen: String,
    pub fallbacks: Vec<String>,
}

pub struct ReorderOutcome {
    pub head_moved: bool,
    pub info: MultiObjectiveInfo,
    pub cost_decision: Option<CostDecision>,
}

pub async fn try_apply_multi_objective_post_step(
    score_map: &HashMap<String, f64>,
    merchant_id: &str,
    txn_detail: &TxnDetail,
    txn_card_info: &TxnCardInfo,
    tolerance_pp: f64,
) -> ReorderOutcome {
    if score_map.len() < 2 {
        let sr_head = current_head(score_map);
        return auth_won(
            sr_head,
            tolerance_pp,
            &HashMap::new(),
            score_map.len(),
            "Only one PSP available; nothing to reorder.".to_string(),
        );
    }
    let cluster_key = derive_cluster_key(txn_detail, txn_card_info);
    let psps: Vec<String> = score_map.keys().cloned().collect();
    let costs = hypersense_client::lookup_costs(merchant_id, &cluster_key, &psps).await;
    reorder_for_cost(score_map, tolerance_pp, &costs)
}

pub fn reorder_for_cost(
    score_map: &HashMap<String, f64>,
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

    let fallbacks = build_cost_ordered_fallbacks(score_map, &qualified, &cheapest_psp, &key_for);

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
        cost_decision: Some(CostDecision {
            chosen: cheapest_psp,
            fallbacks,
        }),
    }
}

fn build_cost_ordered_fallbacks(
    score_map: &HashMap<String, f64>,
    qualified: &[String],
    chosen: &str,
    key_for: &dyn Fn(&str) -> f64,
) -> Vec<String> {
    let mut by_cost: Vec<String> = qualified
        .iter()
        .filter(|gw| gw.as_str() != chosen)
        .cloned()
        .collect();
    by_cost.sort_by(|a, b| {
        key_for(a)
            .partial_cmp(&key_for(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let qualified_set: std::collections::HashSet<&str> =
        qualified.iter().map(|s| s.as_str()).collect();
    let mut by_auth: Vec<(String, f64)> = score_map
        .iter()
        .filter(|(gw, _)| !qualified_set.contains(gw.as_str()))
        .map(|(gw, score)| (gw.clone(), *score))
        .collect();
    by_auth.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    by_cost.extend(by_auth.into_iter().map(|(gw, _)| gw));
    by_cost
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
        cost_decision: None,
    }
}

