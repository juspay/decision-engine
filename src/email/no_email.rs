use super::{EmailClient, EmailError, EmailMessage};

pub struct NoEmailClient;

#[async_trait::async_trait]
impl EmailClient for NoEmailClient {
    async fn send_email(&self, message: EmailMessage) -> error_stack::Result<(), EmailError> {
        // Extract the action URL from the body so developers can complete email
        // verification, password reset, or an invite manually. Avoid logging the full
        // HTML body for other email types. Reset and invite URLs carry a live single-use
        // credential, so they are only logged in debug builds — never from a release
        // binary.
        // `starts_with` (not `contains`) for the release-logged verification branch: invite
        // subjects embed an admin-chosen merchant name, so a substring match could let a
        // crafted merchant name route a live set-password link into release logs.
        let subject_lower = message.subject.to_lowercase();
        let action_url = if subject_lower.starts_with("confirm your email")
            || (cfg!(debug_assertions)
                && (subject_lower.contains("reset your password")
                    || subject_lower.contains("invited")))
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
