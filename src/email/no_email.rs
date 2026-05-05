use super::{EmailClient, EmailError, EmailMessage};

pub struct NoEmailClient;

#[async_trait::async_trait]
impl EmailClient for NoEmailClient {
    async fn send_email(&self, message: EmailMessage) -> error_stack::Result<(), EmailError> {
        crate::logger::info!(
            to = %message.to,
            subject = %message.subject,
            "Email sending skipped (no_email_client configured)"
        );
        Ok(())
    }

    async fn health_check(&self) -> error_stack::Result<(), EmailError> {
        Ok(())
    }
}
