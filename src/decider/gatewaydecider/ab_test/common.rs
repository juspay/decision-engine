use super::arms::ArmSide;

/// Deterministic arm assignment using djb2 hash of payment_id.
/// Returns `Variant` if hash % 100 < variant_split_pct, else `Control`.
/// Same payment_id always returns the same arm — retries and every endpoint (evaluate, decide
/// gateway, hybrid) land on the same arm.
pub fn assign_arm(payment_id: &str, variant_split_pct: u8) -> ArmSide {
    let hash = payment_id.bytes().fold(5381u64, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(b as u64)
    });
    if (hash % 100) < variant_split_pct as u64 {
        ArmSide::Variant
    } else {
        ArmSide::Control
    }
}
