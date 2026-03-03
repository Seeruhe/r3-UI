//! Traffic statistics collection for Xray
//!
//! This module handles collecting and aggregating traffic statistics.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficEntry {
    pub tag: String,
    pub up: i64,
    pub down: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSnapshot {
    pub timestamp: i64,
    pub entries: Vec<TrafficEntry>,
}

pub struct TrafficStore {
    entries: Arc<RwLock<HashMap<String, TrafficEntry>>>,
}

impl TrafficStore {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update(&self, tag: &str, up: i64, down: i64) {
        let mut entries = self.entries.write().await;
        entries.insert(
            tag.to_string(),
            TrafficEntry {
                tag: tag.to_string(),
                up,
                down,
            },
        );
    }

    pub async fn get(&self, tag: &str) -> Option<TrafficEntry> {
        let entries = self.entries.read().await;
        entries.get(tag).cloned()
    }

    pub async fn get_all(&self) -> Vec<TrafficEntry> {
        let entries = self.entries.read().await;
        entries.values().cloned().collect()
    }

    pub async fn snapshot(&self) -> TrafficSnapshot {
        TrafficSnapshot {
            timestamp: chrono::Utc::now().timestamp(),
            entries: self.get_all().await,
        }
    }

    pub async fn reset(&self, tag: &str) {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(tag) {
            entry.up = 0;
            entry.down = 0;
        }
    }
}

impl Default for TrafficStore {
    fn default() -> Self {
        Self::new()
    }
}
