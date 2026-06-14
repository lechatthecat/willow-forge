//! Mailer service. Laravel-like mail sending with `log` and `smtp` drivers.
//!
//! Configured from `src/config/mail.toml`, with `MAIL_*` environment overrides
//! (see [`MailConfig`]). The
//! built [`Mailer`] lives in [`crate::Services`], so handlers send mail through
//! the request context:
//!
//! ```rust,ignore
//! use willow_forge_runtime::Email;
//!
//! let email = Email::new("user@example.com", "Welcome")
//!     .html("<h1>Hi!</h1>")
//!     .text("Hi!");
//! ctx.state.services.mailer.send(&email).await?;
//! ```
//!
//! The default `log` driver renders the message to the tracing log instead of
//! delivering it — safe for local development with no SMTP server running.

use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::app_errors::AppError;

/// Mail configuration loaded from config files and optional `MAIL_*` overrides.
#[derive(Debug, Clone)]
pub struct MailConfig {
    /// `MAIL_MAILER`: `"log"` (default) or `"smtp"`.
    pub driver: String,
    /// `MAIL_HOST`: SMTP server hostname.
    pub host: String,
    /// `MAIL_PORT`: SMTP server port.
    pub port: u16,
    /// `MAIL_USERNAME`: SMTP auth username (empty = no authentication).
    pub username: String,
    /// `MAIL_PASSWORD`: SMTP auth password.
    pub password: String,
    /// `MAIL_ENCRYPTION`: `"tls"`, `"starttls"`, or `"none"`.
    pub encryption: String,
    /// `MAIL_FROM_ADDRESS`: default sender address.
    pub from_address: String,
    /// `MAIL_FROM_NAME`: default sender display name.
    pub from_name: String,
}

impl Default for MailConfig {
    fn default() -> Self {
        Self {
            driver: "log".to_string(),
            host: "127.0.0.1".to_string(),
            port: 2525,
            username: String::new(),
            password: String::new(),
            encryption: "none".to_string(),
            from_address: "hello@example.com".to_string(),
            from_name: "Willow Forge".to_string(),
        }
    }
}

/// A composable email message built with a fluent API.
#[derive(Debug, Clone)]
pub struct Email {
    /// Recipient address (`"Name <addr>"` or bare `"addr"`).
    pub to: String,
    /// Subject line.
    pub subject: String,
    /// Optional HTML body.
    pub html: Option<String>,
    /// Optional plain-text body.
    pub text: Option<String>,
}

impl Email {
    /// Start a new email to `to` with `subject`.
    pub fn new(to: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            to: to.into(),
            subject: subject.into(),
            html: None,
            text: None,
        }
    }

    /// Set the HTML body.
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html = Some(html.into());
        self
    }

    /// Set the plain-text body.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }
}

/// Mail sender. Cheap to clone; held in [`crate::Services`].
#[derive(Clone)]
pub enum Mailer {
    /// Renders messages to the tracing log instead of delivering them.
    Log { from: Mailbox },
    /// Delivers messages over SMTP via lettre.
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        from: Mailbox,
    },
}

impl Mailer {
    /// Build a mailer from configuration. Does not open any network connection.
    pub fn from_config(cfg: &MailConfig) -> Result<Self, AppError> {
        let from = Self::mailbox(&cfg.from_name, &cfg.from_address)?;
        match cfg.driver.as_str() {
            "log" => Ok(Mailer::Log { from }),
            "smtp" => {
                let mut builder = match cfg.encryption.as_str() {
                    "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)
                        .map_err(|e| AppError::Mail(e.to_string()))?,
                    "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
                        .map_err(|e| AppError::Mail(e.to_string()))?,
                    _ => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host),
                };
                builder = builder.port(cfg.port);
                if !cfg.username.is_empty() {
                    builder = builder.credentials(Credentials::new(
                        cfg.username.clone(),
                        cfg.password.clone(),
                    ));
                }
                Ok(Mailer::Smtp {
                    transport: builder.build(),
                    from,
                })
            }
            other => Err(AppError::Mail(format!(
                "unknown MAIL_MAILER driver `{other}` (expected `log` or `smtp`)"
            ))),
        }
    }

    /// Send an email. With the `log` driver this renders the message to the
    /// tracing log under target `mailer` instead of delivering it.
    pub async fn send(&self, email: &Email) -> Result<(), AppError> {
        let message = self.build_message(email)?;
        match self {
            Mailer::Log { .. } => {
                tracing::info!(
                    target: "mailer",
                    "log driver — email not delivered:\n{}",
                    String::from_utf8_lossy(&message.formatted()),
                );
                Ok(())
            }
            Mailer::Smtp { transport, .. } => {
                transport
                    .send(message)
                    .await
                    .map_err(|e| AppError::Mail(e.to_string()))?;
                Ok(())
            }
        }
    }

    fn build_message(&self, email: &Email) -> Result<Message, AppError> {
        let from = match self {
            Mailer::Log { from } | Mailer::Smtp { from, .. } => from.clone(),
        };
        let to: Mailbox = email
            .to
            .parse()
            .map_err(|e| AppError::Mail(format!("invalid recipient `{}`: {e}", email.to)))?;

        let builder = Message::builder()
            .from(from)
            .to(to)
            .subject(email.subject.as_str());

        let message = match (&email.html, &email.text) {
            (Some(html), Some(text)) => {
                builder.multipart(MultiPart::alternative_plain_html(text.clone(), html.clone()))
            }
            (Some(html), None) => builder.singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_HTML)
                    .body(html.clone()),
            ),
            (None, Some(text)) => builder.singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(text.clone()),
            ),
            (None, None) => builder.singlepart(
                SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(String::new()),
            ),
        }
        .map_err(|e| AppError::Mail(e.to_string()))?;

        Ok(message)
    }

    fn mailbox(name: &str, address: &str) -> Result<Mailbox, AppError> {
        let addr = address
            .parse()
            .map_err(|e| AppError::Mail(format!("invalid mail address `{address}`: {e}")))?;
        Ok(Mailbox::new(
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            },
            addr,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_mailer() -> Mailer {
        Mailer::from_config(&MailConfig::default()).unwrap()
    }

    #[tokio::test]
    async fn ml_01_log_driver_text_only_sends_ok() {
        let email = Email::new("user@example.com", "Hi").text("hello");
        assert!(log_mailer().send(&email).await.is_ok());
    }

    #[tokio::test]
    async fn ml_02_html_and_text_multipart_sends_ok() {
        let email = Email::new("user@example.com", "Hi")
            .html("<b>hi</b>")
            .text("hi");
        assert!(log_mailer().send(&email).await.is_ok());
    }

    #[tokio::test]
    async fn ml_03_html_only_sends_ok() {
        let email = Email::new("user@example.com", "Hi").html("<b>hi</b>");
        assert!(log_mailer().send(&email).await.is_ok());
    }

    #[tokio::test]
    async fn ml_04_empty_body_sends_ok() {
        let email = Email::new("user@example.com", "Hi");
        assert!(log_mailer().send(&email).await.is_ok());
    }

    #[tokio::test]
    async fn ml_05_invalid_recipient_errors() {
        let email = Email::new("not-an-email", "Hi").text("x");
        assert!(matches!(
            log_mailer().send(&email).await,
            Err(AppError::Mail(_))
        ));
    }

    #[tokio::test]
    async fn ml_06_named_recipient_sends_ok() {
        let email = Email::new("Alice <alice@example.com>", "Hi").text("x");
        assert!(log_mailer().send(&email).await.is_ok());
    }

    #[test]
    fn ml_07_default_driver_is_log() {
        assert_eq!(MailConfig::default().driver, "log");
        assert!(matches!(log_mailer(), Mailer::Log { .. }));
    }

    #[test]
    fn ml_08_smtp_driver_builds_without_connecting() {
        let cfg = MailConfig {
            driver: "smtp".into(),
            encryption: "none".into(),
            ..MailConfig::default()
        };
        assert!(matches!(Mailer::from_config(&cfg), Ok(Mailer::Smtp { .. })));
    }

    #[test]
    fn ml_09_smtp_starttls_builds() {
        let cfg = MailConfig {
            driver: "smtp".into(),
            encryption: "starttls".into(),
            host: "smtp.example.com".into(),
            ..MailConfig::default()
        };
        assert!(matches!(Mailer::from_config(&cfg), Ok(Mailer::Smtp { .. })));
    }

    #[test]
    fn ml_10_unknown_driver_errors() {
        let cfg = MailConfig {
            driver: "carrier-pigeon".into(),
            ..MailConfig::default()
        };
        assert!(matches!(Mailer::from_config(&cfg), Err(AppError::Mail(_))));
    }

    #[test]
    fn ml_11_invalid_from_address_errors() {
        let cfg = MailConfig {
            from_address: "nonsense".into(),
            ..MailConfig::default()
        };
        assert!(matches!(Mailer::from_config(&cfg), Err(AppError::Mail(_))));
    }
}
