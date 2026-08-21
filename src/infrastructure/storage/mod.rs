use async_trait::async_trait;
use std::path::{Component, Path, PathBuf};
use tokio::fs;

use crate::shared::errors::{AppError, AppResult};

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn upload(&self, relative_path: &str, data: &[u8]) -> AppResult<String>;
    async fn move_file(&self, temp_file_path: &Path, relative_path: &str) -> AppResult<String>;
    async fn delete(&self, relative_path: &str) -> AppResult<()>;
    async fn exists(&self, relative_path: &str) -> bool;
    fn get_public_url(&self, relative_path: &str) -> String;
}

pub struct LocalStorageProvider {
    base_path: PathBuf,
    public_base_url: String,
}

impl LocalStorageProvider {
    pub fn new(base_path: impl Into<PathBuf>, public_base_url: String) -> Self {
        Self {
            base_path: base_path.into(),
            public_base_url,
        }
    }

    pub fn sanitize_path(&self, relative_path: &str) -> AppResult<PathBuf> {
        let path = Path::new(relative_path);
        if relative_path.trim().is_empty() {
            return Err(AppError::Forbidden);
        }

        for component in path.components() {
            if !matches!(component, Component::Normal(_)) {
                return Err(AppError::Forbidden);
            }
        }
        Ok(self.base_path.join(path))
    }
}

#[async_trait]
impl StorageProvider for LocalStorageProvider {
    async fn upload(&self, relative_path: &str, data: &[u8]) -> AppResult<String> {
        let full_path = self.sanitize_path(relative_path)?;
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create storage dir: {}", e)))?;
        }

        fs::write(&full_path, data)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write storage file: {}", e)))?;

        Ok(relative_path.to_string())
    }

    async fn move_file(&self, temp_file_path: &Path, relative_path: &str) -> AppResult<String> {
        let full_path = self.sanitize_path(relative_path)?;
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create storage dir: {}", e)))?;
        }

        if fs::rename(temp_file_path, &full_path).await.is_err() {
            fs::copy(temp_file_path, &full_path)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to copy temp file: {}", e)))?;
            let _ = fs::remove_file(temp_file_path).await;
        }

        Ok(relative_path.to_string())
    }

    async fn delete(&self, relative_path: &str) -> AppResult<()> {
        let full_path = self.sanitize_path(relative_path)?;
        if full_path.exists() {
            let _ = fs::remove_file(&full_path).await;
        }
        Ok(())
    }

    async fn exists(&self, relative_path: &str) -> bool {
        if let Ok(full_path) = self.sanitize_path(relative_path) {
            full_path.exists()
        } else {
            false
        }
    }

    fn get_public_url(&self, relative_path: &str) -> String {
        format!(
            "{}/{}",
            self.public_base_url.trim_end_matches('/'),
            relative_path.trim_start_matches('/')
        )
    }
}
