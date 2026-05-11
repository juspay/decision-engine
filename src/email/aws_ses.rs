use aws_config::BehaviorVersion;
use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message};
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use error_stack::ResultExt;
use hyper_proxy::{Intercept, Proxy, ProxyConnector};

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
        let region = aws_sdk_sesv2::config::Region::new(config.region.clone());

        let aws_config = if let (Some(role_arn), Some(session_name)) =
            (&config.email_role_arn, &config.sts_role_session_name)
        {
            // Load a base config to build the STS client, then assume the target role.
            // The resulting credentials are used for all SES calls (cross-account setup).
            let base_config = aws_config::defaults(BehaviorVersion::latest())
                .region(region.clone())
                .load()
                .await;

            let assume_role_provider =
                aws_config::sts::AssumeRoleProvider::builder(role_arn.as_str())
                    .session_name(session_name.as_str())
                    .configure(&base_config)
                    .build()
                    .await;

            let mut loader = aws_config::defaults(BehaviorVersion::latest())
                .region(region)
                .credentials_provider(assume_role_provider);

            if let Some(proxy_url) = &config.proxy_url {
                let http_client = build_proxied_http_client(proxy_url)
                    .change_context(EmailError::MissingConfig)?;
                loader = loader.http_client(http_client);
            }

            loader.load().await
        } else {
            let mut loader = aws_config::defaults(BehaviorVersion::latest()).region(region);

            if let Some(proxy_url) = &config.proxy_url {
                let http_client = build_proxied_http_client(proxy_url)
                    .change_context(EmailError::MissingConfig)?;
                loader = loader.http_client(http_client);
            }

            loader.load().await
        };

        let client = aws_sdk_sesv2::Client::new(&aws_config);

        Ok(Self {
            client,
            sender_email,
        })
    }
}

fn build_proxied_http_client(
    proxy_url: &str,
) -> Result<aws_smithy_runtime::client::http::hyper_014::HyperClient, EmailError> {
    let proxy_uri = proxy_url
        .parse::<hyper014::Uri>()
        .map_err(|_| EmailError::MissingConfig)?;
    let proxy = Proxy::new(Intercept::All, proxy_uri);
    let connector = ProxyConnector::from_proxy(hyper014::client::HttpConnector::new(), proxy)
        .map_err(|_| EmailError::MissingConfig)?;
    Ok(HyperClientBuilder::new().build(connector))
}

#[async_trait::async_trait]
impl EmailClient for AwsSesEmailClient {
    async fn health_check(&self) -> error_stack::Result<(), EmailError> {
        // get_account() requires ses:GetAccount which send-only roles don't have.
        // Checking the sender identity requires only ses:GetEmailIdentity and directly
        // validates the address that will be used for sending.
        self.client
            .get_email_identity()
            .email_identity(&self.sender_email)
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

        let msg = Message::builder().subject(subject).body(body).build();

        let email_content = EmailContent::builder().simple(msg).build();

        let destination = Destination::builder().to_addresses(&message.to).build();

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
