//! Transactional email over SMTP. Credentials are optional: when they're not
//! configured the mailer logs the message instead of sending, so the flows work
//! end-to-end in development without a mail server.

use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::Settings;

#[derive(Clone)]
pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: String,
    app_base_url: String,
}

impl Mailer {
    /// Build from settings. Missing SMTP host/user/password → dev log mode.
    pub fn from_settings(s: &Settings) -> Self {
        let from = s
            .smtp_from
            .clone()
            .unwrap_or_else(|| "Ferrite Stream <no-reply@ferrite.local>".to_string());

        let transport = match (&s.smtp_host, &s.smtp_user, &s.smtp_password) {
            (Some(host), Some(user), Some(pass)) => {
                match AsyncSmtpTransport::<Tokio1Executor>::relay(host) {
                    Ok(builder) => Some(
                        builder
                            .port(s.smtp_port.unwrap_or(587))
                            .credentials(Credentials::new(user.clone(), pass.clone()))
                            .build(),
                    ),
                    Err(e) => {
                        tracing::error!(error = %e, "invalid SMTP host; falling back to log mode");
                        None
                    }
                }
            }
            _ => {
                tracing::info!("SMTP not configured — emails will be logged, not sent");
                None
            }
        };

        Self {
            transport,
            from,
            app_base_url: s.app_base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Send an email. Failures are logged, never propagated — a flaky mail
    /// server must not fail the API request that triggered the send.
    pub async fn send(&self, to: &str, subject: &str, body: String) {
        let Some(transport) = &self.transport else {
            tracing::info!(%to, %subject, "[email:dev]\n{body}");
            return;
        };

        let message = match (self.from.parse(), to.parse()) {
            (Ok(from), Ok(to_mbox)) => Message::builder()
                .from(from)
                .to(to_mbox)
                .subject(subject)
                .body(body),
            _ => {
                tracing::error!(%to, "invalid email address; not sending");
                return;
            }
        };

        match message {
            Ok(msg) => {
                if let Err(e) = transport.send(msg).await {
                    tracing::error!(error = %e, %to, "failed to send email");
                }
            }
            Err(e) => tracing::error!(error = %e, "failed to build email message"),
        }
    }

    pub async fn send_password_reset(&self, to: &str, token: &str) {
        let url = format!("{}/reset-password?token={}", self.app_base_url, token);
        let body = format!(
            "Someone requested a password reset for your Ferrite Stream account.\n\n\
             Reset it here (link valid for 1 hour):\n{url}\n\n\
             If you didn't request this, you can safely ignore this email."
        );
        self.send(to, "Reset your Ferrite Stream password", body).await;
    }

    pub async fn send_invite(&self, to: &str, workspace: &str, temp_password: &str) {
        let login_url = format!("{}/app", self.app_base_url);
        let body = format!(
            "You've been invited to the \"{workspace}\" workspace on Ferrite Stream.\n\n\
             Sign in here:\n{login_url}\n\n\
             Email: {to}\n\
             Temporary password: {temp_password}\n\n\
             Please change your password after signing in."
        );
        self.send(
            to,
            &format!("You're invited to {workspace} on Ferrite Stream"),
            body,
        )
        .await;
    }
}
