use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{PaymentAuditFilterOptions, PaymentAuditQuery, PaymentAuditScope};
use crate::error::ApiError;

use super::super::common::{fetch_one, DOMAIN_TABLE};
use super::super::filters::{base_window_filters, merchant_filter, scope_flow_type_filters};
use super::super::query::BoundQueryBuilder;
use super::super::time::effective_payment_audit_window_bounds;

/// Values per dropdown, so a merchant with unusual data can't make the response unbounded.
const MAX_OPTIONS: usize = 200;

#[derive(Debug, Clone, Deserialize, Row)]
struct OptionsRow {
    gateways: Vec<String>,
    payment_methods: Vec<String>,
    payment_method_types: Vec<String>,
    card_networks: Vec<String>,
    auth_types: Vec<String>,
    currencies: Vec<String>,
}

/// The distinct values the merchant's payments in the window carried, for the audit's dropdown
/// filters. Read from the same events and normalised the same way as the filters match them
/// (`audit_summaries`), so every option finds payments.
pub async fn load(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditFilterOptions, ApiError> {
    let row = fetch_one::<OptionsRow>(options_query(query).build(client)).await?;
    Ok(PaymentAuditFilterOptions {
        gateways: row.gateways,
        payment_methods: row.payment_methods,
        payment_method_types: row.payment_method_types,
        card_networks: row.card_networks,
        auth_types: row.auth_types,
        currencies: row.currencies,
    })
}

fn options_query(query: &PaymentAuditQuery) -> BoundQueryBuilder {
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    // Attribute values aggregate lower-cased; a hybrid request without a value records `null`.
    let attribute = |column: &str, alias: &str| {
        format!(
            "arraySort(arrayFilter(value -> value NOT IN ('', 'null'), \
             groupUniqArray({MAX_OPTIONS})(lowerUTF8(ifNull({column}, ''))))) AS {alias}"
        )
    };

    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!(
            "arraySort(arrayFilter(value -> value != '', \
             groupUniqArray({MAX_OPTIONS})(ifNull(gateway, '')))) AS gateways"
        ),
        attribute("payment_method", "payment_methods"),
        attribute("payment_method_type", "payment_method_types"),
        attribute("card_network", "card_networks"),
        attribute("auth_type", "auth_types"),
        attribute("currency", "currencies"),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(scope_flow_type_filters(PaymentAuditScope::All));

    builder
}

#[cfg(test)]
mod tests {
    use super::options_query;
    use crate::analytics::models::PaymentAuditQuery;

    #[test]
    fn options_are_normalised_like_the_filters_and_bounded() {
        let query = PaymentAuditQuery::from_request(
            "m_1".to_string(),
            Some("1h".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let fragment = options_query(&query).into_fragment();
        let sql = fragment.sql();
        assert!(sql.contains("groupUniqArray(200)(lowerUTF8(ifNull(currency, '')))"));
        assert!(sql.contains("value NOT IN ('', 'null')"));
        // Gateways match exactly, so they are not lower-cased.
        assert!(sql.contains("groupUniqArray(200)(ifNull(gateway, ''))"));
        assert!(sql.contains("merchant_id = ?"));
        assert!(sql.contains("created_at_ms >= ?"));
    }
}
