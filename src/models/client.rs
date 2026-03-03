use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ClientTraffic {
    pub id: i64,
    pub inbound_id: i64,
    pub email: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    pub expiry_time: i64,
    pub enable: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateClientRequest {
    pub inbound_id: i64,
    pub email: String,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub enable: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ClientStats {
    pub email: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    pub usage: i64,
    pub enable: bool,
    pub expiry_time: i64,
}
