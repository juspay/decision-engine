use aws_config::BehaviorVersion;
use aws_sdk_sesv2::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message};
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
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

        // Credential resolution intentionally uses no proxy — STS is reachable via VPC endpoint.
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

            aws_config::defaults(BehaviorVersion::latest())
                .region(region)
                .credentials_provider(assume_role_provider)
                .load()
                .await
        } else {
            aws_config::defaults(BehaviorVersion::latest())
                .region(region)
                .load()
                .await
        };

        // Proxy is applied only at the SES service-client level so that credential
        // resolution (STS AssumeRole, IMDS) continues to use the direct VPC path.
        let ses_config = {
            let mut builder = aws_sdk_sesv2::config::Builder::from(&aws_config);
            if let Some(proxy_url) = &config.proxy_url {
                let http_client = build_proxied_http_client(proxy_url)
                    .change_context(EmailError::MissingConfig)?;
                builder = builder.http_client(http_client);
            }
            builder.build()
        };

        let client = aws_sdk_sesv2::Client::from_conf(ses_config);

        Ok(Self {
            client,
            sender_email,
        })
    }
}

fn build_proxied_http_client(
    proxy_url: &str,
) -> Result<aws_smithy_runtime_api::client::http::SharedHttpClient, EmailError> {
    let proxy_uri = proxy_url
        .parse::<hyper014::Uri>()
        .map_err(|_| EmailError::MissingConfig)?;
    let proxy = Proxy::new(Intercept::All, proxy_uri);
    let connector = ProxyConnector::from_proxy(hyper014::client::HttpConnector::new(), proxy)
        .map_err(|_| EmailError::MissingConfig)?;
    Ok(HyperClientBuilder::new().build(connector))
}

/// `SdkError`'s own `Display` renders as just "service error" or "dispatch failure",
/// which hides everything needed to act on the failure. The SES error code and message,
/// the HTTP status, the response body and the transport-level cause are all reachable
/// only through the error metadata and the `source()` chain, so they are flattened into
/// a single printable line here.
fn describe_sdk_error<E>(err: &SdkError<E, HttpResponse>) -> String
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    let kind = match err {
        SdkError::ConstructionFailure(_) => "construction_failure",
        SdkError::TimeoutError(_) => "timeout",
        SdkError::DispatchFailure(_) => "dispatch_failure",
        SdkError::ResponseError(_) => "response_error",
        SdkError::ServiceError(_) => "service_error",
        _ => "unknown",
    };

    let meta = err.meta();
    let mut detail = format!(
        "kind={kind} code={} message={}",
        meta.code().unwrap_or("<none>"),
        meta.message().unwrap_or("<none>"),
    );

    // A non-AWS intermediary (proxy, load balancer) answers with a body that does not
    // deserialize into a modeled error, leaving the metadata empty — the raw response is
    // the only place its reason appears.
    if let SdkError::ServiceError(context) = err {
        let raw = context.raw();
        detail.push_str(&format!(" http_status={}", raw.status().as_u16()));
        if let Some(bytes) = raw.body().bytes() {
            let truncated = &bytes[..bytes.len().min(512)];
            detail.push_str(&format!(" body={}", String::from_utf8_lossy(truncated)));
        }
    }

    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        detail.push_str(&format!(" | caused_by: {cause}"));
        source = cause.source();
    }

    detail
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
            .map_err(|err| {
                let detail = describe_sdk_error(&err);
                error_stack::Report::new(err).attach_printable(detail)
            })
            .change_context(EmailError::SendFailed)
            .attach_printable_lazy(|| format!("sender_email={}", self.sender_email))?;
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
            .map_err(|err| {
                let detail = describe_sdk_error(&err);
                error_stack::Report::new(err).attach_printable(detail)
            })
            .change_context(EmailError::SendFailed)
            .attach_printable_lazy(|| format!("sender_email={}", self.sender_email))?;

        Ok(())
    }
}
