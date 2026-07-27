use super::EmailMessage;

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub struct MemberAddedTemplate {
    pub user_email: String,
    pub merchant_name: String,
    pub base_url: String,
}

impl MemberAddedTemplate {
    pub fn into_message(self) -> EmailMessage {
        let merchant = escape_html(&self.merchant_name);
        let base_url = escape_html(&self.base_url);
        let html_body = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1.0">
  <title>You've been added to a merchant — Juspay Decision Engine</title>
</head>
<body style="margin:0;padding:0;background-color:#f1f5f9;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <span style="display:none;max-height:0;overflow:hidden;mso-hide:all;">You now have access to {merchant} on Juspay Decision Engine.&#8202;&#65279;&#847;</span>

  <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="background-color:#f1f5f9;">
    <tr>
      <td align="center" style="padding:40px 16px;">
        <table width="600" cellpadding="0" cellspacing="0" role="presentation" style="max-width:600px;width:100%;">

          <tr>
            <td style="background-color:#0b0e14;border-radius:16px 16px 0 0;padding:28px 40px;text-align:center;">
              <span style="font-size:18px;font-weight:700;color:#ffffff;letter-spacing:-0.02em;">
                <span style="color:#3b82f6;">&#9679;</span>&nbsp;Decision Engine
              </span>
            </td>
          </tr>

          <tr>
            <td style="background-color:#ffffff;padding:48px 48px 44px;border-left:1px solid #e2e8f0;border-right:1px solid #e2e8f0;">
              <h1 style="margin:0 0 16px;font-size:22px;font-weight:700;color:#0f172a;letter-spacing:-0.02em;line-height:1.3;">
                You&rsquo;ve been added to a merchant
              </h1>
              <p style="margin:0 0 32px;font-size:15px;line-height:1.75;color:#475569;">
                An admin has granted you access to <strong style="color:#0f172a;">{merchant}</strong> on Juspay Decision Engine.
                Sign in with your existing credentials to access this merchant&rsquo;s routing, analytics, and payment audits.
              </p>

              <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 8px;">
                <tr>
                  <td style="background-color:#4371ff;border-radius:12px;">
                    <a href="{base_url}/login"
                       style="display:inline-block;background-color:#4371ff;color:#ffffff;font-size:15px;font-weight:600;text-decoration:none;padding:14px 32px;border-radius:12px;letter-spacing:-0.01em;">
                      Go to Decision Engine &rarr;
                    </a>
                  </td>
                </tr>
              </table>
            </td>
          </tr>

          <tr>
            <td style="background-color:#f8fafc;border-radius:0 0 16px 16px;padding:24px 48px;border:1px solid #e2e8f0;border-top:none;">
              <p style="margin:0 0 6px;font-size:12px;color:#94a3b8;line-height:1.65;">
                If you weren&rsquo;t expecting this, contact your account administrator.
              </p>
              <p style="margin:10px 0 0;font-size:11px;color:#cbd5e1;">
                Juspay Decision Engine &nbsp;&middot;&nbsp; Automated security email &mdash; please do not reply.
              </p>
            </td>
          </tr>

        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#,
            merchant = merchant,
            base_url = base_url,
        );

        EmailMessage {
            to: self.user_email,
            subject: format!(
                "You've been added to {} on Decision Engine",
                self.merchant_name
            ),
            html_body,
        }
    }
}

pub struct EmailVerificationTemplate {
    pub user_email: String,
    pub verification_url: String,
}

impl EmailVerificationTemplate {
    pub fn into_message(self) -> EmailMessage {
        let url = escape_html(&self.verification_url);
        let html_body = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1.0">
  <title>Confirm your email — Juspay Decision Engine</title>
</head>
<body style="margin:0;padding:0;background-color:#f1f5f9;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <!-- Preheader -->
  <span style="display:none;max-height:0;overflow:hidden;mso-hide:all;">Confirm your email address to activate your Juspay Decision Engine account.&#8202;&#65279;&#847;</span>

  <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="background-color:#f1f5f9;">
    <tr>
      <td align="center" style="padding:40px 16px;">
        <table width="600" cellpadding="0" cellspacing="0" role="presentation" style="max-width:600px;width:100%;">

          <!-- Header -->
          <tr>
            <td style="background-color:#0b0e14;border-radius:16px 16px 0 0;padding:28px 40px;text-align:center;">
              <span style="font-size:18px;font-weight:700;color:#ffffff;letter-spacing:-0.02em;">
                <span style="color:#3b82f6;">&#9679;</span>&nbsp;Decision Engine
              </span>
            </td>
          </tr>

          <!-- Body -->
          <tr>
            <td style="background-color:#ffffff;padding:48px 48px 44px;border-left:1px solid #e2e8f0;border-right:1px solid #e2e8f0;">

              <h1 style="margin:0 0 16px;font-size:22px;font-weight:700;color:#0f172a;letter-spacing:-0.02em;line-height:1.3;">
                Confirm your email address
              </h1>
              <p style="margin:0 0 32px;font-size:15px;line-height:1.75;color:#475569;">
                Thanks for signing up for Juspay Decision Engine. Click the button below to verify your email and activate your account. Once confirmed you'll have full access to gateway routing, analytics, and payment audits.
              </p>

              <!-- CTA button -->
              <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 40px;">
                <tr>
                  <td style="background-color:#4371ff;border-radius:12px;">
                    <a href="{url}"
                       style="display:inline-block;background-color:#4371ff;color:#ffffff;font-size:15px;font-weight:600;text-decoration:none;padding:14px 32px;border-radius:12px;letter-spacing:-0.01em;">
                      Verify email address &rarr;
                    </a>
                  </td>
                </tr>
              </table>

              <!-- Divider -->
              <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 28px;">
                <tr><td style="border-top:1px solid #e2e8f0;"></td></tr>
              </table>

              <p style="margin:0 0 8px;font-size:13px;color:#94a3b8;">
                Button not working? Copy and paste this link into your browser:
              </p>
              <p style="margin:0;font-size:12px;word-break:break-all;line-height:1.6;">
                <a href="{url}" style="color:#3b82f6;text-decoration:none;">{url}</a>
              </p>
            </td>
          </tr>

          <!-- Footer -->
          <tr>
            <td style="background-color:#f8fafc;border-radius:0 0 16px 16px;padding:24px 48px;border:1px solid #e2e8f0;border-top:none;">
              <p style="margin:0 0 6px;font-size:12px;color:#94a3b8;line-height:1.65;">
                This link expires in <strong style="color:#64748b;">24 hours</strong>.
                If you didn&rsquo;t create an account, you can safely ignore this email &mdash; no action is needed and your address will not be used.
              </p>
              <p style="margin:10px 0 0;font-size:11px;color:#cbd5e1;">
                Juspay Decision Engine &nbsp;&middot;&nbsp; Automated security email &mdash; please do not reply.
              </p>
            </td>
          </tr>

        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#,
            url = url
        );

        EmailMessage {
            to: self.user_email,
            subject: "Confirm your email — Juspay Decision Engine".to_string(),
            html_body,
        }
    }
}

pub struct PasswordResetTemplate {
    pub user_email: String,
    pub reset_url: String,
}

impl PasswordResetTemplate {
    pub fn into_message(self) -> EmailMessage {
        let url = escape_html(&self.reset_url);
        let html_body = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1.0">
  <title>Reset your password — Juspay Decision Engine</title>
</head>
<body style="margin:0;padding:0;background-color:#f1f5f9;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <!-- Preheader -->
  <span style="display:none;max-height:0;overflow:hidden;mso-hide:all;">Reset your Juspay Decision Engine password. This link expires in 30 minutes.&#8202;&#65279;&#847;</span>

  <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="background-color:#f1f5f9;">
    <tr>
      <td align="center" style="padding:40px 16px;">
        <table width="600" cellpadding="0" cellspacing="0" role="presentation" style="max-width:600px;width:100%;">

          <!-- Header -->
          <tr>
            <td style="background-color:#0b0e14;border-radius:16px 16px 0 0;padding:28px 40px;text-align:center;">
              <span style="font-size:18px;font-weight:700;color:#ffffff;letter-spacing:-0.02em;">
                <span style="color:#3b82f6;">&#9679;</span>&nbsp;Decision Engine
              </span>
            </td>
          </tr>

          <!-- Body -->
          <tr>
            <td style="background-color:#ffffff;padding:48px 48px 44px;border-left:1px solid #e2e8f0;border-right:1px solid #e2e8f0;">

              <h1 style="margin:0 0 16px;font-size:22px;font-weight:700;color:#0f172a;letter-spacing:-0.02em;line-height:1.3;">
                Reset your password
              </h1>
              <p style="margin:0 0 32px;font-size:15px;line-height:1.75;color:#475569;">
                We received a request to reset the password for your Juspay Decision Engine account. Click the button below to choose a new password.
              </p>

              <!-- CTA button -->
              <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 40px;">
                <tr>
                  <td style="background-color:#4371ff;border-radius:12px;">
                    <a href="{url}"
                       style="display:inline-block;background-color:#4371ff;color:#ffffff;font-size:15px;font-weight:600;text-decoration:none;padding:14px 32px;border-radius:12px;letter-spacing:-0.01em;">
                      Reset password &rarr;
                    </a>
                  </td>
                </tr>
              </table>

              <!-- Divider -->
              <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 28px;">
                <tr><td style="border-top:1px solid #e2e8f0;"></td></tr>
              </table>

              <p style="margin:0 0 8px;font-size:13px;color:#94a3b8;">
                Button not working? Copy and paste this link into your browser:
              </p>
              <p style="margin:0;font-size:12px;word-break:break-all;line-height:1.6;">
                <a href="{url}" style="color:#3b82f6;text-decoration:none;">{url}</a>
              </p>
            </td>
          </tr>

          <!-- Footer -->
          <tr>
            <td style="background-color:#f8fafc;border-radius:0 0 16px 16px;padding:24px 48px;border:1px solid #e2e8f0;border-top:none;">
              <p style="margin:0 0 6px;font-size:12px;color:#94a3b8;line-height:1.65;">
                This link expires in <strong style="color:#64748b;">30 minutes</strong> and can be used only once.
                If you didn&rsquo;t request a password reset, you can safely ignore this email &mdash; your password will not change.
              </p>
              <p style="margin:10px 0 0;font-size:11px;color:#cbd5e1;">
                Juspay Decision Engine &nbsp;&middot;&nbsp; Automated security email &mdash; please do not reply.
              </p>
            </td>
          </tr>

        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#,
            url = url
        );

        EmailMessage {
            to: self.user_email,
            subject: "Reset your password — Juspay Decision Engine".to_string(),
            html_body,
        }
    }
}

pub struct InviteSetPasswordTemplate {
    pub user_email: String,
    pub merchant_name: String,
    pub set_password_url: String,
}

impl InviteSetPasswordTemplate {
    pub fn into_message(self) -> EmailMessage {
        let merchant = escape_html(&self.merchant_name);
        let url = escape_html(&self.set_password_url);
        let html_body = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1.0">
  <title>You're invited to Decision Engine</title>
</head>
<body style="margin:0;padding:0;background-color:#f1f5f9;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;">
  <!-- Preheader -->
  <span style="display:none;max-height:0;overflow:hidden;mso-hide:all;">You&rsquo;ve been invited to {merchant} on Juspay Decision Engine. Set your password to get started.&#8202;&#65279;&#847;</span>

  <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="background-color:#f1f5f9;">
    <tr>
      <td align="center" style="padding:40px 16px;">
        <table width="600" cellpadding="0" cellspacing="0" role="presentation" style="max-width:600px;width:100%;">

          <!-- Header -->
          <tr>
            <td style="background-color:#0b0e14;border-radius:16px 16px 0 0;padding:28px 40px;text-align:center;">
              <span style="font-size:18px;font-weight:700;color:#ffffff;letter-spacing:-0.02em;">
                <span style="color:#3b82f6;">&#9679;</span>&nbsp;Decision Engine
              </span>
            </td>
          </tr>

          <!-- Body -->
          <tr>
            <td style="background-color:#ffffff;padding:48px 48px 44px;border-left:1px solid #e2e8f0;border-right:1px solid #e2e8f0;">

              <h1 style="margin:0 0 16px;font-size:22px;font-weight:700;color:#0f172a;letter-spacing:-0.02em;line-height:1.3;">
                You&rsquo;ve been invited
              </h1>
              <p style="margin:0 0 32px;font-size:15px;line-height:1.75;color:#475569;">
                You&rsquo;ve been invited to <strong style="color:#0f172a;">{merchant}</strong> on Juspay Decision Engine.
                Click the button below to choose your password and activate your account.
              </p>

              <!-- CTA button -->
              <table cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 40px;">
                <tr>
                  <td style="background-color:#4371ff;border-radius:12px;">
                    <a href="{url}"
                       style="display:inline-block;background-color:#4371ff;color:#ffffff;font-size:15px;font-weight:600;text-decoration:none;padding:14px 32px;border-radius:12px;letter-spacing:-0.01em;">
                      Set your password &rarr;
                    </a>
                  </td>
                </tr>
              </table>

              <!-- Divider -->
              <table width="100%" cellpadding="0" cellspacing="0" role="presentation" style="margin:0 0 28px;">
                <tr><td style="border-top:1px solid #e2e8f0;"></td></tr>
              </table>

              <p style="margin:0 0 8px;font-size:13px;color:#94a3b8;">
                Button not working? Copy and paste this link into your browser:
              </p>
              <p style="margin:0;font-size:12px;word-break:break-all;line-height:1.6;">
                <a href="{url}" style="color:#3b82f6;text-decoration:none;">{url}</a>
              </p>
            </td>
          </tr>

          <!-- Footer -->
          <tr>
            <td style="background-color:#f8fafc;border-radius:0 0 16px 16px;padding:24px 48px;border:1px solid #e2e8f0;border-top:none;">
              <p style="margin:0 0 6px;font-size:12px;color:#94a3b8;line-height:1.65;">
                This link expires in <strong style="color:#64748b;">7 days</strong> and can be used only once.
                If you weren&rsquo;t expecting this invitation, you can safely ignore this email &mdash; the account cannot be used until a password is set.
              </p>
              <p style="margin:10px 0 0;font-size:11px;color:#cbd5e1;">
                Juspay Decision Engine &nbsp;&middot;&nbsp; Automated security email &mdash; please do not reply.
              </p>
            </td>
          </tr>

        </table>
      </td>
    </tr>
  </table>
</body>
</html>"#,
            merchant = merchant,
            url = url
        );

        EmailMessage {
            to: self.user_email,
            subject: format!(
                "You're invited to {} on Decision Engine",
                self.merchant_name
            ),
            html_body,
        }
    }
}
