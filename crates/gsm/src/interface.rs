use crate::types::GsmRule;

/// Synchronous GSM lookup interface.
///
/// Implement this trait to provide a backing store for GSM rules — whether
/// from a CSV file, in-memory HashMap, database, or any other source.
///
/// All lookups are synchronous and zero-cost after initial load, making this
/// safe to call on every payment decision without DB or network overhead.
pub trait GsmLookup {
    fn find_gsm_rule(
        &self,
        connector: &str,
        flow: &str,
        sub_flow: &str,
        code: &str,
        message: &str,
    ) -> Option<&GsmRule>;
}
