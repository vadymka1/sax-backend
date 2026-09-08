use async_trait::async_trait;
use lettre::message::{header::ContentType, Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use std::sync::Arc;

use crate::config::SmtpConfig;

#[async_trait]
pub trait ContactMailer: Send + Sync {
    async fn send_contact_notification(
        &self,
        recipient: &str,
        name: &str,
        email: &str,
        subject: Option<&str>,
        message: &str,
    ) -> Result<(), String>;
}

pub struct SmtpContactMailer {
    config: SmtpConfig,
}

impl SmtpContactMailer {
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl ContactMailer for SmtpContactMailer {
    async fn send_contact_notification(
        &self,
        recipient: &str,
        name: &str,
        email: &str,
        subject: Option<&str>,
        message: &str,
    ) -> Result<(), String> {
        if recipient.trim().is_empty() {
            return Err("SMTP recipient is empty".to_string());
        }

        let from_mailbox: Mailbox =
            format!("{} <{}>", self.config.from_name, self.config.from_email)
                .parse()
                .map_err(|e| format!("Invalid FROM mailbox: {}", e))?;

        let to_mailbox: Mailbox = recipient
            .parse()
            .map_err(|e| format!("Invalid recipient mailbox: {}", e))?;

        let email_subject = format!("New Contact Request: {}", subject.unwrap_or("No Subject"));

        let email_body = format!(
            "You received a new contact inquiry from {}:\n\nName: {}\nEmail: {}\nSubject: {}\n\nMessage:\n{}",
            self.config.from_name,
            name,
            email,
            subject.unwrap_or("N/A"),
            message
        );

        let email_msg = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(email_subject)
            .header(ContentType::TEXT_PLAIN)
            .body(email_body)
            .map_err(|e| format!("Failed to build email message: {}", e))?;

        let mut builder = if self.config.starttls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                .map_err(|e| format!("Failed to configure SMTP relay: {}", e))?
                .port(self.config.port)
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.config.host)
                .port(self.config.port)
        };

        if let (Some(u), Some(p)) = (&self.config.username, &self.config.password) {
            builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
        }

        let transport = builder.build();
        transport
            .send(email_msg)
            .await
            .map_err(|e| format!("SMTP send failed: {}", e))?;

        Ok(())
    }
}

pub struct NoopContactMailer;

#[async_trait]
impl ContactMailer for NoopContactMailer {
    async fn send_contact_notification(
        &self,
        _recipient: &str,
        _name: &str,
        _email: &str,
        _subject: Option<&str>,
        _message: &str,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub type DynContactMailer = Arc<dyn ContactMailer>;
