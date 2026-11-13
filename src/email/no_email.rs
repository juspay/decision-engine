use super::{EmailClient, EmailError, EmailMessage};

pub struct NoEmailClient;

#[async_trait::async_trait]
impl EmailClient for NoEmailClient {
    async fn send_email(&self, message: EmailMessage) -> error_stack::Result<(), EmailError> {
        // Extract the verification URL from the body so developers can complete
        // email verification manually. Avoid logging the full HTML body for other
        // email types (e.g. invite emails) because it would expose temporary passwords.
        let verification_url = if message
            .subject
            .to_lowercase()
            .contains("confirm your email")
        {
            extract_href_from_cta(&message.html_body)
        } else {
            None
        };

        match verification_url {
            Some(url) => crate::logger::info!(
                to = %message.to,
                subject = %message.subject,
                verification_url = %url,
                "Email sending skipped (no_email_client) — use the verification_url to verify manually"
            ),
            None => crate::logger::info!(
                to = %message.to,
                subject = %message.subject,
                "Email sending skipped (no_email_client configured)"
            ),
        }

        Ok(())
    }

    async fn health_check(&self) -> error_stack::Result<(), EmailError> {
        Ok(())
    }
}

/// Extracts the first `href` value from an anchor tag inside a `<td>` CTA button in the HTML body.
fn extract_href_from_cta(html: &str) -> Option<String> {
    let anchor_start = html.find("<a href=\"")?;
    let href_start = anchor_start + "<a href=\"".len();
    let href_end = html[href_start..].find('"')?;
    Some(html[href_start..href_start + href_end].to_string())
}
