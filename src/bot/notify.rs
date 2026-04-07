//! Telegram notification service

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::TelegramBot;

/// Notification types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    TrafficLimit,
    ExpiryWarning,
    Expired,
    Login,
    SystemAlert,
    BackupComplete,
    XrayRestart,
    XrayStopped,
    XrayError,
}

/// Notification message
#[derive(Debug, Clone)]
pub struct Notification {
    pub notification_type: NotificationType,
    pub title: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

/// Notification service for sending alerts via Telegram
pub struct NotificationService {
    bot: Arc<RwLock<Option<TelegramBot>>>,
    enabled: Arc<RwLock<bool>>,
}

impl NotificationService {
    /// Create a new notification service
    pub fn new() -> Self {
        Self {
            bot: Arc::new(RwLock::new(None)),
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Set the bot instance
    pub async fn set_bot(&self, bot: TelegramBot) {
        let mut current = self.bot.write().await;
        *current = Some(bot);
    }

    /// Enable or disable notifications
    pub async fn set_enabled(&self, enabled: bool) {
        let mut current = self.enabled.write().await;
        *current = enabled;
    }

    /// Check if notifications are enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }

    /// Send a notification
    pub async fn send(&self, notification: Notification) -> Result<()> {
        if !self.is_enabled().await {
            return Ok(());
        }

        let bot = self.bot.read().await;
        if let Some(bot) = bot.as_ref() {
            let emoji = notification_type_emoji(&notification.notification_type);
            let text = format!(
                "{} *{}*\n\n{}",
                emoji,
                notification.title,
                notification.message
            );

            bot.send_message(&text).await?;
        }

        Ok(())
    }

    /// Send traffic limit notification
    pub async fn notify_traffic_limit(
        &self,
        client_email: &str,
        used: i64,
        total: i64,
        inbound_tag: &str,
    ) -> Result<()> {
        let percentage = if total > 0 {
            (used as f64 / total as f64 * 100.0) as i32
        } else {
            0
        };
        let notification = Notification {
            notification_type: NotificationType::TrafficLimit,
            title: "Traffic Limit Alert".to_string(),
            message: format!(
                "Client *{}* has reached {}% of traffic limit.\n\n\
                 Used: *{}* / *{}*\n\
                 Inbound: *{}*",
                client_email,
                percentage,
                format_bytes(used),
                format_bytes(total),
                inbound_tag
            ),
            details: Some(serde_json::json!({
                "client_email": client_email,
                "used": used,
                "total": total,
                "percentage": percentage,
                "inbound_tag": inbound_tag
            })),
        };

        self.send(notification).await
    }

    /// Send expiry warning notification
    pub async fn notify_expiry_warning(
        &self,
        client_email: &str,
        days_remaining: i32,
        expiry_date: &str,
        inbound_tag: &str,
    ) -> Result<()> {
        let notification = Notification {
            notification_type: NotificationType::ExpiryWarning,
            title: "Expiry Warning".to_string(),
            message: format!(
                "Client *{}* will expire in *{}* day(s).\n\n\
                 Expiry Date: *{}*\n\
                 Inbound: *{}*",
                client_email,
                days_remaining,
                expiry_date,
                inbound_tag
            ),
            details: Some(serde_json::json!({
                "client_email": client_email,
                "days_remaining": days_remaining,
                "expiry_date": expiry_date,
                "inbound_tag": inbound_tag
            })),
        };

        self.send(notification).await
    }

    /// Send client expired notification
    pub async fn notify_expired(
        &self,
        client_email: &str,
        inbound_tag: &str,
    ) -> Result<()> {
        let notification = Notification {
            notification_type: NotificationType::Expired,
            title: "Client Expired".to_string(),
            message: format!(
                "Client *{}* has expired.\n\n\
                 Inbound: *{}*",
                client_email,
                inbound_tag
            ),
            details: Some(serde_json::json!({
                "client_email": client_email,
                "inbound_tag": inbound_tag
            })),
        };

        self.send(notification).await
    }

    /// Send login notification
    pub async fn notify_login(
        &self,
        username: &str,
        ip_address: &str,
        user_agent: &str,
    ) -> Result<()> {
        let notification = Notification {
            notification_type: NotificationType::Login,
            title: "New Login".to_string(),
            message: format!(
                "User *{}* logged in.\n\n\
                 IP Address: *{}*\n\
                 User Agent: `{}`",
                username,
                ip_address,
                user_agent
            ),
            details: Some(serde_json::json!({
                "username": username,
                "ip_address": ip_address,
                "user_agent": user_agent
            })),
        };

        self.send(notification).await
    }

    /// Send system alert notification
    pub async fn notify_system_alert(
        &self,
        alert_type: &str,
        message: &str,
    ) -> Result<()> {
        let notification = Notification {
            notification_type: NotificationType::SystemAlert,
            title: format!("System Alert: {}", alert_type),
            message: message.to_string(),
            details: Some(serde_json::json!({
                "alert_type": alert_type
            })),
        };

        self.send(notification).await
    }

    /// Send backup complete notification
    pub async fn notify_backup_complete(
        &self,
        filename: &str,
        size: i64,
    ) -> Result<()> {
        let notification = Notification {
            notification_type: NotificationType::BackupComplete,
            title: "Backup Complete".to_string(),
            message: format!(
                "Database backup created successfully.\n\n\
                 File: *{}*\n\
                 Size: *{}*",
                filename,
                format_bytes(size)
            ),
            details: Some(serde_json::json!({
                "filename": filename,
                "size": size
            })),
        };

        self.send(notification).await
    }

    /// Send Xray restart notification
    pub async fn notify_xray_restart(&self) -> Result<()> {
        let notification = Notification {
            notification_type: NotificationType::XrayRestart,
            title: "Xray Restarted".to_string(),
            message: "Xray service has been restarted.".to_string(),
            details: None,
        };

        self.send(notification).await
    }

    /// Send Xray stopped notification
    pub async fn notify_xray_stopped(&self, reason: Option<&str>) -> Result<()> {
        let message = if let Some(reason) = reason {
            format!("Xray service has been stopped.\n\nReason: {}", reason)
        } else {
            "Xray service has been stopped.".to_string()
        };

        let notification = Notification {
            notification_type: NotificationType::XrayStopped,
            title: "Xray Stopped".to_string(),
            message,
            details: None,
        };

        self.send(notification).await
    }

    /// Send Xray error notification
    pub async fn notify_xray_error(&self, error: &str) -> Result<()> {
        let notification = Notification {
            notification_type: NotificationType::XrayError,
            title: "Xray Error".to_string(),
            message: format!(
                "Xray encountered an error:\n\n```\n{}\n```",
                error
            ),
            details: Some(serde_json::json!({
                "error": error
            })),
        };

        self.send(notification).await
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Get emoji for notification type
fn notification_type_emoji(notification_type: &NotificationType) -> &'static str {
    match notification_type {
        NotificationType::TrafficLimit => "⚠️",
        NotificationType::ExpiryWarning => "⏰",
        NotificationType::Expired => "❌",
        NotificationType::Login => "🔐",
        NotificationType::SystemAlert => "🚨",
        NotificationType::BackupComplete => "💾",
        NotificationType::XrayRestart => "🔄",
        NotificationType::XrayStopped => "⏹️",
        NotificationType::XrayError => "🔥",
    }
}

/// Format bytes to human readable string
fn format_bytes(bytes: i64) -> String {
    if bytes < 0 {
        return "0 B".to_string();
    }

    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    const TB: i64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
        assert_eq!(format_bytes(1099511627776), "1.00 TB");
    }

    #[test]
    fn test_notification_type_emoji() {
        assert_eq!(notification_type_emoji(&NotificationType::TrafficLimit), "⚠️");
        assert_eq!(notification_type_emoji(&NotificationType::BackupComplete), "💾");
        assert_eq!(notification_type_emoji(&NotificationType::XrayError), "🔥");
    }
}
