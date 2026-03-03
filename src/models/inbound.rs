use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Inbound {
    pub id: i64,
    pub user_id: i64,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    #[sqlx(default)]
    pub all_time: i64,
    pub remark: Option<String>,
    pub enable: bool,
    #[sqlx(default)]
    pub expiry_time: i64,
    #[sqlx(default)]
    pub traffic_reset: String,
    #[sqlx(default)]
    pub last_traffic_reset_time: i64,
    pub listen: Option<String>,
    pub port: i32,
    pub protocol: String,
    pub settings: Option<String>,
    pub stream_settings: Option<String>,
    pub tag: String,
    pub sniffing: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInboundRequest {
    pub remark: Option<String>,
    pub listen: Option<String>,
    pub port: i32,
    pub protocol: String,
    pub settings: Option<String>,
    pub stream_settings: Option<String>,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub enable: Option<bool>,
    pub sniffing: Option<String>,
    pub traffic_reset: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateInboundRequest {
    pub id: i64,
    pub remark: Option<String>,
    pub listen: Option<String>,
    pub port: Option<i32>,
    pub protocol: Option<String>,
    pub settings: Option<String>,
    pub stream_settings: Option<String>,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub enable: Option<bool>,
    pub sniffing: Option<String>,
    pub traffic_reset: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InboundStats {
    pub id: i64,
    pub tag: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
}

/// Protocol types supported by Xray
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Vmess,
    Vless,
    Trojan,
    Shadowsocks,
    Http,
    Socks,
    Mixed,
    WireGuard,
    DokodemoDoor,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Vmess => "vmess",
            Protocol::Vless => "vless",
            Protocol::Trojan => "trojan",
            Protocol::Shadowsocks => "shadowsocks",
            Protocol::Http => "http",
            Protocol::Socks => "socks",
            Protocol::Mixed => "mixed",
            Protocol::WireGuard => "wireguard",
            Protocol::DokodemoDoor => "dokodemo-door",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vmess" => Some(Protocol::Vmess),
            "vless" => Some(Protocol::Vless),
            "trojan" => Some(Protocol::Trojan),
            "shadowsocks" => Some(Protocol::Shadowsocks),
            "http" => Some(Protocol::Http),
            "socks" => Some(Protocol::Socks),
            "mixed" => Some(Protocol::Mixed),
            "wireguard" => Some(Protocol::WireGuard),
            "dokodemo-door" => Some(Protocol::DokodemoDoor),
            _ => None,
        }
    }
}
