use serde_json::Value;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::shared::errors::{AppError, AppResult};

pub struct AuditLogger<'a> {
    pool: &'a PgPool,
}

impl<'a> AuditLogger<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn log_action(
        &self,
        actor_id: Option<Uuid>,
        action: &str,
        entity_type: &str,
        entity_id: Option<Uuid>,
        old_values: Option<Value>,
        new_values: Option<Value>,
        _ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> AppResult<()> {
        info!(
            actor = ?actor_id,
            action = action,
            entity = entity_type,
            entity_id = ?entity_id,
            "Audit Log Created"
        );

        sqlx::query(
            r#"
            INSERT INTO audit_logs (id, actor_user_id, action, entity_type, entity_id, old_values, new_values, user_agent)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(Uuid::new_v4())
        .bind(actor_id)
        .bind(action)
        .bind(entity_type)
        .bind(entity_id)
        .bind(old_values)
        .bind(new_values)
        .bind(user_agent)
        .execute(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
