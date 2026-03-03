use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Client traffic tracking model - tracks individual client usage per inbound
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClientTraffic {
    pub id: i64,
    pub inbound_id: i64,
    pub email: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    #[sqlx(default)]
    pub expiry_time: i64,
    #[sqlx(default)]
    pub enable: bool,
    /// IP limit for this client (0 = unlimited)
    #[sqlx(default)]
    pub limit_ip: i32,
    /// Telegram user ID for notifications
    #[sqlx(default)]
    pub tg_id: i64,
    /// Subscription ID for generating subscription links
    #[sqlx(default)]
    pub sub_id: String,
    /// Optional comment for this client
    #[sqlx(default)]
    pub comment: String,
    /// Traffic reset strategy (e.g., "0" = never, "1" = daily, "2" = weekly, "3" = monthly)
    #[sqlx(default)]
    pub reset: i32,
    /// Creation timestamp
    #[sqlx(default)]
    pub created_at: i64,
    /// Last update timestamp
    #[sqlx(default)]
    pub updated_at: i64,
}

/// Request to create a new client
#[derive(Debug, Deserialize)]
pub struct CreateClientRequest {
    pub inbound_id: i64,
    pub email: String,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub enable: Option<bool>,
    pub limit_ip: Option<i32>,
    pub tg_id: Option<i64>,
    pub sub_id: Option<String>,
    pub comment: Option<String>,
    pub reset: Option<i32>,
}

/// Request to update an existing client
#[derive(Debug, Deserialize)]
pub struct UpdateClientRequest {
    pub id: i64,
    pub email: Option<String>,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub enable: Option<bool>,
    pub limit_ip: Option<i32>,
    pub tg_id: Option<i64>,
    pub sub_id: Option<String>,
    pub comment: Option<String>,
    pub reset: Option<i32>,
}

/// Client statistics for API responses
#[derive(Debug, Serialize)]
pub struct ClientStats {
    pub id: i64,
    pub inbound_id: i64,
    pub email: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    pub usage: i64,
    pub enable: bool,
    pub expiry_time: i64,
    pub limit_ip: i32,
    pub tg_id: i64,
    pub sub_id: String,
    pub comment: String,
    pub reset: i32,
}

/// Client for import functionality
#[derive(Debug, Deserialize)]
pub struct ClientImport {
    pub email: String,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub enable: Option<bool>,
    pub limit_ip: Option<i32>,
    pub tg_id: Option<i64>,
    pub sub_id: Option<String>,
    pub comment: Option<String>,
    pub reset: Option<i32>,
}

/// Request to import multiple clients
#[derive(Debug, Deserialize)]
pub struct ImportClientsRequest {
    pub inbound_id: i64,
    pub clients: Vec<ClientImport>,
}

/// Client traffic history record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClientTrafficHistory {
    pub id: i64,
    pub client_id: i64,
    pub up: i64,
    pub down: i64,
    pub recorded_at: i64,
}

impl ClientTraffic {
    /// Check if the client has exceeded their traffic limit
    pub fn is_depleted(&self) -> bool {
        if self.total <= 0 {
            return false; // Unlimited
        }
        (self.up + self.down) >= self.total
    }

    /// Check if the client is expired
    pub fn is_expired(&self) -> bool {
        if self.expiry_time <= 0 {
            return false; // Never expires
        }
        let now = chrono::Utc::now().timestamp();
        now >= self.expiry_time
    }

    /// Check if the client is active (enabled, not depleted, not expired)
    pub fn is_active(&self) -> bool {
        self.enable && !self.is_depleted() && !self.is_expired()
    }

    /// Get remaining traffic
    pub fn remaining_traffic(&self) -> i64 {
        if self.total <= 0 {
            return -1; // Unlimited
        }
        let used = self.up + self.down;
        if used >= self.total {
            return 0;
        }
        self.total - used
    }

    /// Get remaining time in seconds
    pub fn remaining_time(&self) -> i64 {
        if self.expiry_time <= 0 {
            return -1; // Never expires
        }
        let now = chrono::Utc::now().timestamp();
        if now >= self.expiry_time {
            return 0;
        }
        self.expiry_time - now
    }
}

impl From<ClientTraffic> for ClientStats {
    fn from(client: ClientTraffic) -> Self {
        Self {
            id: client.id,
            inbound_id: client.inbound_id,
            email: client.email,
            up: client.up,
            down: client.down,
            total: client.total,
            usage: client.up + client.down,
            enable: client.enable,
            expiry_time: client.expiry_time,
            limit_ip: client.limit_ip,
            tg_id: client.tg_id,
            sub_id: client.sub_id,
            comment: client.comment,
            reset: client.reset,
        }
    }
}
