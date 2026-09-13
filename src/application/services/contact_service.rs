use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::guards::{ensure_role, AuthenticatedUser};
use crate::application::dto::{AdminContactMessageDto, ContactRequest, ContactResponse};
use crate::application::services::contact_mailer::DynContactMailer;
use crate::config::SmtpConfig;
use crate::domain::users::Role;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

#[derive(sqlx::FromRow)]
struct ContactMessageRow {
    id: Uuid,
    name: String,
    email: String,
    subject: Option<String>,
    message: String,
    email_status: String,
    email_error: Option<String>,
    email_sent_at: Option<DateTime<Utc>>,
    is_read: bool,
    read_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ContactMessageRow> for AdminContactMessageDto {
    fn from(row: ContactMessageRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            email: row.email,
            subject: row.subject,
            message: row.message,
            email_status: row.email_status,
            email_error: row.email_error,
            email_sent_at: row.email_sent_at,
            is_read: row.is_read,
            read_at: row.read_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

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

    /// Sanitizes an SMTP error by redacting configured password and username,
    /// and truncating to a bounded length (500 chars).
    pub fn sanitize_smtp_error(
        raw_error: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> String {
        let mut sanitized = raw_error.to_string();

        if let Some(pass) = password {
            let trimmed = pass.trim();
            if !trimmed.is_empty() {
                sanitized = sanitized.replace(trimmed, "[REDACTED]");
            }
        }

        if let Some(user) = username {
            let trimmed = user.trim();
            if !trimmed.is_empty() {
                sanitized = sanitized.replace(trimmed, "[REDACTED]");
            }
        }

        sanitized.chars().take(500).collect()
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
                    let safe_err = Self::sanitize_smtp_error(
                        &e,
                        self.smtp_config.username.as_deref(),
                        self.smtp_config.password.as_deref(),
                    );
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

    /// List contact messages for admin inbox with optional filter: all, read, unread
    pub async fn list_messages(
        &self,
        auth: &AuthenticatedUser,
        status_filter: Option<&str>,
    ) -> AppResult<Vec<AdminContactMessageDto>> {
        ensure_role(auth, Role::Admin)?;

        let rows: Vec<ContactMessageRow> = match status_filter {
            Some("read") => {
                sqlx::query_as::<_, ContactMessageRow>(
                    r#"
                    SELECT id, name, email, subject, message, email_status, email_error, email_sent_at, is_read, read_at, created_at, updated_at
                    FROM contact_messages
                    WHERE is_read = TRUE
                    ORDER BY created_at DESC, id DESC
                    "#,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?
            }
            Some("unread") => {
                sqlx::query_as::<_, ContactMessageRow>(
                    r#"
                    SELECT id, name, email, subject, message, email_status, email_error, email_sent_at, is_read, read_at, created_at, updated_at
                    FROM contact_messages
                    WHERE is_read = FALSE
                    ORDER BY created_at DESC, id DESC
                    "#,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?
            }
            _ => {
                sqlx::query_as::<_, ContactMessageRow>(
                    r#"
                    SELECT id, name, email, subject, message, email_status, email_error, email_sent_at, is_read, read_at, created_at, updated_at
                    FROM contact_messages
                    ORDER BY created_at DESC, id DESC
                    "#,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?
            }
        };

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get single contact message by ID (does not auto-mark read)
    pub async fn get_message(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
    ) -> AppResult<AdminContactMessageDto> {
        ensure_role(auth, Role::Admin)?;

        let row: Option<ContactMessageRow> = sqlx::query_as::<_, ContactMessageRow>(
            r#"
            SELECT id, name, email, subject, message, email_status, email_error, email_sent_at, is_read, read_at, created_at, updated_at
            FROM contact_messages
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        row.map(Into::into)
            .ok_or_else(|| AppError::NotFound("Contact message not found".to_string()))
    }

    /// Mark message as read (idempotent)
    pub async fn mark_as_read(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
    ) -> AppResult<AdminContactMessageDto> {
        ensure_role(auth, Role::Admin)?;

        let row: Option<ContactMessageRow> = sqlx::query_as::<_, ContactMessageRow>(
            r#"
            UPDATE contact_messages
            SET is_read = TRUE,
                read_at = COALESCE(read_at, CURRENT_TIMESTAMP),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING id, name, email, subject, message, email_status, email_error, email_sent_at, is_read, read_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        row.map(Into::into)
            .ok_or_else(|| AppError::NotFound("Contact message not found".to_string()))
    }

    /// Mark message as unread (idempotent)
    pub async fn mark_as_unread(
        &self,
        auth: &AuthenticatedUser,
        id: Uuid,
    ) -> AppResult<AdminContactMessageDto> {
        ensure_role(auth, Role::Admin)?;

        let row: Option<ContactMessageRow> = sqlx::query_as::<_, ContactMessageRow>(
            r#"
            UPDATE contact_messages
            SET is_read = FALSE,
                read_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1
            RETURNING id, name, email, subject, message, email_status, email_error, email_sent_at, is_read, read_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        row.map(Into::into)
            .ok_or_else(|| AppError::NotFound("Contact message not found".to_string()))
    }
}
