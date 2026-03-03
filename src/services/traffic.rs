use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficData {
    pub tag: String,
    pub up: i64,
    pub down: i64,
}

pub struct TrafficCollector {
    traffic: Arc<RwLock<HashMap<String, TrafficData>>>,
}

impl TrafficCollector {
    pub fn new() -> Self {
        Self {
            traffic: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update(&self, tag: &str, up: i64, down: i64) {
        let mut traffic = self.traffic.write().await;
        let entry = traffic.entry(tag.to_string()).or_insert(TrafficData {
            tag: tag.to_string(),
            up: 0,
            down: 0,
        });
        entry.up = up;
        entry.down = down;
    }

    pub async fn increment(&self, tag: &str, up_delta: i64, down_delta: i64) {
        let mut traffic = self.traffic.write().await;
        let entry = traffic.entry(tag.to_string()).or_insert(TrafficData {
            tag: tag.to_string(),
            up: 0,
            down: 0,
        });
        entry.up += up_delta;
        entry.down += down_delta;
    }

    pub async fn get(&self, tag: &str) -> Option<TrafficData> {
        let traffic = self.traffic.read().await;
        traffic.get(tag).cloned()
    }

    pub async fn get_all(&self) -> HashMap<String, TrafficData> {
        self.traffic.read().await.clone()
    }

    pub async fn reset(&self, tag: &str) {
        let mut traffic = self.traffic.write().await;
        if let Some(entry) = traffic.get_mut(tag) {
            entry.up = 0;
            entry.down = 0;
        }
    }

    pub async fn reset_all(&self) {
        let mut traffic = self.traffic.write().await;
        for entry in traffic.values_mut() {
            entry.up = 0;
            entry.down = 0;
        }
    }
}

impl Default for TrafficCollector {
    fn default() -> Self {
        Self::new()
    }
}
