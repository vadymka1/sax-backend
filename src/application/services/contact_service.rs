use sqlx::PgPool;
use uuid::Uuid;

use crate::application::dto::{ContactRequest, ContactResponse};
use crate::application::services::contact_mailer::DynContactMailer;
use crate::config::SmtpConfig;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

pub struct ContactService {
    pool: PgPool,
    mailer: DynContactMailer,
    smtp_config: SmtpConfig,
}

impl ContactService {
    pub fn new(pool: PgPool, mailer: DynContactMailer, smtp_config: SmtpConfig) -> Self {
        Self {
            pool,
            mailer,
            smtp_config,
        }
    }

    pub fn validate_request(req: &ContactRequest) -> AppResult<()> {
        let mut errors = Vec::new();

        let name = req.name.trim();
        if name.is_empty() {
            errors.push(ApiErrorDetails {
                field: "name".to_string(),
                message: "Name cannot be empty".to_string(),
            });
        } else if name.chars().count() > 100 {
            errors.push(ApiErrorDetails {
                field: "name".to_string(),
                message: "Name cannot exceed 100 characters".to_string(),
            });
        }

        let email = req.email.trim();
        if email.is_empty() {
            errors.push(ApiErrorDetails {
                field: "email".to_string(),
                message: "Email cannot be empty".to_string(),
            });
        } else if email.chars().count() > 255 {
            errors.push(ApiErrorDetails {
                field: "email".to_string(),
                message: "Email cannot exceed 255 characters".to_string(),
            });
        } else {
            // Validate syntactic structure of email
            let is_valid_email = email.contains('@')
                && email.split('@').count() == 2
                && !email.starts_with('@')
                && !email.ends_with('@')
                && email.split('@').nth(1).is_some_and(|domain| {
                    domain.contains('.')
                        && !domain.starts_with('.')
                        && !domain.ends_with('.')
                        && domain.split('.').all(|part| !part.is_empty())
                });

            if !is_valid_email {
                errors.push(ApiErrorDetails {
                    field: "email".to_string(),
                    message: "Invalid email format".to_string(),
                });
            }
        }

        let message = req.message.trim();
        if message.is_empty() {
            errors.push(ApiErrorDetails {
                field: "message".to_string(),
                message: "Message cannot be empty".to_string(),
            });
        } else if message.chars().count() > 5000 {
            errors.push(ApiErrorDetails {
                field: "message".to_string(),
                message: "Message cannot exceed 5000 characters".to_string(),
            });
        }

        if let Some(ref subj) = req.subject {
            if subj.trim().chars().count() > 200 {
                errors.push(ApiErrorDetails {
                    field: "subject".to_string(),
                    message: "Subject cannot exceed 200 characters".to_string(),
                });
            }
        }

        if !errors.is_empty() {
            return Err(AppError::ValidationError(errors));
        }

        Ok(())
    }

    pub async fn submit_contact(&self, req: ContactRequest) -> AppResult<ContactResponse> {
        Self::validate_request(&req)?;

        let id = Uuid::new_v4();
        let name = req.name.trim().to_string();
        let email = req.email.trim().to_string();
        let subject = req
            .subject
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let message = req.message.trim().to_string();

        // 1. Authoritative persistence in PostgreSQL
        sqlx::query(
            r#"
            INSERT INTO contact_messages (id, name, email, subject, message, email_status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, 'pending', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(id)
        .bind(&name)
        .bind(&email)
        .bind(&subject)
        .bind(&message)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 2. SMTP notification attempt (isolated from client result)
        if self.smtp_config.enabled {
            let recipient = if !self
                .smtp_config
                .contact_notification_email
                .trim()
                .is_empty()
            {
                self.smtp_config.contact_notification_email.clone()
            } else {
                self.smtp_config.from_email.clone()
            };

            match self
                .mailer
                .send_contact_notification(&recipient, &name, &email, subject.as_deref(), &message)
                .await
            {
                Ok(()) => {
                    let _ = sqlx::query(
                        r#"
                        UPDATE contact_messages
                        SET email_status = 'sent', email_sent_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
                        WHERE id = $1
                        "#,
                    )
                    .bind(id)
                    .execute(&self.pool)
                    .await;

                    tracing::info!(
                        contact_message_id = %id,
                        email_status = "sent",
                        "Contact notification email sent successfully"
                    );
                }
                Err(e) => {
                    let mut sanitized = e;
                    if let Some(ref pass) = self.smtp_config.password {
                        if !pass.is_empty() {
                            sanitized = sanitized.replace(pass, "[REDACTED]");
                        }
                    }
                    let safe_err: String = sanitized.chars().take(500).collect();
                    let _ = sqlx::query(
                        r#"
                        UPDATE contact_messages
                        SET email_status = 'failed', email_error = $2, updated_at = CURRENT_TIMESTAMP
                        WHERE id = $1
                        "#,
                    )
                    .bind(id)
                    .bind(&safe_err)
                    .execute(&self.pool)
                    .await;

                    tracing::warn!(
                        contact_message_id = %id,
                        email_status = "failed",
                        error = %safe_err,
                        "Contact notification email delivery failed (submission persisted safely)"
                    );
                }
            }
        } else {
            let _ = sqlx::query(
                r#"
                UPDATE contact_messages
                SET email_status = 'disabled', updated_at = CURRENT_TIMESTAMP
                WHERE id = $1
                "#,
            )
            .bind(id)
            .execute(&self.pool)
            .await;

            tracing::info!(
                contact_message_id = %id,
                email_status = "disabled",
                "Contact submission saved; SMTP is disabled"
            );
        }

        Ok(ContactResponse {
            status: "success".to_string(),
            message: "Contact request submitted successfully".to_string(),
        })
    }
}
