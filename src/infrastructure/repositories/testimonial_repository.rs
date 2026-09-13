use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::application::dto::ReorderTestimonialItem;
use crate::domain::testimonials::{TestimonialModerationStatus, TestimonialSubmissionSource};
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TestimonialRowWithMedia {
    pub id: Uuid,
    pub author_name: String,
    pub author_role: Option<String>,
    pub text: String,
    pub avatar_media_id: Option<Uuid>,
    pub sort_order: i32,
    pub is_visible: bool,
    pub moderation_status: TestimonialModerationStatus,
    pub submission_source: TestimonialSubmissionSource,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub media_id: Option<Uuid>,
    pub media_storage_key: Option<String>,
    pub media_alt_text: Option<String>,
}

/// Deterministic 64-bit advisory lock key dedicated to serializing testimonial creation sort_order allocation.
/// Value derived from ascii prefix "spa_test": 0x7370615f74657374 (8318236166184514932 i64).
pub const TESTIMONIAL_CREATE_ORDER_LOCK_KEY: i64 = 8_318_236_166_184_514_932;

pub struct TestimonialRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TestimonialRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_active(&self) -> AppResult<Vec<TestimonialRowWithMedia>> {
        sqlx::query_as::<_, TestimonialRowWithMedia>(
            r#"
            SELECT
                t.id,
                t.author_name,
                t.author_role,
                t.text,
                t.avatar_media_id,
                t.sort_order,
                t.is_visible,
                t.moderation_status,
                t.submission_source,
                t.created_at,
                t.updated_at,
                t.deleted_at,
                m.id AS media_id,
                m.storage_key AS media_storage_key,
                m.alt_text AS media_alt_text
            FROM testimonials t
            LEFT JOIN media_assets m ON t.avatar_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE t.deleted_at IS NULL
            ORDER BY t.sort_order ASC, t.id ASC
            "#,
        )
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn list_public_visible(&self) -> AppResult<Vec<TestimonialRowWithMedia>> {
        sqlx::query_as::<_, TestimonialRowWithMedia>(
            r#"
            SELECT
                t.id,
                t.author_name,
                t.author_role,
                t.text,
                t.avatar_media_id,
                t.sort_order,
                t.is_visible,
                t.moderation_status,
                t.submission_source,
                t.created_at,
                t.updated_at,
                t.deleted_at,
                m.id AS media_id,
                m.storage_key AS media_storage_key,
                m.alt_text AS media_alt_text
            FROM testimonials t
            LEFT JOIN media_assets m ON t.avatar_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE t.deleted_at IS NULL AND t.is_visible = TRUE AND t.moderation_status = 'approved'
            ORDER BY t.sort_order ASC, t.id ASC
            "#,
        )
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<TestimonialRowWithMedia>> {
        sqlx::query_as::<_, TestimonialRowWithMedia>(
            r#"
            SELECT
                t.id,
                t.author_name,
                t.author_role,
                t.text,
                t.avatar_media_id,
                t.sort_order,
                t.is_visible,
                t.moderation_status,
                t.submission_source,
                t.created_at,
                t.updated_at,
                t.deleted_at,
                m.id AS media_id,
                m.storage_key AS media_storage_key,
                m.alt_text AS media_alt_text
            FROM testimonials t
            LEFT JOIN media_assets m ON t.avatar_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE t.id = $1 AND t.deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn create(
        &self,
        author_name: &str,
        author_role: Option<&str>,
        text: &str,
        avatar_media_id: Option<Uuid>,
        is_visible: bool,
    ) -> AppResult<TestimonialRowWithMedia> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Serialize next sort_order allocation so concurrent creates cannot choose the same value.
        // pg_advisory_xact_lock is transaction-scoped: automatically released on COMMIT or ROLLBACK.
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(TESTIMONIAL_CREATE_ORDER_LOCK_KEY)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 1. Calculate next sort order among approved testimonials inside transaction
        let max_order: (i32,) = sqlx::query_as(
            "SELECT COALESCE(MAX(sort_order), 0) + 10 FROM testimonials WHERE deleted_at IS NULL AND moderation_status = 'approved'",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let sort_order = max_order.0;
        let id = Uuid::new_v4();

        // 2. Insert row with approved status and admin source
        sqlx::query(
            r#"
            INSERT INTO testimonials (id, author_name, author_role, text, avatar_media_id, sort_order, is_visible, moderation_status, submission_source)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'approved', 'admin')
            "#,
        )
        .bind(id)
        .bind(author_name)
        .bind(author_role)
        .bind(text)
        .bind(avatar_media_id)
        .bind(sort_order)
        .bind(is_visible)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 3. Fetch newly created row joined with media asset
        let row = sqlx::query_as::<_, TestimonialRowWithMedia>(
            r#"
            SELECT
                t.id,
                t.author_name,
                t.author_role,
                t.text,
                t.avatar_media_id,
                t.sort_order,
                t.is_visible,
                t.moderation_status,
                t.submission_source,
                t.created_at,
                t.updated_at,
                t.deleted_at,
                m.id AS media_id,
                m.storage_key AS media_storage_key,
                m.alt_text AS media_alt_text
            FROM testimonials t
            LEFT JOIN media_assets m ON t.avatar_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE t.id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(row)
    }

    pub async fn create_public(
        &self,
        author_name: &str,
        author_role: Option<&str>,
        text: &str,
    ) -> AppResult<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO testimonials (id, author_name, author_role, text, avatar_media_id, sort_order, is_visible, moderation_status, submission_source)
            VALUES ($1, $2, $3, $4, NULL, 0, FALSE, 'pending', 'public')
            "#,
        )
        .bind(id)
        .bind(author_name)
        .bind(author_role)
        .bind(text)
        .execute(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(id)
    }

    pub async fn approve(&self, id: Uuid) -> AppResult<Option<TestimonialRowWithMedia>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Shared advisory lock ensures approve and admin create cannot race on sort_order allocation
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(TESTIMONIAL_CREATE_ORDER_LOCK_KEY)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let existing: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, moderation_status FROM testimonials WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let (target_id, current_status) = match existing {
            Some(row) => row,
            None => return Ok(None),
        };

        if current_status != "approved" {
            // Calculate next canonical order among approved testimonials
            let max_order: (i32,) = sqlx::query_as(
                "SELECT COALESCE(MAX(sort_order), 0) + 10 FROM testimonials WHERE deleted_at IS NULL AND moderation_status = 'approved'",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

            let next_order = max_order.0;

            sqlx::query(
                "UPDATE testimonials SET moderation_status = 'approved', is_visible = TRUE, sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            )
            .bind(next_order)
            .bind(target_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        let row = sqlx::query_as::<_, TestimonialRowWithMedia>(
            r#"
            SELECT
                t.id,
                t.author_name,
                t.author_role,
                t.text,
                t.avatar_media_id,
                t.sort_order,
                t.is_visible,
                t.moderation_status,
                t.submission_source,
                t.created_at,
                t.updated_at,
                t.deleted_at,
                m.id AS media_id,
                m.storage_key AS media_storage_key,
                m.alt_text AS media_alt_text
            FROM testimonials t
            LEFT JOIN media_assets m ON t.avatar_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE t.id = $1
            "#,
        )
        .bind(target_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(Some(row))
    }

    pub async fn reject(&self, id: Uuid) -> AppResult<Option<TestimonialRowWithMedia>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let existing: Option<(Uuid, String)> = sqlx::query_as(
            "SELECT id, moderation_status FROM testimonials WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let (target_id, current_status) = match existing {
            Some(row) => row,
            None => return Ok(None),
        };

        if current_status != "rejected" {
            sqlx::query(
                "UPDATE testimonials SET moderation_status = 'rejected', is_visible = FALSE, sort_order = 0, updated_at = CURRENT_TIMESTAMP WHERE id = $1",
            )
            .bind(target_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        let row = sqlx::query_as::<_, TestimonialRowWithMedia>(
            r#"
            SELECT
                t.id,
                t.author_name,
                t.author_role,
                t.text,
                t.avatar_media_id,
                t.sort_order,
                t.is_visible,
                t.moderation_status,
                t.submission_source,
                t.created_at,
                t.updated_at,
                t.deleted_at,
                m.id AS media_id,
                m.storage_key AS media_storage_key,
                m.alt_text AS media_alt_text
            FROM testimonials t
            LEFT JOIN media_assets m ON t.avatar_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE t.id = $1
            "#,
        )
        .bind(target_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(Some(row))
    }

    pub async fn update(
        &self,
        id: Uuid,
        author_name: Option<&str>,
        author_role: Option<Option<&str>>,
        text: Option<&str>,
        avatar_media_id: Option<Option<Uuid>>,
        is_visible: Option<bool>,
    ) -> AppResult<Option<TestimonialRowWithMedia>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Lock row FOR UPDATE to prevent concurrent race conditions
        let existing: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM testimonials WHERE id = $1 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if existing.is_none() {
            return Ok(None);
        }

        // Apply author_name update
        if let Some(name) = author_name {
            sqlx::query("UPDATE testimonials SET author_name = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
                .bind(name)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        // Apply author_role update (distinguishes omitted vs null)
        if let Some(opt_role) = author_role {
            sqlx::query("UPDATE testimonials SET author_role = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
                .bind(opt_role)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        // Apply text update
        if let Some(txt) = text {
            sqlx::query(
                "UPDATE testimonials SET text = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            )
            .bind(txt)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        // Apply avatar_media_id update (distinguishes omitted vs null)
        if let Some(opt_mid) = avatar_media_id {
            sqlx::query("UPDATE testimonials SET avatar_media_id = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
                .bind(opt_mid)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        // Apply is_visible update
        if let Some(vis) = is_visible {
            sqlx::query("UPDATE testimonials SET is_visible = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
                .bind(vis)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        let row = sqlx::query_as::<_, TestimonialRowWithMedia>(
            r#"
            SELECT
                t.id,
                t.author_name,
                t.author_role,
                t.text,
                t.avatar_media_id,
                t.sort_order,
                t.is_visible,
                t.moderation_status,
                t.submission_source,
                t.created_at,
                t.updated_at,
                t.deleted_at,
                m.id AS media_id,
                m.storage_key AS media_storage_key,
                m.alt_text AS media_alt_text
            FROM testimonials t
            LEFT JOIN media_assets m ON t.avatar_media_id = m.id AND m.deleted_at IS NULL AND m.status = 'active'
            WHERE t.id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(Some(row))
    }

    pub async fn soft_delete(&self, id: Uuid) -> AppResult<bool> {
        let res = sqlx::query(
            "UPDATE testimonials SET deleted_at = CURRENT_TIMESTAMP, is_visible = FALSE, updated_at = CURRENT_TIMESTAMP WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    pub async fn reorder_testimonials_tx(&self, items: &[ReorderTestimonialItem]) -> AppResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 1. Fetch active approved IDs from DB
        let active_rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM testimonials WHERE deleted_at IS NULL AND moderation_status = 'approved' ORDER BY sort_order ASC, id ASC",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let active_ids: Vec<Uuid> = active_rows.into_iter().map(|r| r.0).collect();
        let db_ids_set: HashSet<Uuid> = active_ids.iter().cloned().collect();

        // 2. Validate request item counts and uniqueness
        if items.len() != db_ids_set.len() {
            return Err(AppError::ValidationError(vec![ApiErrorDetails {
                field: "items".to_string(),
                message: format!(
                    "Reorder items count ({}) does not match active approved testimonials count ({})",
                    items.len(),
                    db_ids_set.len()
                ),
            }]));
        }

        let mut req_ids_set = HashSet::new();

        for item in items {
            if item.sort_order < 0 {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!("sort_order cannot be negative: {}", item.sort_order),
                }]));
            }

            if !db_ids_set.contains(&item.id) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!(
                        "Referenced testimonial ID {} is invalid, not approved, or deleted",
                        item.id
                    ),
                }]));
            }

            if !req_ids_set.insert(item.id) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!("Duplicate testimonial ID in reorder request: {}", item.id),
                }]));
            }
        }

        // 3. Deterministically canonicalize sort order using (sort_order, original_request_index) tie-breaking
        let mut indexed_items: Vec<(usize, &ReorderTestimonialItem)> =
            items.iter().enumerate().collect();
        indexed_items.sort_by(|(idx_a, item_a), (idx_b, item_b)| {
            item_a
                .sort_order
                .cmp(&item_b.sort_order)
                .then_with(|| idx_a.cmp(idx_b))
        });
        let final_ordered_ids: Vec<Uuid> =
            indexed_items.into_iter().map(|(_, item)| item.id).collect();

        // 4. Two-phase update to guarantee collision safety with check constraint (sort_order >= 0)
        // Phase 1: assign temporary high positive values (1_000_000 + idx)
        for (idx, id) in final_ordered_ids.iter().enumerate() {
            let temp_order = 1_000_000 + (idx as i32);
            sqlx::query(
                "UPDATE testimonials SET sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            )
            .bind(temp_order)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        // Phase 2: assign final canonical values (10, 20, 30, ...)
        for (idx, id) in final_ordered_ids.iter().enumerate() {
            let canonical_order = (idx as i32 + 1) * 10;
            sqlx::query(
                "UPDATE testimonials SET sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            )
            .bind(canonical_order)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}
