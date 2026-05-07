use crate::{
    interface::GsmLookup,
    types::{GsmErrorInfo, GsmInfo, GsmRule},
};

/// Mirrors `create_issuer_code_lookup_key` in hyperswitch.
/// The issuer lookup stores rules using this synthetic key in both `code` and `message`.
fn issuer_lookup_key(card_network: &str, issuer_code: &str) -> String {
    format!("Network:{}|IssuerCode:{:0>2}", card_network, issuer_code)
}

/// Look up a GSM rule using the same two-level priority as hyperswitch:
///
/// 1. Issuer code lookup — requires both `card_network` and `issuer_error_code`.
///    Uses a synthetic key (`Network:{network}|IssuerCode:{code}`) for both the
///    `code` and `message` fields in the index.
///
/// 2. Connector error code lookup — falls back to `connector_error_code` /
///    `connector_error_message`, defaulting to empty string if absent.
pub fn get_gsm_rule<'a, S: GsmLookup>(
    store: &'a S,
    connector: &str,
    flow: &str,
    sub_flow: &str,
    connector_error_code: Option<&str>,
    connector_error_message: Option<&str>,
    issuer_error_code: Option<&str>,
    card_network: Option<&str>,
) -> Option<&'a GsmRule> {
    if let (Some(network), Some(issuer_code)) = (card_network, issuer_error_code) {
        let key = issuer_lookup_key(network, issuer_code);
        if let Some(rule) = store.find_gsm_rule(connector, flow, sub_flow, &key, &key) {
            return Some(rule);
        }
    }

    store.find_gsm_rule(
        connector,
        flow,
        sub_flow,
        connector_error_code.unwrap_or(""),
        connector_error_message.unwrap_or(""),
    )
}

/// Look up a GSM rule from a structured `GsmErrorInfo` and convert the result to `GsmInfo`.
/// Returns `None` when no matching rule exists in the store.
pub fn lookup_from_error_info<S: GsmLookup>(
    store: &S,
    error_info: &GsmErrorInfo,
) -> Option<GsmInfo> {
    let rule = get_gsm_rule(
        store,
        &error_info.connector,
        &error_info.flow,
        &error_info.sub_flow,
        error_info.error_code.as_deref(),
        error_info.error_message.as_deref(),
        error_info.issuer_error_code.as_deref(),
        error_info.card_network.as_deref(),
    )?;

    Some(GsmInfo {
        decision: rule.decision.to_string(),
        step_up_possible: rule.step_up_possible,
        clear_pan_possible: rule.clear_pan_possible,
        alternate_network_possible: rule.alternate_network_possible,
        unified_code: rule.unified_code.clone(),
        unified_message: rule.unified_message.clone(),
        error_category: rule.error_category.clone(),
        standardised_code: rule.standardised_code.clone(),
        description: rule.description.clone(),
        user_guidance_message: rule.user_guidance_message.clone(),
    })
}
