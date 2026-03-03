//! Database backup service for Telegram bot

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

use super::TelegramBot;

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub backup_path: PathBuf,
    pub cron_schedule: String,
    pub keep_count: usize,
    pub send_to_telegram: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backup_path: PathBuf::from("./backup"),
            cron_schedule: "0 3 * * *".to_string(), // 3 AM daily
            keep_count: 7,
            send_to_telegram: false,
        }
    }
}

/// Backup file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub filename: String,
    pub path: PathBuf,
    pub size: u64,
    pub created_at: DateTime<Utc>,
}

/// Backup service
pub struct BackupService {
    config: Arc<RwLock<BackupConfig>>,
    bot: Arc<RwLock<Option<TelegramBot>>>,
    database_path: PathBuf,
}

impl BackupService {
    /// Create a new backup service
    pub fn new(config: BackupConfig, database_path: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            bot: Arc::new(RwLock::new(None)),
            database_path,
        }
    }

    /// Set the Telegram bot for notifications
    pub async fn set_bot(&self, bot: TelegramBot) {
        let mut current = self.bot.write().await;
        *current = Some(bot);
    }

    /// Update backup configuration
    pub async fn update_config(&self, config: BackupConfig) {
        let mut current = self.config.write().await;
        *current = config;
    }

    /// Get current configuration
    pub async fn get_config(&self) -> BackupConfig {
        self.config.read().await.clone()
    }

    /// Ensure backup directory exists
    async fn ensure_backup_dir(&self) -> Result<PathBuf> {
        let config = self.config.read().await;
        let backup_path = &config.backup_path;

        if !backup_path.exists() {
            fs::create_dir_all(backup_path).await?;
        }

        Ok(backup_path.clone())
    }

    /// Create a database backup
    pub async fn create_backup(&self) -> Result<BackupInfo> {
        let config = self.config.read().await.clone();

        // Ensure backup directory exists
        let backup_dir = self.ensure_backup_dir().await?;

        // Generate backup filename
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("backup_{}.db", timestamp);
        let backup_path = backup_dir.join(&filename);

        // Copy database file
        if !self.database_path.exists() {
            return Err(anyhow!("Database file not found: {:?}", self.database_path));
        }

        fs::copy(&self.database_path, &backup_path).await?;

        // Get file size
        let metadata = fs::metadata(&backup_path).await?;
        let size = metadata.len();

        let backup_info = BackupInfo {
            filename,
            path: backup_path,
            size,
            created_at: Utc::now(),
        };

        // Clean up old backups
        self.cleanup_old_backups().await?;

        // Send to Telegram if configured
        if config.send_to_telegram {
            if let Some(bot) = self.bot.write().await.as_ref() {
                let _ = bot.send_message(&format!(
                    "💾 Backup created: {}\nSize: {} bytes",
                    backup_info.filename,
                    backup_info.size
                )).await;
            }
        }

        tracing::info!("Backup created: {:?}", backup_info.path);

        Ok(backup_info)
    }

    /// List all backups
    pub async fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let backup_dir = self.ensure_backup_dir().await?;

        let mut backups = Vec::new();
        let mut entries = fs::read_dir(&backup_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().map(|e| e == "db").unwrap_or(false) {
                let metadata = entry.metadata().await?;
                let created = metadata.created().ok();
                let modified = metadata.modified().ok();

                let created_at = match (created, modified) {
                    (Some(t), _) => t.into(),
                    (_, Some(t)) => t.into(),
                    _ => Utc::now(),
                };

                backups.push(BackupInfo {
                    filename: path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    path,
                    size: metadata.len(),
                    created_at,
                });
            }
        }

        // Sort by creation time (newest first)
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Delete old backups based on keep_count
    pub async fn cleanup_old_backups(&self) -> Result<Vec<PathBuf>> {
        let config = self.config.read().await.clone();
        let backups = self.list_backups().await?;

        let mut deleted = Vec::new();

        // Delete backups exceeding keep_count
        for backup in backups.into_iter().skip(config.keep_count) {
            if fs::remove_file(&backup.path).await.is_ok() {
                deleted.push(backup.path);
            }
        }

        if !deleted.is_empty() {
            tracing::info!("Cleaned up {} old backup(s)", deleted.len());
        }

        Ok(deleted)
    }

    /// Restore database from a backup
    pub async fn restore_backup(&self, backup_path: &Path) -> Result<()> {
        if !backup_path.exists() {
            return Err(anyhow!("Backup file not found: {:?}", backup_path));
        }

        // Create a backup of current database first
        let current_backup = format!(
            "{}.pre_restore.{}",
            self.database_path.display(),
            Utc::now().format("%Y%m%d_%H%M%S")
        );

        if self.database_path.exists() {
            fs::copy(&self.database_path, &current_backup).await?;
            tracing::info!("Created pre-restore backup: {}", current_backup);
        }

        // Copy backup to database location
        fs::copy(backup_path, &self.database_path).await?;

        tracing::info!("Database restored from: {:?}", backup_path);

        Ok(())
    }

    /// Delete a specific backup
    pub async fn delete_backup(&self, filename: &str) -> Result<()> {
        let config = self.config.read().await;
        let backup_path = config.backup_path.join(filename);

        if !backup_path.exists() {
            return Err(anyhow!("Backup file not found: {}", filename));
        }

        fs::remove_file(&backup_path).await?;

        tracing::info!("Backup deleted: {}", filename);

        Ok(())
    }

    /// Get backup file path for downloading
    pub async fn get_backup_path(&self, filename: &str) -> Result<PathBuf> {
        let config = self.config.read().await;
        let path = config.backup_path.join(filename);

        if !path.exists() {
            return Err(anyhow!("Backup file not found"));
        }

        Ok(path)
    }

    /// Import database from file
    pub async fn import_database(&self, source_path: &Path) -> Result<()> {
        if !source_path.exists() {
            return Err(anyhow!("Source file not found: {:?}", source_path));
        }

        // Create backup of current database
        let current_backup = format!(
            "{}.pre_import.{}",
            self.database_path.display(),
            Utc::now().format("%Y%m%d_%H%M%S")
        );

        if self.database_path.exists() {
            fs::copy(&self.database_path, &current_backup).await?;
            tracing::info!("Created pre-import backup: {}", current_backup);
        }

        // Copy source to database location
        fs::copy(source_path, &self.database_path).await?;

        tracing::info!("Database imported from: {:?}", source_path);

        Ok(())
    }

    /// Send backup to Telegram
    pub async fn send_backup_to_telegram(&self, backup_path: &Path) -> Result<()> {
        let bot = self.bot.read().await;

        if bot.is_some() {
            // Note: File sending would require additional implementation
            // with teloxide's send_document method
            tracing::info!("Would send backup to Telegram: {:?}", backup_path);
        }

        Ok(())
    }
}

/// Format file size to human readable string
fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }

    #[test]
    fn test_backup_config_default() {
        let config = BackupConfig::default();
        assert!(config.enabled);
        assert_eq!(config.keep_count, 7);
        assert_eq!(config.cron_schedule, "0 3 * * *");
    }
}
