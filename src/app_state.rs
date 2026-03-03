use std::sync::Arc;
use sqlx::SqlitePool;

use crate::config::Settings;
use crate::services::xray::XrayManager;
use crate::websocket::hub::WsHub;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub settings: Arc<Settings>,
    pub xray: Arc<XrayManager>,
    pub ws_hub: Arc<WsHub>,
}
