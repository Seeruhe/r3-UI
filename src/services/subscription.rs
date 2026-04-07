//! Subscription service for generating client configurations in multiple formats

use serde::{Deserialize, Serialize};
use base64::Engine;

/// Subscription information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    pub url: String,
    pub token: String,
    pub enabled: bool,
}

/// Client configuration for subscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub email: String,
    pub protocol: String,
    pub address: String,
    pub port: i32,
    pub uuid: String,
    pub flow: Option<String>,
    pub encryption: String,
    pub transport_type: String,
    pub transport_settings: Option<TransportSettings>,
    pub security: String,
    pub security_settings: Option<SecuritySettings>,
    pub remark: String,
    pub upload: i64,
    pub download: i64,
    pub total: i64,
    pub expiry_time: i64,
}

/// Transport settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportSettings {
    pub path: Option<String>,
    pub host: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub grpc_service_name: Option<String>,
}

/// Security settings (TLS/Reality)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub server_name: Option<String>,
    pub allow_insecure: bool,
    pub fingerprint: Option<String>,
    pub public_key: Option<String>,
    pub short_id: Option<String>,
    pub spider_x: Option<String>,
}

/// Subscription format types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SubscriptionFormat {
    Base64,      // Standard base64 encoded links
    Clash,       // Clash YAML format
    SingBox,     // sing-box JSON format
    Surge,       // Surge format
    Quantumult,  // Quantumult format
    Surfboard,   // Surfboard format
}

impl SubscriptionFormat {
    /// Parse format from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "b64" | "base64" | "default" => Some(Self::Base64),
            "clash" | "yaml" => Some(Self::Clash),
            "singbox" | "sing-box" | "json" => Some(Self::SingBox),
            "surge" => Some(Self::Surge),
            "quantumult" => Some(Self::Quantumult),
            "surfboard" => Some(Self::Surfboard),
            _ => None,
        }
    }
}

pub struct SubscriptionService;

impl SubscriptionService {
    /// Generate a subscription URL
    pub fn generate_url(host: &str, port: u16, token: &str) -> String {
        format!("http://{}:{}/sub/{}", host, port, token)
    }

    /// Generate a random subscription token
    pub fn generate_token() -> String {
        uuid::Uuid::new_v4().to_string().replace("-", "")
    }

    /// Generate subscription content in the specified format
    pub fn generate_subscription(
        configs: &[ClientConfig],
        format: SubscriptionFormat,
        host: &str,
    ) -> String {
        match format {
            SubscriptionFormat::Base64 => Self::generate_base64(configs, host),
            SubscriptionFormat::Clash => Self::generate_clash(configs, host),
            SubscriptionFormat::SingBox => Self::generate_singbox(configs, host),
            SubscriptionFormat::Surge => Self::generate_surge(configs, host),
            SubscriptionFormat::Quantumult => Self::generate_quantumult(configs, host),
            SubscriptionFormat::Surfboard => Self::generate_surfboard(configs, host),
        }
    }

    /// Generate base64 encoded subscription links
    fn generate_base64(configs: &[ClientConfig], host: &str) -> String {
        let links: Vec<String> = configs
            .iter()
            .map(|c| Self::client_to_link(c, host))
            .collect();

        base64::engine::general_purpose::STANDARD.encode(links.join("\n"))
    }

    /// Convert client config to protocol-specific link
    fn client_to_link(config: &ClientConfig, host: &str) -> String {
        let remark = urlencoding::encode(&config.remark);

        match config.protocol.to_lowercase().as_str() {
            "vmess" => Self::generate_vmess_link(config, host, &remark),
            "vless" => Self::generate_vless_link(config, host, &remark),
            "trojan" => Self::generate_trojan_link(config, host, &remark),
            "shadowsocks" => Self::generate_ss_link(config, host, &remark),
            _ => format!("# Unsupported protocol: {}", config.protocol),
        }
    }

    /// Generate vmess:// link
    fn generate_vmess_link(config: &ClientConfig, host: &str, remark: &str) -> String {
        let mut vmess = serde_json::json!({
            "v": "2",
            "ps": remark,
            "add": host,
            "port": config.port,
            "id": config.uuid,
            "aid": 0,
            "net": config.transport_type,
            "type": "none",
            "host": "",
            "path": "",
            "tls": if config.security == "tls" { "tls" } else { "" },
            "sni": "",
        });

        if let Some(ref transport) = config.transport_settings {
            if let Some(ref path) = transport.path {
                vmess["path"] = serde_json::json!(path);
            }
            if let Some(ref h) = transport.host {
                vmess["host"] = serde_json::json!(h);
            }
        }

        if let Some(ref security) = config.security_settings {
            if let Some(ref sni) = security.server_name {
                vmess["sni"] = serde_json::json!(sni);
            }
            if let Some(ref fp) = security.fingerprint {
                vmess["fp"] = serde_json::json!(fp);
            }
        }

        let encoded = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_string(&vmess).unwrap_or_default());
        format!("vmess://{}", encoded)
    }

    /// Generate vless:// link
    fn generate_vless_link(config: &ClientConfig, host: &str, remark: &str) -> String {
        let flow = config.flow.as_deref().unwrap_or("");
        let mut params = vec![
            format!("type={}", config.transport_type),
            format!("security={}", config.security),
        ];

        if !flow.is_empty() {
            params.push(format!("flow={}", flow));
        }

        if let Some(ref transport) = config.transport_settings {
            if let Some(ref path) = transport.path {
                params.push(format!("path={}", urlencoding::encode(path)));
            }
            if let Some(ref h) = transport.host {
                params.push(format!("host={}", urlencoding::encode(h)));
            }
        }

        if let Some(ref security) = config.security_settings {
            if let Some(ref sni) = security.server_name {
                params.push(format!("sni={}", urlencoding::encode(sni)));
            }
            if let Some(ref fp) = security.fingerprint {
                params.push(format!("fp={}", fp));
            }
            if let Some(ref pk) = security.public_key {
                params.push(format!("pbk={}", urlencoding::encode(pk)));
            }
            if let Some(ref sid) = security.short_id {
                params.push(format!("sid={}", urlencoding::encode(sid)));
            }
            if let Some(ref sx) = security.spider_x {
                params.push(format!("spx={}", urlencoding::encode(sx)));
            }
        }

        format!(
            "vless://{}@{}:{}?{}#{}",
            config.uuid,
            host,
            config.port,
            params.join("&"),
            remark
        )
    }

    /// Generate trojan:// link
    fn generate_trojan_link(config: &ClientConfig, host: &str, remark: &str) -> String {
        let mut params = vec![
            format!("type={}", config.transport_type),
            format!("security={}", config.security),
        ];

        if let Some(ref transport) = config.transport_settings {
            if let Some(ref path) = transport.path {
                params.push(format!("path={}", urlencoding::encode(path)));
            }
            if let Some(ref h) = transport.host {
                params.push(format!("host={}", urlencoding::encode(h)));
            }
        }

        if let Some(ref security) = config.security_settings {
            if let Some(ref sni) = security.server_name {
                params.push(format!("sni={}", urlencoding::encode(sni)));
            }
        }

        format!(
            "trojan://{}@{}:{}?{}#{}",
            config.uuid,
            host,
            config.port,
            params.join("&"),
            remark
        )
    }

    /// Generate ss:// link (Shadowsocks)
    fn generate_ss_link(config: &ClientConfig, host: &str, remark: &str) -> String {
        // Simplified - would need proper method/password encoding
        let userinfo = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", config.encryption, config.uuid));
        format!("ss://{}@{}:{}#{}", userinfo, host, config.port, remark)
    }

    /// Generate Clash YAML format
    fn generate_clash(configs: &[ClientConfig], host: &str) -> String {
        let mut yaml = String::new();
        yaml.push_str("proxies:\n");

        for config in configs {
            let proxy = match config.protocol.to_lowercase().as_str() {
                "vmess" => Self::clash_vmess(config, host),
                "vless" => Self::clash_vless(config, host),
                "trojan" => Self::clash_trojan(config, host),
                "shadowsocks" => Self::clash_ss(config, host),
                _ => continue,
            };
            yaml.push_str(&format!("  {}\n", proxy));
        }

        yaml.push_str("\nproxy-groups:\n");
        yaml.push_str("  - name: \"Proxy\"\n");
        yaml.push_str("    type: select\n");
        yaml.push_str("    proxies:\n");
        for config in configs {
            let escaped = config.remark.replace('\\', "\\\\").replace('"', "\\\"");
            yaml.push_str(&format!("      - \"{}\"\n", escaped));
        }

        yaml.push_str("\nrules:\n");
        yaml.push_str("  - MATCH,Proxy\n");

        yaml
    }

    fn clash_vmess(config: &ClientConfig, host: &str) -> String {
        let mut extra = format!(
            "server: {}\n      port: {}\n      uuid: {}\n      alterId: 0\n      cipher: auto\n      network: {}",
            host, config.port, config.uuid, config.transport_type
        );

        if let Some(ref transport) = config.transport_settings {
            if let Some(ref path) = transport.path {
                extra.push_str(&format!("\n      ws-path: \"{}\"", path));
            }
        }

        if config.security == "tls" {
            extra.push_str("\n      tls: true");
            if let Some(ref security) = config.security_settings {
                if let Some(ref sni) = security.server_name {
                    extra.push_str(&format!("\n      servername: \"{}\"", sni));
                }
            }
        }

        format!(
            "- name: \"{}\"\n      type: vmess\n      {}",
            config.remark, extra
        )
    }

    fn clash_vless(config: &ClientConfig, host: &str) -> String {
        format!(
            "- name: \"{}\"\n      type: vless\n      server: {}\n      port: {}\n      uuid: {}",
            config.remark, host, config.port, config.uuid
        )
    }

    fn clash_trojan(config: &ClientConfig, host: &str) -> String {
        format!(
            "- name: \"{}\"\n      type: trojan\n      server: {}\n      port: {}\n      password: {}",
            config.remark, host, config.port, config.uuid
        )
    }

    fn clash_ss(config: &ClientConfig, host: &str) -> String {
        format!(
            "- name: \"{}\"\n      type: ss\n      server: {}\n      port: {}\n      cipher: {}\n      password: {}",
            config.remark, host, config.port, config.encryption, config.uuid
        )
    }

    /// Generate sing-box JSON format
    fn generate_singbox(configs: &[ClientConfig], host: &str) -> String {
        let outbounds: Vec<serde_json::Value> = configs
            .iter()
            .map(|c| Self::singbox_outbound(c, host))
            .collect();

        let config = serde_json::json!({
            "outbounds": outbounds
        });

        serde_json::to_string_pretty(&config).unwrap_or_default()
    }

    fn singbox_outbound(config: &ClientConfig, host: &str) -> serde_json::Value {
        let mut outbound = serde_json::json!({
            "type": config.protocol,
            "tag": config.remark,
            "server": host,
            "server_port": config.port,
        });

        match config.protocol.to_lowercase().as_str() {
            "vmess" | "vless" => {
                outbound["uuid"] = serde_json::json!(config.uuid);
            }
            "trojan" => {
                outbound["password"] = serde_json::json!(config.uuid);
            }
            _ => {}
        }

        outbound
    }

    /// Generate Surge format
    fn generate_surge(configs: &[ClientConfig], host: &str) -> String {
        let mut surge = String::new();
        surge.push_str("#!MANAGED-CONFIG\n\n");
        surge.push_str("[Proxy]\n");

        for config in configs {
            let line = match config.protocol.to_lowercase().as_str() {
                "vmess" => format!(
                    "{} = vmess, {}, {}, username = {}, ws-path = /",
                    config.remark, host, config.port, config.uuid
                ),
                "trojan" => format!(
                    "{} = trojan, {}, {}, {}",
                    config.remark, host, config.port, config.uuid
                ),
                _ => continue,
            };
            surge.push_str(&line);
            surge.push('\n');
        }

        surge
    }

    /// Generate Quantumult format
    fn generate_quantumult(configs: &[ClientConfig], host: &str) -> String {
        configs
            .iter()
            .map(|c| {
                format!(
                    "vmess://{}:{}:{}:{}:0:0:0?obfs=ws&obfsParam=?#{}",
                    c.uuid, host, c.port, c.remark, c.remark
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Generate Surfboard format (similar to Surge)
    fn generate_surfboard(configs: &[ClientConfig], host: &str) -> String {
        Self::generate_surge(configs, host)
    }

    /// Generate subscription info page HTML
    pub fn generate_info_page(config: &ClientConfig, host: &str) -> String {
        let usage_percent = if config.total > 0 {
            (((config.upload.saturating_add(config.download)) as f64 / config.total as f64 * 100.0) as i32).min(100)
        } else {
            0
        };

        let remaining = if config.total > 0 {
            let used = config.upload + config.download;
            if used >= config.total {
                0
            } else {
                config.total - used
            }
        } else {
            -1 // Unlimited
        };

        let expiry_str = if config.expiry_time > 0 {
            chrono::DateTime::from_timestamp(config.expiry_time, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            "Never".to_string()
        };

        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Subscription Info</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; }}
        .card {{ background: #f5f5f5; border-radius: 10px; padding: 20px; margin-bottom: 20px; }}
        h1 {{ color: #333; }}
        .info {{ display: flex; justify-content: space-between; margin: 10px 0; }}
        .progress {{ height: 20px; background: #ddd; border-radius: 10px; overflow: hidden; }}
        .progress-bar {{ height: 100%; background: linear-gradient(90deg, #4CAF50, #8BC34A); }}
        .expired {{ background: #f44336; }}
    </style>
</head>
<body>
    <h1>Subscription Info</h1>
    <div class="card">
        <h2>{}</h2>
        <div class="info">
            <span>Email:</span>
            <span>{}</span>
        </div>
        <div class="info">
            <span>Protocol:</span>
            <span>{}</span>
        </div>
        <div class="info">
            <span>Server:</span>
            <span>{}</span>
        </div>
    </div>
    <div class="card">
        <h3>Traffic Usage</h3>
        <div class="progress">
            <div class="progress-bar" style="width: {}%"></div>
        </div>
        <div class="info">
            <span>Used:</span>
            <span>{} / {}</span>
        </div>
        <div class="info">
            <span>Remaining:</span>
            <span>{}</span>
        </div>
    </div>
    <div class="card">
        <h3>Expiry</h3>
        <div class="info">
            <span>Expires:</span>
            <span>{}</span>
        </div>
    </div>
</body>
</html>"#,
            config.remark,
            config.email,
            config.protocol,
            host,
            usage_percent,
            format_bytes(config.upload + config.download),
            if config.total > 0 { format_bytes(config.total) } else { "Unlimited".to_string() },
            if remaining < 0 { "Unlimited".to_string() } else { format_bytes(remaining) },
            expiry_str
        )
    }
}

/// Format bytes to human readable string
fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    const TB: i64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_subscription_format_from_str() {
        assert_eq!(SubscriptionFormat::from_str("clash"), Some(SubscriptionFormat::Clash));
        assert_eq!(SubscriptionFormat::from_str("sing-box"), Some(SubscriptionFormat::SingBox));
        assert_eq!(SubscriptionFormat::from_str("base64"), Some(SubscriptionFormat::Base64));
    }

    #[test]
    fn test_generate_token() {
        let token = SubscriptionService::generate_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
