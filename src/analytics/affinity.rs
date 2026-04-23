pub fn api_shard_key(request_id: &str) -> String {
    if request_id.is_empty() {
        "unknown".to_string()
    } else {
        request_id.to_string()
    }
}

pub fn domain_shard_key(
    request_id: Option<&str>,
    global_request_id: Option<&str>,
    payment_id: Option<&str>,
    event_id: u64,
) -> String {
    first_non_empty([request_id, global_request_id, payment_id])
        .map(str::to_string)
        .unwrap_or_else(|| event_id.to_string())
}

fn first_non_empty<'a, I>(values: I) -> Option<&'a str>
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{api_shard_key, domain_shard_key};

    #[test]
    fn api_shard_key_uses_request_id() {
        assert_eq!(api_shard_key("req_123"), "req_123");
    }

    #[test]
    fn domain_shard_key_prefers_request_id() {
        assert_eq!(
            domain_shard_key(Some("req_1"), Some("global_1"), Some("pay_1"), 99),
            "req_1"
        );
    }

    #[test]
    fn domain_shard_key_falls_back_in_order() {
        assert_eq!(
            domain_shard_key(None, Some("global_1"), Some("pay_1"), 99),
            "global_1"
        );
        assert_eq!(domain_shard_key(None, None, Some("pay_1"), 99), "pay_1");
        assert_eq!(domain_shard_key(None, None, None, 99), "99");
    }
}
