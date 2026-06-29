use std::collections::HashMap;

use super::cluster_key::derive_cluster_key;
use super::hypersense_client;
use super::hypersense_client::PspCost;
use super::{
    CostPickStrategy, MultiObjectiveInfo, MultiObjectiveOutcome, PspSummary, LAMBDA_COST,
    MAX_ECON_BAND, Z_NOISE,
};
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
    bucket_size: i32,
    margin: f64,
    strategy: CostPickStrategy,
) -> ReorderOutcome {
    if score_map.len() < 2 {
        let sr_head = current_head(score_map);
        return auth_won(
            sr_head,
            0.0,
            &HashMap::new(),
            score_map.len(),
            margin,
            "Only one PSP available; nothing to reorder.".to_string(),
        );
    }
    let cluster_key = derive_cluster_key(txn_detail, txn_card_info);
    let psps: Vec<String> = score_map.keys().cloned().collect();
    let costs = hypersense_client::lookup_costs(merchant_id, &cluster_key, &psps).await;
    reorder_for_cost(score_map, bucket_size, margin, &costs, strategy)
}

/// Standard error of a success-rate estimate over a moving window of `bucket_size` txns:
/// `√(p̂(1−p̂)/B)`. This is the auth band's noise floor and is **cost-independent** — a bad
/// cost number can never widen it.
fn sr_std_err(p: f64, bucket_size: i32) -> f64 {
    let b = bucket_size.max(1) as f64;
    (p * (1.0 - p) / b).max(0.0).sqrt()
}

/// Derived auth band + EV pick. The band is the **gate** (who may be promoted); EV is the
/// **pick** (which of the admitted is best). See `scratch/deriving-routing-config.md` §5.
pub fn reorder_for_cost(
    score_map: &HashMap<String, f64>,
    bucket_size: i32,
    margin: f64,
    costs: &HashMap<String, PspCost>,
    strategy: CostPickStrategy,
) -> ReorderOutcome {
    let sr_head = current_head(score_map);

    let best_auth = match sr_head.as_ref().map(|(_, a)| *a) {
        Some(a) if a.is_finite() => a,
        _ => {
            return auth_won(
                sr_head,
                0.0,
                costs,
                score_map.len(),
                margin,
                "SR head has no finite score; cannot evaluate cost.".to_string(),
            );
        }
    };

    let cost_bps = |gw: &str| -> f64 {
        match costs.get(gw) {
            Some(c) if c.available => c.effective_cost_bps,
            _ => f64::INFINITY,
        }
    };

    // Resolve the head PSP (lowest name on ties) and its cost.
    let head_psp = score_map
        .iter()
        .filter(|(_, s)| (**s - best_auth).abs() < f64::EPSILON)
        .map(|(gw, _)| gw.clone())
        .min();
    let head_cost = head_psp.as_deref().map(cost_bps).unwrap_or(f64::INFINITY);
    let head_floor_pp = Z_NOISE * (2.0_f64).sqrt() * sr_std_err(best_auth, bucket_size) * 100.0;

    // Cost is needed on both sides to compare expected value; without it the head wins.
    if !head_cost.is_finite() {
        return auth_won(
            sr_head,
            head_floor_pp,
            costs,
            score_map.len(),
            margin,
            "No cost data for the SR head; cannot evaluate a cost tradeoff.".to_string(),
        );
    }

    let sigma_head = sr_std_err(best_auth, bucket_size);
    // Expected profit per unit ticket (ticket cancels across PSPs): auth·(margin − cost).
    let ev = |score: f64, c_bps: f64| -> f64 { score * (margin - c_bps / 10_000.0) };
    // Per-candidate admission gate (fraction of auth-rate) and whether the candidate clears it.
    let gate_for = |score: f64, c_bps: f64| -> (bool, f64) {
        let gap = best_auth - score;
        let floor = Z_NOISE * (sigma_head.powi(2) + sr_std_err(score, bucket_size).powi(2)).sqrt();
        let econ = if c_bps.is_finite() && margin > 0.0 {
            // Cost may widen the band beyond the floor, but only up to MAX_ECON_BAND so a
            // wrong cost feed can't admit a measurably-worse PSP (circuit breaker).
            (LAMBDA_COST * ((head_cost - c_bps) / 10_000.0) / margin)
                .clamp(0.0, MAX_ECON_BAND)
        } else {
            0.0
        };
        let gate = floor.max(econ);
        (gap <= gate + f64::EPSILON, gate)
    };

    // Start from the head and only move on a strict EV improvement (no churn on ties).
    let head_ev = ev(best_auth, head_cost);
    let mut chosen_psp = head_psp.clone();
    let mut chosen_score = best_auth;
    let mut chosen_cost = head_cost;
    let mut chosen_ev = head_ev;
    let mut chosen_gate_pp = head_floor_pp;
    let mut admitted_count = 1usize; // head is always admitted

    for (gw, &score) in score_map.iter() {
        if Some(gw) == head_psp.as_ref() {
            continue;
        }
        let c = cost_bps(gw);
        let (admitted, gate) = gate_for(score, c);
        if !admitted {
            continue;
        }
        admitted_count += 1;
        // Eligible to *win* only with cost data (both EV and the cost compare need it).
        if !c.is_finite() {
            continue;
        }
        let cand_ev = ev(score, c);
        // The gate (who is admitted) is identical across strategies; only the pick differs.
        // MaxEv promotes on a strict expected-value gain; CheapestInBand promotes purely on
        // a strictly lower cost, ignoring the auth tradeoff (the §5.4 "band alone" pick).
        let improves = match strategy {
            CostPickStrategy::MaxEv => cand_ev > chosen_ev + f64::EPSILON,
            CostPickStrategy::CheapestInBand => c < chosen_cost - f64::EPSILON,
        };
        if improves {
            chosen_ev = cand_ev;
            chosen_psp = Some(gw.clone());
            chosen_score = score;
            chosen_cost = c;
            chosen_gate_pp = gate * 100.0;
        }
    }

    let head_summary = head_psp.as_ref().map(|psp| PspSummary {
        psp: psp.clone(),
        auth_rate: best_auth,
        cost_bps: Some(head_cost),
    });

    // Head still wins: auth objective held (no admitted alternative beat it on EV).
    if chosen_psp == head_psp {
        return ReorderOutcome {
            head_moved: false,
            info: MultiObjectiveInfo {
                outcome: MultiObjectiveOutcome::AuthWon,
                reason: format!(
                    "SR head retained — no admitted PSP beat it on expected value ({} within band).",
                    admitted_count
                ),
                tolerance_pp: head_floor_pp,
                sr_head: head_summary.clone(),
                chosen: head_summary,
                cost_saved_bps: None,
                qualified_count: admitted_count,
                margin,
            },
            cost_decision: None,
        };
    }

    let chosen_name = chosen_psp.clone().unwrap_or_default();
    let cost_saved_bps = head_cost - chosen_cost;
    let basis = match strategy {
        CostPickStrategy::MaxEv => "expected value",
        CostPickStrategy::CheapestInBand => "lowest cost in band",
    };
    let reason = format!(
        "Promoted '{}' over '{}' on {} — saves {:.2} bps for {:.2}pp auth, inside the {:.2}pp band.",
        chosen_name,
        head_psp.clone().unwrap_or_default(),
        basis,
        cost_saved_bps,
        (best_auth - chosen_score) * 100.0,
        chosen_gate_pp,
    );

    let fallbacks = build_ev_ordered_fallbacks(score_map, &chosen_name, &ev, &cost_bps);

    ReorderOutcome {
        head_moved: true,
        info: MultiObjectiveInfo {
            outcome: MultiObjectiveOutcome::CostWon,
            reason,
            tolerance_pp: chosen_gate_pp,
            sr_head: head_summary,
            chosen: Some(PspSummary {
                psp: chosen_name.clone(),
                auth_rate: chosen_score,
                cost_bps: Some(chosen_cost),
            }),
            cost_saved_bps: Some(cost_saved_bps),
            qualified_count: admitted_count,
            margin,
        },
        cost_decision: Some(CostDecision {
            chosen: chosen_name,
            fallbacks,
        }),
    }
}

/// Fallbacks after the chosen PSP: PSPs with cost data ordered by descending expected value
/// (best alternative first), then any PSPs without cost data ordered by descending auth.
fn build_ev_ordered_fallbacks(
    score_map: &HashMap<String, f64>,
    chosen: &str,
    ev: &dyn Fn(f64, f64) -> f64,
    cost_bps: &dyn Fn(&str) -> f64,
) -> Vec<String> {
    let mut with_cost: Vec<(String, f64)> = score_map
        .iter()
        .filter(|(gw, _)| gw.as_str() != chosen)
        .filter(|(gw, _)| cost_bps(gw).is_finite())
        .map(|(gw, score)| (gw.clone(), ev(*score, cost_bps(gw))))
        .collect();
    with_cost.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut without_cost: Vec<(String, f64)> = score_map
        .iter()
        .filter(|(gw, _)| gw.as_str() != chosen)
        .filter(|(gw, _)| !cost_bps(gw).is_finite())
        .map(|(gw, score)| (gw.clone(), *score))
        .collect();
    without_cost.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut out: Vec<String> = with_cost.into_iter().map(|(gw, _)| gw).collect();
    out.extend(without_cost.into_iter().map(|(gw, _)| gw));
    out
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
    margin: f64,
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
            margin,
        },
        cost_decision: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scores(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(g, s)| (g.to_string(), *s)).collect()
    }
    fn costs(pairs: &[(&str, f64)]) -> HashMap<String, PspCost> {
        pairs
            .iter()
            .map(|(g, c)| {
                (
                    g.to_string(),
                    PspCost {
                        available: true,
                        effective_cost_bps: *c,
                    },
                )
            })
            .collect()
    }

    // §5.5 walkthrough: B=200, margin 20%. Gate drops D (6pp worse); EV picks B over the
    // cheaper C (B's extra auth beats C's lower cost).
    #[test]
    fn walkthrough_gate_drops_d_and_ev_picks_b_over_cheaper_c() {
        let s = scores(&[("A", 0.910), ("B", 0.902), ("C", 0.885), ("D", 0.850)]);
        let c = costs(&[("A", 200.0), ("B", 150.0), ("C", 120.0), ("D", 110.0)]);
        let out = reorder_for_cost(&s, 200, 0.20, &c, CostPickStrategy::MaxEv);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::CostWon);
        // D never admitted (6pp gap exceeds its ~4.5pp economic + ~3pp floor gate).
        assert_eq!(out.info.qualified_count, 3, "A, B, C admitted; D rejected");
        let chosen = out.cost_decision.expect("cost decision").chosen;
        assert_eq!(chosen, "B", "EV should pick B, not the cheaper C or D");
    }

    // Same field/gate as above, but CheapestInBand picks the cheapest admitted PSP (C at
    // 120bps) rather than the max-EV one (B). D stays rejected — the gate is unchanged; only
    // the pick differs. This is the §5.4 "band alone" behavior, surfaced for comparison.
    #[test]
    fn cheapest_in_band_picks_cheaper_c_over_higher_ev_b() {
        let s = scores(&[("A", 0.910), ("B", 0.902), ("C", 0.885), ("D", 0.850)]);
        let c = costs(&[("A", 200.0), ("B", 150.0), ("C", 120.0), ("D", 110.0)]);
        let out = reorder_for_cost(&s, 200, 0.20, &c, CostPickStrategy::CheapestInBand);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::CostWon);
        assert_eq!(out.info.qualified_count, 3, "same gate: A, B, C admitted; D rejected");
        let chosen = out.cost_decision.expect("cost decision").chosen;
        assert_eq!(chosen, "C", "cheapest-in-band should pick C (120bps), not EV's B");
    }

    // A wrong (too-cheap) cost feed would flip EV to a much-worse PSP, but the
    // cost-independent noise floor rejects it. Head stays.
    #[test]
    fn bad_cost_cannot_breach_noise_floor() {
        // B is 5pp worse on auth; feed claims it's almost free.
        let s = scores(&[("A", 0.92), ("B", 0.87)]);
        let c = costs(&[("A", 180.0), ("B", 50.0)]);
        // The gate protects both strategies: even CheapestInBand can't reach a rejected PSP.
        for strategy in [CostPickStrategy::MaxEv, CostPickStrategy::CheapestInBand] {
            let out = reorder_for_cost(&s, 200, 0.20, &c, strategy);
            assert_eq!(
                out.info.outcome,
                MultiObjectiveOutcome::AuthWon,
                "5pp gap exceeds the ~3pp noise floor; B must be rejected regardless of cost"
            );
        }
    }

    // Head already best on EV → AuthWon, no churn.
    #[test]
    fn head_wins_when_no_admitted_alternative_beats_ev() {
        let s = scores(&[("A", 0.91), ("B", 0.905)]);
        let c = costs(&[("A", 100.0), ("B", 130.0)]); // B cheaper? no, pricier -> EV lower
        let out = reorder_for_cost(&s, 200, 0.20, &c, CostPickStrategy::MaxEv);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::AuthWon);
    }
}
