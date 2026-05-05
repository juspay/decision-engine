use error_stack::ResultExt;
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use std::time::Duration;

use crate::config::{SmtpConfig, SmtpTls};

use super::{EmailClient, EmailError, EmailMessage};

pub struct SmtpEmailClient {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    sender_email: String,
    endpoint: String,
}

impl SmtpEmailClient {
    pub fn new(config: &SmtpConfig, sender_email: String) -> error_stack::Result<Self, EmailError> {
        let creds = Credentials::new(config.username.clone(), config.password.clone());

        let (builder, default_port) = match config.tls {
            SmtpTls::None => (
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host),
                1025u16,
            ),
            SmtpTls::StartTls => (
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
                    .change_context(EmailError::BuildFailed)?,
                587u16,
            ),
            SmtpTls::Tls => (
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
                    .change_context(EmailError::BuildFailed)?,
                465u16,
            ),
        };

        let port = config.port.unwrap_or(default_port);
        let mailer = builder
            .credentials(creds)
            .port(port)
            .timeout(Some(Duration::from_secs(5)))
            .build();

        // Validate sender_email at construction time so misconfiguration is
        // caught at startup rather than on the first send attempt.
        sender_email
            .parse::<lettre::message::Mailbox>()
            .change_context(EmailError::BuildFailed)
            .attach_printable_lazy(|| {
                format!("invalid sender_email address: {:?}", sender_email)
            })?;

        Ok(Self {
            mailer,
            sender_email,
            endpoint: format!("{}:{}", config.host, port),
        })
    }
}

#[async_trait::async_trait]
impl EmailClient for SmtpEmailClient {
    async fn health_check(&self) -> error_stack::Result<(), EmailError> {
        self.mailer
            .test_connection()
            .await
            .change_context(EmailError::SendFailed)
            .attach_printable_lazy(|| format!("SMTP endpoint: {}", self.endpoint))?;
        Ok(())
    }

    async fn send_email(&self, message: EmailMessage) -> error_stack::Result<(), EmailError> {
        let from = self
            .sender_email
            .parse::<lettre::message::Mailbox>()
            .change_context(EmailError::BuildFailed)?;

        let to = message
            .to
            .parse::<lettre::message::Mailbox>()
            .change_context(EmailError::BuildFailed)?;

        let email = Message::builder()
            .from(from)
            .to(to)
            .subject(message.subject)
            .header(ContentType::TEXT_HTML)
            .body(message.html_body)
            .change_context(EmailError::BuildFailed)?;

        self.mailer
            .send(email)
            .await
            .change_context(EmailError::SendFailed)?;

        Ok(())
    }
}
