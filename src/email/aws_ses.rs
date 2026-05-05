use aws_config::BehaviorVersion;
use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message};
use error_stack::ResultExt;

use crate::config::AwsSesEmailConfig;

use super::{EmailClient, EmailError, EmailMessage};

pub struct AwsSesEmailClient {
    client: aws_sdk_sesv2::Client,
    sender_email: String,
}

impl AwsSesEmailClient {
    pub async fn new(
        config: &AwsSesEmailConfig,
        sender_email: String,
    ) -> error_stack::Result<Self, EmailError> {
        let region = aws_config::meta::region::RegionProviderChain::first_try(
            aws_sdk_sesv2::config::Region::new(config.region.clone()),
        );

        let aws_config = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .load()
            .await;

        let client = aws_sdk_sesv2::Client::new(&aws_config);

        Ok(Self {
            client,
            sender_email,
        })
    }
}

#[async_trait::async_trait]
impl EmailClient for AwsSesEmailClient {
    async fn health_check(&self) -> error_stack::Result<(), EmailError> {
        self.client
            .get_account()
            .send()
            .await
            .change_context(EmailError::SendFailed)?;
        Ok(())
    }

    async fn send_email(&self, message: EmailMessage) -> error_stack::Result<(), EmailError> {
        let subject = Content::builder()
            .data(message.subject)
            .charset("UTF-8")
            .build()
            .change_context(EmailError::BuildFailed)?;

        let html_body = Content::builder()
            .data(message.html_body)
            .charset("UTF-8")
            .build()
            .change_context(EmailError::BuildFailed)?;

        let body = Body::builder().html(html_body).build();

        let msg = Message::builder()
            .subject(subject)
            .body(body)
            .build();

        let email_content = EmailContent::builder().simple(msg).build();

        let destination = Destination::builder()
            .to_addresses(&message.to)
            .build();

        self.client
            .send_email()
            .from_email_address(&self.sender_email)
            .destination(destination)
            .content(email_content)
            .send()
            .await
            .change_context(EmailError::SendFailed)?;

        Ok(())
    }
}
