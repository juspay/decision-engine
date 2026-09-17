use crate::analytics::models::{PaymentAuditQuery, PaymentAuditResponse, PaymentAuditScope};
use crate::error::ApiError;

pub async fn load(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditResponse, ApiError> {
    super::payment_audit::load(client, query, PaymentAuditScope::Preview).await
}
