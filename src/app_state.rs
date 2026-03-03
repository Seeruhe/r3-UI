use std::sync::Arc;
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::config::Settings;
use crate::services::xray::XrayManager;
use crate::services::system::SystemMonitor;
use crate::bot::{TelegramBot, NotificationService};
use crate::bot::backup::BackupService;
use crate::websocket::hub::WsHub;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub settings: Arc<Settings>,
    pub xray: Arc<XrayManager>,
    pub system_monitor: Arc<SystemMonitor>,
    pub ws_hub: Arc<WsHub>,
    pub xray_process: Arc<RwLock<XrayProcessState>>,
    pub telegram_bot: Arc<RwLock<Option<TelegramBot>>>,
    pub notification_service: Arc<NotificationService>,
    pub backup_service: Arc<RwLock<Option<BackupService>>>,
}

/// Xray process state tracking
#[derive(Clone, Default)]
pub struct XrayProcessState {
    pub is_running: bool,
    pub pid: Option<u32>,
    pub started_at: Option<i64>,
    pub restart_count: u32,
}

impl XrayProcessState {
    pub fn is_running(&self) -> bool {
        self.is_running
    }
}
