use super::EmailMessage;

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Shared chrome for every transactional email: a plain wordmark, a white content
/// card, and a neutral footer. `preheader` is the hidden inbox-preview line;
/// `content` is the already-escaped inner HTML of the message body.
fn render_layout(preheader: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1.0">
  <meta name="x-apple-disable-message-reformatting">
  <title>Juspay Decision Engine</title>
</head>
<body style="margin:0;padding:0;background-color:#f4f5f7;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <span style="display:none;max-height:0;overflow:hidden;mso-hide:all;">{preheader}</span>

  <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="background-color:#f4f5f7;">
    <tr>
      <td align="center" style="padding:32px 16px;">
        <table width="600" cellpadding="0" cellspacing="0" role="presentation" style="max-width:600px;width:100%;">

          <tr>
            <td style="padding:4px 4px 22px;">
              <table cellpadding="0" cellspacing="0" role="presentation">
                <tr>
                  <td width="38" height="38" align="center" valign="middle" style="background-color:#0561E2;background-image:linear-gradient(135deg,#0099FF,#0561E2);border-radius:10px;">
                    <span style="font-size:19px;font-weight:700;color:#ffffff;line-height:38px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">J</span>
                  </td>
                  <td style="padding-left:12px;" valign="middle">
                    <span style="font-size:17px;font-weight:700;color:#0F172A;letter-spacing:-0.02em;">Juspay</span><span style="font-size:17px;font-weight:500;color:#475569;letter-spacing:-0.02em;">&nbsp;Decision Engine</span>
                  </td>
                </tr>
              </table>
            </td>
          </tr>

          <tr>
            <td style="background-color:#ffffff;border:1px solid #e5e7eb;border-radius:10px;padding:40px 40px 36px;">
{content}
            </td>
          </tr>

          <tr>
            <td style="padding:20px 4px 4px;">
              <p style="margin:0;font-size:12px;line-height:1.6;color:#9ca3af;">
                This is an automated message from Juspay Decision Engine. Please do not reply to this email.
              </p>
            </td>
          </tr>

        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#,
        preheader = escape_html(preheader),
        content = content,
    )
}

/// A single primary call-to-action button. `href` is escaped; `label` is trusted
/// static copy supplied by the caller.
fn primary_button(href: &str, label: &str) -> String {
    format!(
        r#"              <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0;">
                <tr>
                  <td style="background-color:#2563eb;border-radius:8px;">
                    <a href="{href}" style="display:inline-block;background-color:#2563eb;color:#ffffff;font-size:15px;font-weight:600;text-decoration:none;padding:13px 28px;border-radius:8px;">{label}</a>
                  </td>
                </tr>
              </table>"#,
        href = escape_html(href),
        label = label,
    )
}

/// The plain-text fallback shown under a CTA for clients that block buttons.
fn fallback_link(url: &str) -> String {
    let url = escape_html(url);
    format!(
        r#"              <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="margin:32px 0 0;">
                <tr><td style="border-top:1px solid #e5e7eb;"></td></tr>
              </table>
              <p style="margin:24px 0 6px;font-size:13px;color:#6b7280;">Or paste this link into your browser:</p>
              <p style="margin:0;font-size:13px;word-break:break-all;line-height:1.6;">
                <a href="{url}" style="color:#2563eb;text-decoration:none;">{url}</a>
              </p>"#,
        url = url
    )
}

pub struct MemberAddedTemplate {
    pub user_email: String,
    pub merchant_name: String,
    pub base_url: String,
}

impl MemberAddedTemplate {
    pub fn into_message(self) -> EmailMessage {
        let merchant = escape_html(&self.merchant_name);
        let login_url = format!("{}/login", self.base_url);
        let content = format!(
            r#"              <h1 style="margin:0 0 14px;font-size:20px;font-weight:600;color:#111827;line-height:1.3;">Access granted</h1>
              <p style="margin:0 0 28px;font-size:15px;line-height:1.6;color:#374151;">
                An administrator has granted your account access to <strong style="color:#111827;">{merchant}</strong> on Juspay Decision Engine. Sign in with your existing credentials to continue.
              </p>
{button}
              <p style="margin:28px 0 0;font-size:13px;line-height:1.6;color:#6b7280;">
                If you were not expecting this, please contact your account administrator.
              </p>"#,
            merchant = merchant,
            button = primary_button(&login_url, "Sign in"),
        );

        EmailMessage {
            to: self.user_email,
            subject: format!("You now have access to {}", self.merchant_name),
            html_body: render_layout(
                &format!("You now have access to {} on Juspay Decision Engine.", self.merchant_name),
                &content,
            ),
        }
    }
}

pub struct EmailVerificationTemplate {
    pub user_email: String,
    pub verification_url: String,
}

impl EmailVerificationTemplate {
    pub fn into_message(self) -> EmailMessage {
        let content = format!(
            r#"              <h1 style="margin:0 0 14px;font-size:20px;font-weight:600;color:#111827;line-height:1.3;">Confirm your email address</h1>
              <p style="margin:0 0 28px;font-size:15px;line-height:1.6;color:#374151;">
                Please confirm this email address to activate your Juspay Decision Engine account.
              </p>
{button}
              <p style="margin:28px 0 0;font-size:13px;line-height:1.6;color:#6b7280;">
                This link expires in 24 hours. If you did not create an account, you can ignore this email.
              </p>
{fallback}"#,
            button = primary_button(&self.verification_url, "Confirm email address"),
            fallback = fallback_link(&self.verification_url),
        );

        EmailMessage {
            to: self.user_email,
            subject: "Confirm your email address".to_string(),
            html_body: render_layout(
                "Confirm your email address to activate your Juspay Decision Engine account.",
                &content,
            ),
        }
    }
}

pub struct PasswordResetTemplate {
    pub user_email: String,
    pub reset_url: String,
}

impl PasswordResetTemplate {
    pub fn into_message(self) -> EmailMessage {
        let content = format!(
            r#"              <h1 style="margin:0 0 14px;font-size:20px;font-weight:600;color:#111827;line-height:1.3;">Reset your password</h1>
              <p style="margin:0 0 28px;font-size:15px;line-height:1.6;color:#374151;">
                We received a request to reset the password for your Juspay Decision Engine account. Use the button below to choose a new password.
              </p>
{button}
              <p style="margin:28px 0 0;font-size:13px;line-height:1.6;color:#6b7280;">
                This link expires in 1 hour. If you did not request a password reset, you can ignore this email and your password will remain unchanged.
              </p>
{fallback}"#,
            button = primary_button(&self.reset_url, "Reset password"),
            fallback = fallback_link(&self.reset_url),
        );

        EmailMessage {
            to: self.user_email,
            subject: "Reset your password".to_string(),
            html_body: render_layout(
                "Reset the password for your Juspay Decision Engine account.",
                &content,
            ),
        }
    }
}

pub struct InviteUserTemplate {
    pub user_email: String,
    pub merchant_name: String,
    pub temporary_password: String,
    pub base_url: String,
}

impl InviteUserTemplate {
    pub fn into_message(self) -> EmailMessage {
        let merchant = escape_html(&self.merchant_name);
        let email = escape_html(&self.user_email);
        let password = escape_html(&self.temporary_password);
        let login_url = format!("{}/login", self.base_url);
        let content = format!(
            r#"              <h1 style="margin:0 0 14px;font-size:20px;font-weight:600;color:#111827;line-height:1.3;">You have been invited to {merchant}</h1>
              <p style="margin:0 0 28px;font-size:15px;line-height:1.6;color:#374151;">
                You have been added to <strong style="color:#111827;">{merchant}</strong> on Juspay Decision Engine. Sign in with the credentials below, then change your password from your account settings.
              </p>

              <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="background-color:#f9fafb;border:1px solid #e5e7eb;border-radius:8px;margin:0 0 28px;">
                <tr>
                  <td style="padding:16px 20px 12px;">
                    <p style="margin:0 0 4px;font-size:11px;font-weight:600;color:#9ca3af;text-transform:uppercase;letter-spacing:0.05em;">Email</p>
                    <p style="margin:0;font-size:14px;color:#111827;">{email}</p>
                  </td>
                </tr>
                <tr>
                  <td style="border-top:1px solid #e5e7eb;padding:12px 20px 16px;">
                    <p style="margin:0 0 4px;font-size:11px;font-weight:600;color:#9ca3af;text-transform:uppercase;letter-spacing:0.05em;">Temporary password</p>
                    <p style="margin:0;font-size:14px;font-family:'Courier New',Courier,monospace;color:#111827;">{password}</p>
                  </td>
                </tr>
              </table>

{button}
              <p style="margin:28px 0 0;font-size:13px;line-height:1.6;color:#6b7280;">
                For security, change your password after your first sign-in. If you were not expecting this invitation, please contact your account administrator.
              </p>"#,
            merchant = merchant,
            email = email,
            password = password,
            button = primary_button(&login_url, "Sign in"),
        );

        EmailMessage {
            to: self.user_email,
            subject: format!("You've been invited to {}", self.merchant_name),
            html_body: render_layout(
                &format!("You've been added to {} on Juspay Decision Engine.", self.merchant_name),
                &content,
            ),
        }
    }
}
