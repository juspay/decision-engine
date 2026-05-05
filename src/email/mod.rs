#[cfg(feature = "email-aws-ses")]
pub mod aws_ses;
pub mod no_email;
pub mod smtp;
pub mod templates;

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub to: String,
    pub subject: String,
    pub html_body: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("Failed to send email")]
    SendFailed,
    #[error("Failed to build email message")]
    BuildFailed,
    #[error("Email client configuration is missing or invalid")]
    MissingConfig,
}

#[async_trait::async_trait]
pub trait EmailClient: Send + Sync {
    async fn send_email(&self, message: EmailMessage) -> error_stack::Result<(), EmailError>;

    /// Verify the client can reach the email backend. Called at server startup.
    async fn health_check(&self) -> error_stack::Result<(), EmailError>;
}

pub type DynEmailClient = Arc<dyn EmailClient>;
