//! Xray configuration generation
//!
//! This module handles generating Xray configuration files from the database models.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrayConfig {
    pub log: LogConfig,
    pub stats: serde_json::Value,
    pub api: ApiConfig,
    pub policy: PolicyConfig,
    pub inbounds: Vec<InboundConfig>,
    pub outbounds: Vec<OutboundConfig>,
    pub routing: RoutingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub loglevel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub tag: String,
    pub services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub system: SystemPolicy,
    pub levels: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPolicy {
    pub stats_inbound_uplink: bool,
    pub stats_inbound_downlink: bool,
    pub stats_outbound_uplink: bool,
    pub stats_outbound_downlink: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundConfig {
    pub tag: String,
    pub listen: String,
    pub port: i32,
    pub protocol: String,
    pub settings: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_settings: Option<serde_json::Value>,
    pub sniffing: SniffingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SniffingConfig {
    pub enabled: bool,
    pub dest_override: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains_excluded: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundConfig {
    pub tag: String,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub domain_strategy: String,
    pub rules: Vec<RoutingRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outbound_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inbound_tag: Option<Vec<String>>,
}

impl Default for XrayConfig {
    fn default() -> Self {
        Self {
            log: LogConfig {
                loglevel: "warning".to_string(),
                access: None,
                error: None,
            },
            stats: serde_json::json!({}),
            api: ApiConfig {
                tag: "api".to_string(),
                services: vec![
                    "StatsService".to_string(),
                    "HandlerService".to_string(),
                    "LoggerService".to_string(),
                ],
            },
            policy: PolicyConfig {
                system: SystemPolicy {
                    stats_inbound_uplink: true,
                    stats_inbound_downlink: true,
                    stats_outbound_uplink: true,
                    stats_outbound_downlink: true,
                },
                levels: serde_json::json!({
                    "0": {
                        "handshake": 4,
                        "connIdle": 300,
                        "uplinkOnly": 2,
                        "downlinkOnly": 5,
                        "statsUserUplink": false,
                        "statsUserDownlink": false,
                        "bufferSize": 10240
                    }
                }),
            },
            inbounds: vec![],
            outbounds: vec![
                OutboundConfig {
                    tag: "direct".to_string(),
                    protocol: "freedom".to_string(),
                    settings: None,
                },
                OutboundConfig {
                    tag: "blocked".to_string(),
                    protocol: "blackhole".to_string(),
                    settings: Some(serde_json::json!({})),
                },
            ],
            routing: RoutingConfig {
                domain_strategy: "AsIs".to_string(),
                rules: vec![
                    RoutingRule {
                        rule_type: "field".to_string(),
                        ip: Some(vec!["geoip:private".to_string()]),
                        domain: None,
                        outbound_tag: Some("blocked".to_string()),
                        inbound_tag: None,
                    },
                ],
            },
        }
    }
}

impl XrayConfig {
    /// Convert to JSON string
    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Add an inbound to the configuration
    pub fn add_inbound(&mut self, inbound: InboundConfig) {
        self.inbounds.push(inbound);
    }

    /// Remove an inbound by tag
    pub fn remove_inbound(&mut self, tag: &str) {
        self.inbounds.retain(|i| i.tag != tag);
    }

    /// Add a routing rule
    pub fn add_rule(&mut self, rule: RoutingRule) {
        self.routing.rules.push(rule);
    }
}

/// Protocol-specific settings generators
pub struct ProtocolSettings;

impl ProtocolSettings {
    /// Generate VMess settings
    pub fn vmess_clients(clients: Vec<VmessClient>) -> serde_json::Value {
        serde_json::json!({
            "clients": clients
        })
    }

    /// Generate VLESS settings
    pub fn vless_clients(clients: Vec<VlessClient>, decryption: &str) -> serde_json::Value {
        serde_json::json!({
            "clients": clients,
            "decryption": decryption
        })
    }

    /// Generate Trojan settings
    pub fn trojan_clients(clients: Vec<TrojanClient>) -> serde_json::Value {
        serde_json::json!({
            "clients": clients
        })
    }

    /// Generate Shadowsocks settings
    pub fn shadowsocks(method: &str, password: &str, network: &str) -> serde_json::Value {
        serde_json::json!({
            "method": method,
            "password": password,
            "network": network
        })
    }

    /// Generate SOCKS settings
    pub fn socks(auth: &str, accounts: Vec<SocksAccount>, udp: bool) -> serde_json::Value {
        serde_json::json!({
            "auth": auth,
            "accounts": accounts,
            "udp": udp
        })
    }

    /// Generate HTTP settings
    pub fn http(accounts: Vec<HttpAccount>) -> serde_json::Value {
        serde_json::json!({
            "accounts": accounts
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmessClient {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alter_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlessClient {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrojanClient {
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocksAccount {
    pub user: String,
    pub pass: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpAccount {
    pub user: String,
    pub pass: String,
}

/// Stream settings generators
pub struct StreamSettings;

impl StreamSettings {
    /// TCP stream settings
    pub fn tcp(header_type: Option<&str>, host: Option<&str>, path: Option<&str>) -> serde_json::Value {
        let header = match (header_type, host, path) {
            (Some("http"), Some(h), Some(p)) => serde_json::json!({
                "type": "http",
                "request": {
                    "headers": {
                        "Host": [h]
                    },
                    "path": p
                }
            }),
            (Some("http"), Some(h), None) => serde_json::json!({
                "type": "http",
                "request": {
                    "headers": {
                        "Host": [h]
                    }
                }
            }),
            _ => serde_json::json!({ "type": "none" }),
        };

        serde_json::json!({
            "network": "tcp",
            "tcpSettings": {
                "header": header
            }
        })
    }

    /// WebSocket stream settings
    pub fn websocket(path: &str, host: Option<&str>) -> serde_json::Value {
        let mut headers = serde_json::Map::new();
        if let Some(h) = host {
            headers.insert("Host".to_string(), serde_json::json!([h]));
        }

        serde_json::json!({
            "network": "ws",
            "wsSettings": {
                "path": path,
                "headers": headers
            }
        })
    }

    /// HTTP/2 stream settings
    pub fn http2(path: &str, host: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "network": "http",
            "httpSettings": {
                "path": path,
                "host": host
            }
        })
    }

    /// gRPC stream settings
    pub fn grpc(service_name: &str, multi_mode: bool) -> serde_json::Value {
        serde_json::json!({
            "network": "grpc",
            "grpcSettings": {
                "serviceName": service_name,
                "multiMode": multi_mode
            }
        })
    }

    /// TLS security settings
    pub fn tls(server_name: &str, certificates: Vec<TlsCertificate>) -> serde_json::Value {
        serde_json::json!({
            "security": "tls",
            "tlsSettings": {
                "serverName": server_name,
                "certificates": certificates,
                "rejectUnknownSni": true
            }
        })
    }

    /// Reality security settings
    pub fn reality(
        show: bool,
        dest: &str,
        server_names: &[&str],
        private_key: &str,
        short_ids: &[&str],
    ) -> serde_json::Value {
        serde_json::json!({
            "security": "reality",
            "realitySettings": {
                "show": show,
                "dest": dest,
                "serverNames": server_names,
                "privateKey": private_key,
                "shortIds": short_ids
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsCertificate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}
