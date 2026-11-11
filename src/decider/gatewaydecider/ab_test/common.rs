use super::arms::{ArmSide, ResolvedVariant};

/// Bucket in 0..100 of a payment id, by djb2 hash.
fn bucket(payment_id: &str) -> u64 {
    let hash = payment_id.bytes().fold(5381u64, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(b as u64)
    });
    hash % 100
}

/// Deterministic arm assignment from the payment id's bucket. Variants own consecutive slices of
/// the 100 buckets in list order (the first variant `[0, split₁)`, the next `[split₁, split₁ +
/// split₂)`, …) and control owns the rest. A stopped variant keeps its slice, so stopping one
/// variant never moves a payment between the others.
///
/// Same payment_id always returns the same arm — retries and every endpoint (evaluate, decide
/// gateway, hybrid) land on the same arm.
pub fn assign_arm(payment_id: &str, variants: &[ResolvedVariant]) -> ArmSide {
    let bucket = bucket(payment_id);
    let mut upper = 0u64;
    for variant in variants {
        upper += u64::from(variant.split_pct);
        if bucket < upper {
            return ArmSide::Variant(variant.id.clone());
        }
    }
    ArmSide::Control
}
