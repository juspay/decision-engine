//! Local, deterministic cost seeds for the multi-objective simulator.
//!
//! In production the candidate PSP costs come from the live Hypersense fee-rate API. For
//! the Decision Simulator we want costs that are *offline, repeatable, and economically
//! realistic* so the auth-vs-cost tradeoff can be demonstrated without a network call.
//!
//! The pricing itself is **config-driven** (`config.hypersense.seed_costs`), not hardcoded.
//! Each PSP entry is a `default` fee plus optional `tiers` that override it by card network,
//! funding type (credit/debit), card program, and/or transaction currency — so new networks
//! (amex, discover…), programs (commercial, corporate…), funding types, and currencies are
//! added by editing config, no code change. This reproduces the two real-world US credit
//! pricing models out of the box:
//!
//! * **Adyen → Interchange++ (IC++).** Real pass-through cost (`interchange + scheme +
//!   markup`) that *varies by card scenario* — a premium card costs more than a standard
//!   one, debit far less than credit, EUR (capped) far less than US.
//! * **Stripe → blended.** One flat rate for every card (just a `default`, no tiers).
//!
//! Each model is the same amount-independent `{pct_bps, fixed}` shape the live fee cache
//! solves: `effective_cost_bps = pct_bps + fixed/amount·10_000`.
//!
//! **Precedence:** a tier applies only if *every* field it specifies matches (all by
//! case-insensitive equality); among the applicable ones, the tier matching the **most**
//! fields wins, ties broken by the more discriminating dimension (currency > network >
//! funding > program). When no tier matches, the entry's `default` is used.
//!
//! **Region is approximated by `transaction_currency`** (USD ⇒ US, EUR ⇒ EEA) — the only
//! geography signal available at decide time. A true `region` / issuer-country dimension,
//! `cross_border`, and regulated-vs-unregulated US debit need BIN/issuer enrichment in
//! `derive_cluster_key` first, so US debit here is a single blended estimate.
//!
//! **3DS is intentionally not a tier dimension.** In the US, authentication shifts fraud
//! liability but does not change interchange, so `*_3ds_*` and `*_no3ds_*` price the same.

use std::collections::HashMap;

use crate::config::{SeedCostEntry, SeedCostTier};

use super::cluster_key::ClusterKey;
use super::hypersense_client::PspCost;

/// Effective bps for an amount-independent `{pct_bps, fixed}` split.
fn effective_cost_bps(pct_bps: f64, fixed: f64, amount: f64) -> f64 {
    if amount > 0.0 {
        pct_bps + (fixed / amount) * 10_000.0
    } else {
        pct_bps
    }
}

/// Cluster field as a lowercase-comparable str ("" when absent — never matches a
/// specified tier field, since the tier value is non-empty).
fn field(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("")
}

/// Match specificity of a tier against a cluster, or `None` if it doesn't apply.
///
/// Each field the tier specifies must match the same-named cluster field by case-insensitive
/// equality. The returned score is `matched_field_count·32 + tiebreak`, so a tier matching
/// more fields always outranks one matching fewer, and same-count ties resolve by the more
/// discriminating dimension (issuer_region > currency > network > funding > program). A
/// wildcard (`None`) field adds nothing.
fn tier_score(tier: &SeedCostTier, cluster: &ClusterKey) -> Option<u32> {
    let mut count = 0u32;
    let mut tiebreak = 0u32;

    if let Some(v) = &tier.card_issuing_country {
        if !v.eq_ignore_ascii_case(field(&cluster.card_issuing_country)) {
            return None;
        }
        count += 1;
        tiebreak += 16;
    }
    if let Some(v) = &tier.transaction_currency {
        if !v.eq_ignore_ascii_case(field(&cluster.transaction_currency)) {
            return None;
        }
        count += 1;
        tiebreak += 8;
    }
    if let Some(v) = &tier.card_network {
        if !v.eq_ignore_ascii_case(field(&cluster.card_network)) {
            return None;
        }
        count += 1;
        tiebreak += 4;
    }
    if let Some(v) = &tier.payment_method_type {
        if !v.eq_ignore_ascii_case(field(&cluster.payment_method_type)) {
            return None;
        }
        count += 1;
        tiebreak += 2;
    }
    if let Some(v) = &tier.card_type {
        if !v.eq_ignore_ascii_case(field(&cluster.card_type)) {
            return None;
        }
        count += 1;
        tiebreak += 1;
    }

    Some(count * 32 + tiebreak)
}

/// Resolve the `{pct_bps, fixed}` for a PSP entry against a cluster: the most specific
/// matching tier, falling back to the entry's `default`.
fn resolve_fee(entry: &SeedCostEntry, cluster: &ClusterKey) -> (f64, f64) {
    entry
        .tiers
        .iter()
        .filter_map(|t| tier_score(t, cluster).map(|s| (s, t)))
        .max_by_key(|(s, _)| *s)
        .map(|(_, t)| (t.pct_bps, t.fixed))
        .unwrap_or((entry.default.pct_bps, entry.default.fixed))
}

/// Returns deterministic per-PSP costs from the config seed table, mirroring the shape of
/// `hypersense_client::lookup_costs`. PSPs without a configured entry are omitted, so the
/// multi-objective algorithm treats them as "no cost data" (head-wins) — exactly as it
/// would for a live PSP the fee API doesn't price.
pub fn lookup_seed_costs(
    entries: &[SeedCostEntry],
    cluster: &ClusterKey,
    psps: &[String],
) -> HashMap<String, PspCost> {
    let amount = cluster.amount.unwrap_or(0.0);
    psps.iter()
        .filter_map(|psp| {
            let entry = entries.iter().find(|e| e.psp.eq_ignore_ascii_case(psp))?;
            let (pct_bps, fixed) = resolve_fee(entry, cluster);
            Some((
                psp.clone(),
                PspCost {
                    available: true,
                    effective_cost_bps: effective_cost_bps(pct_bps, fixed, amount),
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SeedCostEntry, SeedCostTier, SeedFeeModel};

    /// Full cluster builder across all four matchable dimensions.
    fn cluster(
        network: &str,
        funding: &str,
        program: &str,
        currency: &str,
        amount: f64,
    ) -> ClusterKey {
        ClusterKey {
            amount: Some(amount),
            card_network: Some(network.to_string()),
            payment_method_type: Some(funding.to_string()),
            card_type: Some(program.to_string()),
            transaction_currency: Some(currency.to_string()),
            ..Default::default()
        }
    }

    /// Convenience: a US credit card of the given network/program.
    fn usd_credit(network: &str, program: &str, amount: f64) -> ClusterKey {
        cluster(network, "credit", program, "USD", amount)
    }

    /// Tier builder — pass `None` for any wildcard dimension.
    fn t(
        network: Option<&str>,
        funding: Option<&str>,
        program: Option<&str>,
        currency: Option<&str>,
        pct_bps: f64,
        fixed: f64,
    ) -> SeedCostTier {
        SeedCostTier {
            card_network: network.map(String::from),
            payment_method_type: funding.map(String::from),
            card_type: program.map(String::from),
            transaction_currency: currency.map(String::from),
            card_issuing_country: None,
            pct_bps,
            fixed,
        }
    }

    /// Tier scoped to an issuer region ("us" | "eu" | "intl"), plus optional network/funding.
    fn region_tier(
        region: &str,
        network: Option<&str>,
        funding: Option<&str>,
        program: Option<&str>,
        pct_bps: f64,
        fixed: f64,
    ) -> SeedCostTier {
        SeedCostTier {
            card_issuing_country: Some(region.to_string()),
            ..t(network, funding, program, None, pct_bps, fixed)
        }
    }

    /// Cluster builder including issuer region.
    fn cluster_region(
        region: &str,
        network: &str,
        funding: &str,
        program: &str,
        amount: f64,
    ) -> ClusterKey {
        ClusterKey {
            card_issuing_country: Some(region.to_string()),
            ..cluster(network, funding, program, "USD", amount)
        }
    }

    // Representative slice of the shipped development.toml seed table.
    fn entries() -> Vec<SeedCostEntry> {
        vec![
            SeedCostEntry {
                psp: "stripe".to_string(),
                default: SeedFeeModel {
                    pct_bps: 290.0,
                    fixed: 0.30,
                },
                tiers: vec![],
            },
            SeedCostEntry {
                psp: "adyen".to_string(),
                default: SeedFeeModel {
                    pct_bps: 194.0,
                    fixed: 0.24,
                }, // global fallback = US visa credit standard
                tiers: vec![
                    // US (USD)
                    t(Some("visa"), None, Some("standard"), Some("USD"), 194.0, 0.24),
                    t(Some("visa"), None, Some("premium"), Some("USD"), 254.0, 0.24),
                    t(Some("visa"), None, Some("ultra_premium"), Some("USD"), 310.0, 0.24),
                    t(Some("visa"), Some("debit"), None, Some("USD"), 95.0, 0.30),
                    t(Some("mastercard"), None, Some("standard"), Some("USD"), 190.0, 0.24),
                    t(Some("mastercard"), None, Some("premium"), Some("USD"), 250.0, 0.24),
                    t(Some("amex"), None, None, Some("USD"), 343.0, 0.10),
                    // EEA (EUR) — consumer capped, commercial exempt.
                    t(Some("visa"), Some("debit"), None, Some("EUR"), 80.0, 0.11),
                    t(Some("visa"), Some("credit"), None, Some("EUR"), 90.0, 0.11),
                    t(Some("visa"), Some("credit"), Some("commercial"), Some("EUR"), 235.0, 0.11),
                ],
            },
        ]
    }

    #[test]
    fn stripe_is_blended_regardless_of_tier() {
        let e = entries();
        let psps = vec!["stripe".to_string()];
        let std = lookup_seed_costs(&e, &usd_credit("visa", "standard", 100.0), &psps);
        let prem = lookup_seed_costs(&e, &usd_credit("visa", "premium", 100.0), &psps);
        // 2.9% + $0.30 on $100 = $3.20 → 320 bps, identical for both tiers.
        assert!((std["stripe"].effective_cost_bps - 320.0).abs() < 1e-6);
        assert!((prem["stripe"].effective_cost_bps - 320.0).abs() < 1e-6);
    }

    #[test]
    fn adyen_ic_plus_plus_varies_by_tier() {
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let std = lookup_seed_costs(&e, &usd_credit("visa", "standard", 100.0), &psps);
        let prem = lookup_seed_costs(&e, &usd_credit("visa", "premium", 100.0), &psps);
        // Standard 1.94% + $0.24 = $2.18 → 218 bps; premium 2.54% + $0.24 = $2.78 → 278 bps.
        assert!((std["adyen"].effective_cost_bps - 218.0).abs() < 1e-6);
        assert!((prem["adyen"].effective_cost_bps - 278.0).abs() < 1e-6);
    }

    #[test]
    fn most_specific_tier_wins_over_default() {
        // Mastercard premium should hit the mc-premium tier (250), not the visa-shaped default.
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let costs = lookup_seed_costs(&e, &usd_credit("mastercard", "premium", 100.0), &psps);
        // 2.50% + $0.24 on $100 = $2.74 → 274 bps.
        assert!((costs["adyen"].effective_cost_bps - 274.0).abs() < 1e-6);
    }

    #[test]
    fn unmatched_network_falls_back_to_default() {
        // An unlisted network (rupay) has no tier → uses adyen's default (visa-standard shape).
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let costs = lookup_seed_costs(&e, &usd_credit("rupay", "standard", 100.0), &psps);
        assert!((costs["adyen"].effective_cost_bps - 218.0).abs() < 1e-6);
    }

    #[test]
    fn program_matches_exactly_no_prefix_collision() {
        // Exact match keeps "premium" and "ultra_premium" distinct — the prefix of the latter
        // must NOT fall into the former.
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let prem = lookup_seed_costs(&e, &usd_credit("visa", "premium", 100.0), &psps);
        let ultra = lookup_seed_costs(&e, &usd_credit("visa", "ultra_premium", 100.0), &psps);
        assert!((prem["adyen"].effective_cost_bps - 278.0).abs() < 1e-6); // 254 + $0.24
        assert!((ultra["adyen"].effective_cost_bps - 334.0).abs() < 1e-6); // 310 + $0.24
    }

    #[test]
    fn debit_is_cheaper_than_credit() {
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let debit = lookup_seed_costs(&e, &cluster("visa", "debit", "standard", "USD", 100.0), &psps);
        let credit = lookup_seed_costs(&e, &usd_credit("visa", "standard", 100.0), &psps);
        // Debit hits the funding tier (0.95% + $0.30 = 95 + 30 = 125 bps @ $100).
        assert!((debit["adyen"].effective_cost_bps - 125.0).abs() < 1e-6);
        assert!(debit["adyen"].effective_cost_bps < credit["adyen"].effective_cost_bps);
    }

    #[test]
    fn funding_outranks_program_at_equal_specificity() {
        // A visa debit *premium* card matches both the visa-premium tier and the visa-debit
        // tier (both 3 fields); funding is the more discriminating dimension, so debit wins.
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let costs = lookup_seed_costs(&e, &cluster("visa", "debit", "premium", "USD", 100.0), &psps);
        assert!((costs["adyen"].effective_cost_bps - 125.0).abs() < 1e-6);
    }

    #[test]
    fn currency_tier_applies_and_outranks_program() {
        // A visa *premium* card paid in EUR hits the EEA visa-credit cap tier (US premium is
        // USD-scoped and can't match), so it prices at the capped EEA rate, not US premium.
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let costs = lookup_seed_costs(&e, &cluster("visa", "credit", "premium", "EUR", 100.0), &psps);
        // 0.90% + €0.11 on €100 = 90 + 11 = 101 bps.
        assert!((costs["adyen"].effective_cost_bps - 101.0).abs() < 1e-6);
    }

    #[test]
    fn eea_commercial_beats_the_consumer_cap_tier() {
        // EEA commercial cards are exempt from the consumer interchange cap. The 4-field
        // commercial tier outranks the 3-field consumer-credit tier by field count.
        let e = entries();
        let psps = vec!["adyen".to_string()];
        let costs =
            lookup_seed_costs(&e, &cluster("visa", "credit", "commercial", "EUR", 100.0), &psps);
        // 2.35% + €0.11 on €100 = 235 + 11 = 246 bps.
        assert!((costs["adyen"].effective_cost_bps - 246.0).abs() < 1e-6);
    }

    #[test]
    fn adyen_beats_stripe_on_a_standard_card() {
        let e = entries();
        let psps = vec!["adyen".to_string(), "stripe".to_string()];
        let costs = lookup_seed_costs(&e, &usd_credit("visa", "standard", 44.0), &psps);
        // On a $44 standard card: Adyen ~248 bps vs Stripe ~358 bps blended.
        assert!(costs["adyen"].effective_cost_bps < costs["stripe"].effective_cost_bps);
    }

    // Issuer region separates same-currency (USD) scenarios at a US merchant: a US-issued
    // debit (Durbin) prices differently from an EU-issued consumer debit even though both
    // are USD card-present-style txns. Region is the most discriminating dimension.
    #[test]
    fn issuer_region_separates_same_currency_debit() {
        let adyen = SeedCostEntry {
            psp: "adyen".to_string(),
            default: SeedFeeModel { pct_bps: 194.0, fixed: 0.24 },
            tiers: vec![
                region_tier("us", Some("visa"), Some("debit"), None, 78.0, 0.30), // ~108bps @ $100
                region_tier("eu", Some("visa"), Some("debit"), None, 68.0, 0.30), // ~98bps  @ $100
            ],
        };
        let e = vec![adyen];
        let psps = vec!["adyen".to_string()];
        let us = lookup_seed_costs(&e, &cluster_region("us", "visa", "debit", "standard", 100.0), &psps);
        let eu = lookup_seed_costs(&e, &cluster_region("eu", "visa", "debit", "standard", 100.0), &psps);
        assert!((us["adyen"].effective_cost_bps - 108.0).abs() < 1e-6);
        assert!((eu["adyen"].effective_cost_bps - 98.0).abs() < 1e-6);
    }

    // A region-scoped tier (more fields matched) outranks a region-agnostic one of the same
    // shape, and an `intl` card that matches no US/EU tier falls to the intl tier.
    #[test]
    fn intl_card_takes_the_intl_tier() {
        let adyen = SeedCostEntry {
            psp: "adyen".to_string(),
            default: SeedFeeModel { pct_bps: 194.0, fixed: 0.24 },
            tiers: vec![
                region_tier("us", Some("visa"), Some("credit"), Some("standard"), 222.0, 0.24),
                region_tier("intl", None, None, None, 239.0, 0.24), // ~263bps @ $100
            ],
        };
        let e = vec![adyen];
        let psps = vec!["adyen".to_string()];
        let intl = lookup_seed_costs(&e, &cluster_region("intl", "visa", "credit", "standard", 100.0), &psps);
        // US tier requires issuer=us; intl card can't match it, so it lands on the intl tier.
        assert!((intl["adyen"].effective_cost_bps - 263.0).abs() < 1e-6);
    }

    #[test]
    fn unknown_psp_is_omitted() {
        let e = entries();
        let psps = vec!["worldpay".to_string()];
        let costs = lookup_seed_costs(&e, &usd_credit("visa", "standard", 100.0), &psps);
        assert!(costs.is_empty(), "unseeded PSPs carry no cost data");
    }

    #[test]
    fn empty_config_yields_no_costs() {
        let psps = vec!["adyen".to_string(), "stripe".to_string()];
        let costs = lookup_seed_costs(&[], &usd_credit("visa", "standard", 100.0), &psps);
        assert!(costs.is_empty());
    }
}
