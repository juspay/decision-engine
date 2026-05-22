/// Deterministic arm assignment using djb2 hash of payment_id.
/// Returns "variant" if hash % 100 < variant_split_pct, else "control".
/// Same payment_id always returns the same arm — retries land on the same gateway.
pub fn assign_arm(payment_id: &str, variant_split_pct: u8) -> &'static str {
    let hash = payment_id.bytes().fold(5381u64, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(b as u64)
    });
    if (hash % 100) < variant_split_pct as u64 {
        "variant"
    } else {
        "control"
    }
}
