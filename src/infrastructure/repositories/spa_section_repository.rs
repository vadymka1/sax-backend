use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::application::dto::{AdminSpaSectionDto, ReorderSpaSectionItem};
use crate::domain::sections::SpaSection;
use crate::shared::errors::{ApiErrorDetails, AppError, AppResult};

pub struct SpaSectionRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> SpaSectionRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, id: Uuid) -> AppResult<Option<SpaSection>> {
        sqlx::query_as::<_, SpaSection>(
            r#"
            SELECT id, page_id, section_key, title, navigation_label, sort_order, is_visible, created_at, updated_at
            FROM spa_sections
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn find_by_key(
        &self,
        page_id: Uuid,
        section_key: &str,
    ) -> AppResult<Option<SpaSection>> {
        sqlx::query_as::<_, SpaSection>(
            r#"
            SELECT id, page_id, section_key, title, navigation_label, sort_order, is_visible, created_at, updated_at
            FROM spa_sections
            WHERE page_id = $1 AND section_key = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(page_id)
        .bind(section_key)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn list_for_page(&self, page_id: Uuid) -> AppResult<Vec<SpaSection>> {
        sqlx::query_as::<_, SpaSection>(
            r#"
            SELECT id, page_id, section_key, title, navigation_label, sort_order, is_visible, created_at, updated_at
            FROM spa_sections
            WHERE page_id = $1 AND deleted_at IS NULL
            ORDER BY sort_order ASC, id ASC
            "#,
        )
        .bind(page_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn exists(&self, id: Uuid) -> AppResult<bool> {
        let res: (bool,) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM spa_sections WHERE id = $1 AND deleted_at IS NULL)",
        )
        .bind(id)
        .fetch_one(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(res.0)
    }

    pub async fn list_admin_for_page(&self, page_id: Uuid) -> AppResult<Vec<AdminSpaSectionDto>> {
        sqlx::query_as::<_, AdminSpaSectionDto>(
            r#"
            SELECT
                s.id,
                s.section_key AS key,
                s.title,
                s.navigation_label,
                s.sort_order,
                s.is_visible,
                COALESCE(COUNT(b.id) FILTER (WHERE b.deleted_at IS NULL), 0) AS content_block_count,
                s.created_at,
                s.updated_at
            FROM spa_sections s
            LEFT JOIN sections b ON b.spa_section_id = s.id
            WHERE s.page_id = $1 AND s.deleted_at IS NULL
            GROUP BY s.id
            ORDER BY s.sort_order ASC, s.id ASC
            "#,
        )
        .bind(page_id)
        .fetch_all(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn find_admin_by_id(
        &self,
        page_id: Uuid,
        id: Uuid,
    ) -> AppResult<Option<AdminSpaSectionDto>> {
        sqlx::query_as::<_, AdminSpaSectionDto>(
            r#"
            SELECT
                s.id,
                s.section_key AS key,
                s.title,
                s.navigation_label,
                s.sort_order,
                s.is_visible,
                COALESCE(COUNT(b.id) FILTER (WHERE b.deleted_at IS NULL), 0) AS content_block_count,
                s.created_at,
                s.updated_at
            FROM spa_sections s
            LEFT JOIN sections b ON b.spa_section_id = s.id
            WHERE s.page_id = $1 AND s.id = $2 AND s.deleted_at IS NULL
            GROUP BY s.id
            "#,
        )
        .bind(page_id)
        .bind(id)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn create_dynamic_for_page(
        &self,
        page_id: Uuid,
        base_key: &str,
        title: &str,
        nav_label: &str,
    ) -> AppResult<SpaSection> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 1. Lock home page row FOR UPDATE to serialize section creation on this page
        let page_exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM pages WHERE id = $1 FOR UPDATE")
                .bind(page_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if page_exists.is_none() {
            return Err(AppError::NotFound("Page not found".to_string()));
        }

        // 2. Query existing keys matching base_key or base_key-* (including soft-deleted ones)
        let existing_keys: Vec<(String,)> = sqlx::query_as(
            "SELECT section_key FROM spa_sections WHERE page_id = $1 AND (section_key = $2 OR section_key LIKE $3)",
        )
        .bind(page_id)
        .bind(base_key)
        .bind(format!("{}-%", base_key))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let key_set: HashSet<String> = existing_keys.into_iter().map(|r| r.0).collect();

        // Deterministic unique key allocation inside page lock
        let mut allocated_key = base_key.to_string();
        if key_set.contains(&allocated_key) {
            let mut counter = 2;
            loop {
                let candidate = format!("{}-{}", base_key, counter);
                if !key_set.contains(&candidate) {
                    allocated_key = candidate;
                    break;
                }
                counter += 1;
            }
        }

        // 3. Allocate next sort_order inside page lock
        let max_order: (i32,) = sqlx::query_as(
            "SELECT COALESCE(MAX(sort_order), 0) + 10 FROM spa_sections WHERE page_id = $1 AND deleted_at IS NULL",
        )
        .bind(page_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        let sort_order = max_order.0;

        // 4. Atomically INSERT the new SpaSection
        let id = Uuid::new_v4();
        let section = sqlx::query_as::<_, SpaSection>(
            r#"
            INSERT INTO spa_sections (id, page_id, section_key, title, navigation_label, sort_order, is_visible)
            VALUES ($1, $2, $3, $4, $5, $6, TRUE)
            RETURNING id, page_id, section_key, title, navigation_label, sort_order, is_visible, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(page_id)
        .bind(&allocated_key)
        .bind(title)
        .bind(nav_label)
        .bind(sort_order)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(section)
    }

    pub async fn update_section(
        &self,
        id: Uuid,
        title: Option<&str>,
        nav_label: Option<&str>,
        is_visible: Option<bool>,
    ) -> AppResult<Option<SpaSection>> {
        sqlx::query_as::<_, SpaSection>(
            r#"
            UPDATE spa_sections
            SET
                title = COALESCE($2, title),
                navigation_label = COALESCE($3, navigation_label),
                is_visible = COALESCE($4, is_visible),
                updated_at = CURRENT_TIMESTAMP
            WHERE id = $1 AND deleted_at IS NULL
            RETURNING id, page_id, section_key, title, navigation_label, sort_order, is_visible, created_at, updated_at
            "#
        )
        .bind(id)
        .bind(title)
        .bind(nav_label)
        .bind(is_visible)
        .fetch_optional(self.pool)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }

    pub async fn soft_delete_empty_for_page(&self, page_id: Uuid, id: Uuid) -> AppResult<bool> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 1. Lock target SpaSection row FOR UPDATE
        let section: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM spa_sections WHERE id = $1 AND page_id = $2 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(id)
        .bind(page_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if section.is_none() {
            return Ok(false);
        }

        // 2. Count active content blocks inside the transaction
        let block_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sections WHERE spa_section_id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        if block_count.0 > 0 {
            tx.rollback()
                .await
                .map_err(|e| AppError::DatabaseError(e.to_string()))?;
            return Err(AppError::ResourceConflict(
                "SpaSection cannot be deleted while it contains content blocks. Move or delete the blocks first."
                    .to_string(),
            ));
        }

        // 3. Atomically soft delete
        sqlx::query(
            "UPDATE spa_sections SET deleted_at = CURRENT_TIMESTAMP, is_visible = FALSE, updated_at = CURRENT_TIMESTAMP WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(true)
    }

    pub async fn reorder_sections_tx(
        &self,
        page_id: Uuid,
        items: &[ReorderSpaSectionItem],
    ) -> AppResult<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // 1. Fetch active IDs from DB for page_id
        let active_rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM spa_sections WHERE page_id = $1 AND deleted_at IS NULL ORDER BY sort_order ASC, id ASC",
        )
        .bind(page_id)
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
                    "Reorder items count ({}) does not match active sections count ({})",
                    items.len(),
                    db_ids_set.len()
                ),
            }]));
        }

        let mut req_ids_set = HashSet::new();
        let mut req_orders_set = HashSet::new();

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
                    message: format!("Referenced section ID {} is invalid or deleted", item.id),
                }]));
            }

            if !req_ids_set.insert(item.id) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!("Duplicate section ID in reorder request: {}", item.id),
                }]));
            }

            if !req_orders_set.insert(item.sort_order) {
                return Err(AppError::ValidationError(vec![ApiErrorDetails {
                    field: "items".to_string(),
                    message: format!(
                        "Duplicate sort_order in reorder request: {}",
                        item.sort_order
                    ),
                }]));
            }
        }

        // 3. Batch update sort_orders
        for item in items {
            sqlx::query(
                "UPDATE spa_sections SET sort_order = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2 AND page_id = $3",
            )
            .bind(item.sort_order)
            .bind(item.id)
            .bind(page_id)
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
