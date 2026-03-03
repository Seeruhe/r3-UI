//! Xray API client
//!
//! This module provides communication with Xray's internal API for stats and management.

use serde::{Deserialize, Serialize};

/// Xray stats API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub stat: Vec<StatValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatValue {
    pub name: String,
    pub value: i64,
}

/// Parse stats name to extract tag and direction
pub fn parse_stats_name(name: &str) -> Option<(String, String, String)> {
    // Format: inbound>>>tag>>>traffic>>>uplink or downlink
    let parts: Vec<&str> = name.split(">>>").collect();
    if parts.len() >= 4 {
        Some((
            parts[0].to_string(),      // inbound/outbound
            parts[1].to_string(),      // tag
            parts[3].to_string(),      // uplink/downlink
        ))
    } else {
        None
    }
}

/// Build stats query name
pub fn build_inbound_uplink_name(tag: &str) -> String {
    format!("inbound>>>{}>>>traffic>>>uplink", tag)
}

pub fn build_inbound_downlink_name(tag: &str) -> String {
    format!("inbound>>>{}>>>traffic>>>downlink", tag)
}
