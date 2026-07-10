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
    margin: f64,
) -> ReorderOutcome {
    if score_map.len() < 2 {
        let sr_head = current_head(score_map);
        return auth_won(
            sr_head,
            &HashMap::new(),
            score_map.len(),
            margin,
            "Only one PSP available; nothing to reorder.".to_string(),
        );
    }
    let cluster_key = derive_cluster_key(txn_detail, txn_card_info);
    let psps: Vec<String> = score_map.keys().cloned().collect();
    let costs = hypersense_client::lookup_costs(merchant_id, &cluster_key, &psps).await;
    reorder_for_cost(score_map, margin, &costs)
}

/// Pure expected-value pick: rank every PSP that has cost data by
/// `EV = auth·(margin − cost/10_000)` and promote the highest. There is **no explicit
/// auth band** and no admission gate — a PSP wins purely on expected value.
///
pub fn reorder_for_cost(
    score_map: &HashMap<String, f64>,
    margin: f64,
    costs: &HashMap<String, PspCost>,
) -> ReorderOutcome {
    let sr_head = current_head(score_map);

    let best_auth = match sr_head.as_ref().map(|(_, a)| *a) {
        Some(a) if a.is_finite() => a,
        _ => {
            return auth_won(
                sr_head,
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

    // Cost is needed to place the head in the EV ranking; without it we keep the head.
    if !head_cost.is_finite() {
        return auth_won(
            sr_head,
            costs,
            score_map.len(),
            margin,
            "No cost data for the SR head; cannot rank it on expected value.".to_string(),
        );
    }

    // Expected profit per unit ticket (ticket cancels across PSPs): auth·(margin − cost).
    let ev = |score: f64, c_bps: f64| -> f64 { score * (margin - c_bps / 10_000.0) };

    // Start from the head and move to any PSP with a strictly better EV (no churn on ties).
    let head_ev = ev(best_auth, head_cost);
    // EVs of every PSP we can rank (i.e. that has cost data), so we can report the
    // top-two EV gap — the margin of victory of the winning pick.
    let mut ranked_evs: Vec<f64> = vec![head_ev];
    let mut chosen_psp = head_psp.clone();
    let mut chosen_score = best_auth;
    let mut chosen_cost = head_cost;
    let mut chosen_ev = head_ev;
    let mut ranked_count = 1usize; // head is always ranked

    for (gw, &score) in score_map.iter() {
        if Some(gw) == head_psp.as_ref() {
            continue;
        }
        let c = cost_bps(gw);
        // No cost data ⇒ no EV ⇒ this PSP can't be ranked or chosen.
        if !c.is_finite() {
            continue;
        }
        ranked_count += 1;
        let cand_ev = ev(score, c);
        ranked_evs.push(cand_ev);
        // Promote on a strict expected-value gain (no churn on ties).
        if cand_ev > chosen_ev + f64::EPSILON {
            chosen_ev = cand_ev;
            chosen_psp = Some(gw.clone());
            chosen_score = score;
            chosen_cost = c;
        }
    }

    let ev_gap_top2 = top2_gap(ranked_evs);

    let head_summary = head_psp
        .as_ref()
        .map(|psp| make_summary(psp.clone(), best_auth, Some(head_cost), costs));

    // Head still wins: it was already the highest-EV PSP.
    if chosen_psp == head_psp {
        return ReorderOutcome {
            head_moved: false,
            info: MultiObjectiveInfo {
                outcome: MultiObjectiveOutcome::AuthWon,
                reason: format!(
                    "SR head retained — it is the highest expected-value PSP ({} ranked on EV).",
                    ranked_count
                ),
                sr_head: head_summary.clone(),
                chosen: head_summary,
                cost_saved_bps: None,
                qualified_count: ranked_count,
                margin,
                ev_gap_top2,
            },
            cost_decision: None,
        };
    }

    let chosen_name = chosen_psp.clone().unwrap_or_default();
    let cost_saved_bps = head_cost - chosen_cost;
    let reason = format!(
        "Promoted '{}' over '{}' on expected value — saves {:.2} bps for {:.2}pp auth.",
        chosen_name,
        head_psp.clone().unwrap_or_default(),
        cost_saved_bps,
        (best_auth - chosen_score) * 100.0,
    );

    let fallbacks = build_ev_ordered_fallbacks(score_map, &chosen_name, &ev, &cost_bps);

    ReorderOutcome {
        head_moved: true,
        info: MultiObjectiveInfo {
            outcome: MultiObjectiveOutcome::CostWon,
            reason,
            sr_head: head_summary,
            chosen: Some(make_summary(
                chosen_name.clone(),
                chosen_score,
                Some(chosen_cost),
                costs,
            )),
            cost_saved_bps: Some(cost_saved_bps),
            qualified_count: ranked_count,
            margin,
            ev_gap_top2,
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
        make_summary(psp, auth_rate, cost_bps, costs)
    });
    ReorderOutcome {
        head_moved: false,
        info: MultiObjectiveInfo {
            outcome: MultiObjectiveOutcome::AuthWon,
            reason,
            sr_head: summary.clone(),
            chosen: summary,
            cost_saved_bps: None,
            qualified_count,
            margin,
            // These paths bail before any EV ranking (head has no finite score, or no
            // cost data for the head), so there is no top-two EV gap to report.
            ev_gap_top2: None,
        },
        cost_decision: None,
    }
}

/// Build a `PspSummary`, pulling the cost source and fitted breakdown (pct/fixed/ic_category) from
/// the PSP's cost entry so callers can see *which* model priced it.
fn make_summary(
    psp: String,
    auth_rate: f64,
    cost_bps: Option<f64>,
    costs: &HashMap<String, PspCost>,
) -> PspSummary {
    let c = costs.get(&psp);
    PspSummary {
        psp,
        auth_rate,
        cost_bps,
        cost_source: c.map(|c| c.source),
        cost_model: c.and_then(|c| c.cost_model.clone()),
    }
}

/// Gap between the two largest values (`max − second_max`), or `None` with fewer than
/// two entries. Used to report the EV margin between the top-two eligible PSPs.
fn top2_gap(mut evs: Vec<f64>) -> Option<f64> {
    if evs.len() < 2 {
        return None;
    }
    evs.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    Some(evs[0] - evs[1])
}

#[cfg(test)]
mod tests {
    use super::super::CostSource;
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
                        source: CostSource::InHouse,
                        cost_model: None,
                    },
                )
            })
            .collect()
    }

    // §5.5 walkthrough: B=200, margin 20%. Gate drops D (6pp worse); EV picks B over the
    // cheaper C (B's extra auth beats C's lower cost).
    // Pure EV over all four PSPs (no band, no gate). EV = auth·(0.20 − cost/10_000):
    // A .1638, B .16687, C .16638, D .16065. B is highest → promoted over head A.
    #[test]
    fn ev_picks_highest_ev_over_head_and_cheaper_psps() {
        let s = scores(&[("A", 0.910), ("B", 0.902), ("C", 0.885), ("D", 0.850)]);
        let c = costs(&[("A", 200.0), ("B", 150.0), ("C", 120.0), ("D", 110.0)]);
        let out = reorder_for_cost(&s, 0.20, &c);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::CostWon);
        // No gate: every PSP with cost data is ranked, including D.
        assert_eq!(out.info.qualified_count, 4, "all four PSPs ranked on EV");
        let chosen = out.cost_decision.expect("cost decision").chosen;
        assert_eq!(chosen, "B", "EV should pick B, not the cheaper C or D");
    }

    // The old noise-floor circuit breaker is gone: pure EV promotes a much-worse-auth PSP
    // whenever its cost makes its EV higher. B is 5pp below A on auth but far cheaper —
    // EV_A .16744 < EV_B .16965 → B wins.
    #[test]
    fn pure_ev_promotes_far_worse_auth_when_ev_wins() {
        let s = scores(&[("A", 0.92), ("B", 0.87)]);
        let c = costs(&[("A", 180.0), ("B", 50.0)]);
        let out = reorder_for_cost(&s, 0.20, &c);
        assert_eq!(
            out.info.outcome,
            MultiObjectiveOutcome::CostWon,
            "no band gate now — B's higher EV wins"
        );
        assert_eq!(out.cost_decision.expect("cost decision").chosen, "B");
    }

    // Head already highest on EV → AuthWon, no churn.
    #[test]
    fn head_wins_when_it_is_highest_ev() {
        let s = scores(&[("A", 0.91), ("B", 0.905)]);
        let c = costs(&[("A", 100.0), ("B", 130.0)]); // B pricier -> EV lower
        let out = reorder_for_cost(&s, 0.20, &c);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::AuthWon);
    }

    // ev_gap_top2 = EV(#1) − EV(#2) over every PSP ranked on EV, reported on both
    // outcomes. EV = auth·(margin − cost/10_000), margin 20%.
    #[test]
    fn ev_gap_top2_reports_margin_of_victory() {
        // All four ranked; EVs B .16687 > C .16638 > A .1638 > D .16065. Top-two gap = B − C.
        let s = scores(&[("A", 0.910), ("B", 0.902), ("C", 0.885), ("D", 0.850)]);
        let c = costs(&[("A", 200.0), ("B", 150.0), ("C", 120.0), ("D", 110.0)]);
        let out = reorder_for_cost(&s, 0.20, &c);
        assert_eq!(out.info.outcome, MultiObjectiveOutcome::CostWon);
        let gap = out.info.ev_gap_top2.expect("PSPs ranked on EV");
        assert!(
            (gap - 0.00049).abs() < 1e-6,
            "top-two gap should be B−C; got {gap}"
        );

        // AuthWon still reports the head's EV lead over the runner-up (A − B).
        let s2 = scores(&[("A", 0.91), ("B", 0.905)]);
        let c2 = costs(&[("A", 100.0), ("B", 130.0)]);
        let out2 = reorder_for_cost(&s2, 0.20, &c2);
        assert_eq!(out2.info.outcome, MultiObjectiveOutcome::AuthWon);
        let gap2 = out2.info.ev_gap_top2.expect("A, B ranked on EV");
        assert!(
            (gap2 - 0.003665).abs() < 1e-6,
            "AuthWon gap should be A−B; got {gap2}"
        );

        // Only one PSP has cost data → nothing to rank a second place against → None.
        let s3 = scores(&[("A", 0.90)]);
        let c3 = costs(&[("A", 100.0)]);
        let out3 = reorder_for_cost(&s3, 0.20, &c3);
        assert_eq!(
            out3.info.ev_gap_top2, None,
            "single eligible PSP has no top-two gap"
        );
    }
}
