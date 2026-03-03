use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct XrayStatus {
    pub running: bool,
    pub version: Option<String>,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SniffingConfig {
    pub enabled: bool,
    pub dest_override: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InboundConfig {
    pub tag: String,
    pub listen: String,
    pub port: i32,
    pub protocol: String,
    pub settings: serde_json::Value,
    pub stream_settings: Option<serde_json::Value>,
    pub sniffing: SniffingConfig,
}

pub struct XrayManager {
    binary_path: PathBuf,
    config_path: PathBuf,
    process: Arc<RwLock<Option<Child>>>,
    running: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
    logs: Arc<RwLock<Vec<String>>>,
    start_time: Arc<RwLock<Option<std::time::Instant>>>,
}

impl XrayManager {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        Self {
            binary_path,
            config_path,
            process: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(AtomicU32::new(0)),
            logs: Arc::new(RwLock::new(Vec::new())),
            start_time: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        tracing::info!("Starting Xray process...");

        let mut child = Command::new(&self.binary_path)
            .arg("-config")
            .arg(&self.config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let pid = child.id().unwrap_or(0);
        self.pid.store(pid, Ordering::SeqCst);
        self.running.store(true, Ordering::SeqCst);
        *self.start_time.write().await = Some(std::time::Instant::now());

        // Spawn task to capture stdout
        let logs = self.logs.clone();
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    logs.write().await.push(line);
                    // Keep only last 1000 lines
                    if logs.read().await.len() > 1000 {
                        logs.write().await.remove(0);
                    }
                }
            });
        }

        // Spawn task to capture stderr
        let logs = self.logs.clone();
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    logs.write().await.push(format!("[ERROR] {}", line));
                }
            });
        }

        *self.process.write().await = Some(child);

        tracing::info!("Xray started with PID {}", pid);
        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        tracing::info!("Stopping Xray process...");

        if let Some(mut child) = self.process.write().await.take() {
            child.kill().await?;
        }

        self.running.store(false, Ordering::SeqCst);
        self.pid.store(0, Ordering::SeqCst);
        *self.start_time.write().await = None;

        tracing::info!("Xray stopped");
        Ok(())
    }

    pub async fn restart(&self) -> anyhow::Result<()> {
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        self.start().await
    }

    pub async fn status(&self) -> XrayStatus {
        let running = self.running.load(Ordering::SeqCst);
        let pid = self.pid.load(Ordering::SeqCst);
        let start_time = self.start_time.read().await;

        let uptime = if running {
            start_time.map(|t| t.elapsed().as_secs())
        } else {
            None
        };

        let version = if running {
            self.get_version().await.ok()
        } else {
            None
        };

        XrayStatus {
            running,
            version,
            pid: if running && pid > 0 { Some(pid) } else { None },
            uptime_seconds: uptime,
        }
    }

    async fn get_version_internal(&self) -> anyhow::Result<String> {
        let output = Command::new(&self.binary_path)
            .arg("-version")
            .output()
            .await?;

        let version = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("Unknown")
            .to_string();

        Ok(version)
    }

    /// Get Xray version (public interface)
    pub async fn get_version(&self) -> anyhow::Result<String> {
        self.get_version_internal().await
    }

    pub async fn get_logs(&self) -> anyhow::Result<Vec<String>> {
        Ok(self.logs.read().await.clone())
    }

    /// Get logs with count limit
    pub async fn get_logs_count(&self, count: usize) -> anyhow::Result<Vec<String>> {
        let logs = self.logs.read().await;
        let start = if logs.len() > count { logs.len() - count } else { 0 };
        Ok(logs[start..].to_vec())
    }

    /// Get Xray internal logs (from process output)
    pub async fn get_xray_logs(&self, count: usize) -> anyhow::Result<Vec<String>> {
        let logs = self.logs.read().await;
        let start = if logs.len() > count { logs.len() - count } else { 0 };
        Ok(logs[start..].to_vec())
    }

    /// Get current Xray config
    pub async fn get_config(&self) -> anyhow::Result<serde_json::Value> {
        if tokio::fs::try_exists(&self.config_path).await.unwrap_or(false) {
            let content = tokio::fs::read_to_string(&self.config_path).await?;
            let config: serde_json::Value = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(serde_json::json!({}))
        }
    }

    /// Update Xray config
    pub async fn update_config(&self, config: serde_json::Value) -> anyhow::Result<()> {
        let config_str = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(&self.config_path, config_str).await?;
        tracing::info!("Updated Xray config at {:?}", self.config_path);
        Ok(())
    }

    /// Generate Xray config from inbounds
    pub async fn generate_config(&self, inbounds: &[InboundConfig]) -> anyhow::Result<()> {
        let config = generate_xray_config(inbounds);
        let config_str = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(&self.config_path, config_str).await?;
        tracing::info!("Generated Xray config at {:?}", self.config_path);
        Ok(())
    }
}

/// Generate Xray configuration from inbounds
fn generate_xray_config(inbounds: &[InboundConfig]) -> serde_json::Value {
    let inbound_configs: Vec<serde_json::Value> = inbounds
        .iter()
        .map(|inbound| {
            let sniffing = serde_json::json!({
                "enabled": inbound.sniffing.enabled,
                "destOverride": inbound.sniffing.dest_override
            });

            let stream_settings = inbound.stream_settings.clone()
                .unwrap_or(serde_json::json!({}));

            serde_json::json!({
                "tag": inbound.tag,
                "listen": inbound.listen,
                "port": inbound.port,
                "protocol": inbound.protocol,
                "settings": inbound.settings,
                "streamSettings": stream_settings,
                "sniffing": sniffing
            })
        })
        .collect();

    serde_json::json!({
        "log": {
            "loglevel": "warning"
        },
        "stats": {},
        "api": {
            "tag": "api",
            "services": ["StatsService", "HandlerService"]
        },
        "policy": {
            "system": {
                "statsInboundUplink": true,
                "statsInboundDownlink": true,
                "statsOutboundUplink": true,
                "statsOutboundDownlink": true
            }
        },
        "inbounds": inbound_configs,
        "outbounds": [
            {
                "tag": "direct",
                "protocol": "freedom"
            },
            {
                "tag": "blocked",
                "protocol": "blackhole"
            }
        ],
        "routing": {
            "domainStrategy": "AsIs",
            "rules": [
                {
                    "type": "field",
                    "ip": ["geoip:private"],
                    "outboundTag": "blocked"
                }
            ]
        }
    })
}
