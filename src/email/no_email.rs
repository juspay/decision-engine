use super::{EmailClient, EmailError, EmailMessage};

pub struct NoEmailClient;

#[async_trait::async_trait]
impl EmailClient for NoEmailClient {
    async fn send_email(&self, message: EmailMessage) -> error_stack::Result<(), EmailError> {
        // Extract the action URL from the body so developers can complete email
        // verification or password reset manually. Avoid logging the full HTML body
        // for other email types (e.g. invite emails) because it would expose
        // temporary passwords. Reset URLs carry a live single-use credential, so they
        // are only logged in debug builds — never from a release binary.
        let subject_lower = message.subject.to_lowercase();
        let action_url = if subject_lower.contains("confirm your email")
            || (cfg!(debug_assertions) && subject_lower.contains("reset your password"))
        {
            extract_href_from_cta(&message.html_body)
        } else {
            None
        };

        match action_url {
            Some(url) => crate::logger::info!(
                to = %message.to,
                subject = %message.subject,
                action_url = %url,
                "Email sending skipped (no_email_client) — use the action_url to complete the flow manually"
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
