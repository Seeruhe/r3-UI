use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Traffic {
    pub id: i64,
    pub inbound_id: i64,
    pub up: i64,
    pub down: i64,
}

#[derive(Debug, Serialize)]
pub struct TrafficStats {
    pub inbound_id: i64,
    pub tag: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
pub struct TrafficUpdate {
    pub tag: String,
    pub up: i64,
    pub down: i64,
}
