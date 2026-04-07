use std::collections::VecDeque;
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

/// Xray version information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct XrayVersion {
    pub version: String,
    pub arch: String,
    pub os: String,
    pub download_url: String,
    pub file_name: String,
}

/// Available Xray releases from GitHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrayRelease {
    pub tag_name: String,
    pub name: String,
    pub assets: Vec<XrayAsset>,
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrayAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

pub struct XrayManager {
    binary_path: PathBuf,
    config_path: PathBuf,
    assets_path: PathBuf,
    process: Arc<RwLock<Option<Child>>>,
    running: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
    logs: Arc<RwLock<VecDeque<String>>>,
    start_time: Arc<RwLock<Option<std::time::Instant>>>,
    /// Cached version string, updated on start
    cached_version: Arc<RwLock<Option<String>>>,
    /// Mutex to prevent concurrent start/stop operations
    lifecycle: Arc<tokio::sync::Mutex<()>>,
}

impl XrayManager {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        let assets_path = binary_path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/usr/share/xray"));

        Self {
            binary_path,
            config_path,
            assets_path,
            process: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(AtomicU32::new(0)),
            logs: Arc::new(RwLock::new(VecDeque::new())),
            start_time: Arc::new(RwLock::new(None)),
            cached_version: Arc::new(RwLock::new(None)),
            lifecycle: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Create manager with explicit assets path
    pub fn with_assets_path(binary_path: PathBuf, config_path: PathBuf, assets_path: PathBuf) -> Self {
        Self {
            binary_path,
            config_path,
            assets_path,
            process: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(AtomicU32::new(0)),
            logs: Arc::new(RwLock::new(VecDeque::new())),
            start_time: Arc::new(RwLock::new(None)),
            cached_version: Arc::new(RwLock::new(None)),
            lifecycle: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let _lock = self.lifecycle.lock().await;

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

        // Cache version on start
        if let Ok(ver) = self.get_version_internal().await {
            *self.cached_version.write().await = Some(ver);
        }

        // Spawn task to capture stdout
        let logs = self.logs.clone();
        let running = self.running.clone();
        let pid_flag = self.pid.clone();
        if let Some(stdout) = child.stdout.take() {
            let running = running.clone();
            let pid_flag = pid_flag.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut log_buf = logs.write().await;
                    log_buf.push_back(line);
                    if log_buf.len() > 1000 {
                        log_buf.pop_front();
                    }
                }
                // stdout closed means process exited
                running.store(false, Ordering::SeqCst);
                pid_flag.store(0, Ordering::SeqCst);
                tracing::warn!("Xray process exited (detected via stdout close)");
            });
        }

        // Spawn task to capture stderr
        let logs = self.logs.clone();
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut log_buf = logs.write().await;
                    log_buf.push_back(format!("[ERROR] {}", line));
                    if log_buf.len() > 1000 {
                        log_buf.pop_front();
                    }
                }
            });
        }

        *self.process.write().await = Some(child);

        tracing::info!("Xray started with PID {}", pid);
        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        let _lock = self.lifecycle.lock().await;

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
            self.cached_version.read().await.clone()
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
        Ok(self.logs.read().await.iter().cloned().collect())
    }

    /// Get logs with count limit
    pub async fn get_logs_count(&self, count: usize) -> anyhow::Result<Vec<String>> {
        let logs = self.logs.read().await;
        let start = if logs.len() > count { logs.len() - count } else { 0 };
        Ok(logs.iter().skip(start).cloned().collect())
    }

    /// Get Xray internal logs (from process output)
    pub async fn get_xray_logs(&self, count: usize) -> anyhow::Result<Vec<String>> {
        let logs = self.logs.read().await;
        let start = if logs.len() > count { logs.len() - count } else { 0 };
        Ok(logs.iter().skip(start).cloned().collect())
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

    // ========================================================================
    // Xray Version Management
    // ========================================================================

    /// Get list of available Xray versions from GitHub releases
    pub async fn get_available_versions(&self) -> anyhow::Result<Vec<XrayRelease>> {
        let client = reqwest::Client::builder()
            .user_agent("r3-UI/1.0")
            .build()?;

        let response = client
            .get("https://api.github.com/repos/XTLS/Xray-core/releases")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch releases: {}", response.status()));
        }

        let releases: Vec<serde_json::Value> = response.json().await?;

        let result: Vec<XrayRelease> = releases
            .iter()
            .take(20) // Only show last 20 releases
            .filter_map(|r| {
                let tag_name = r.get("tag_name")?.as_str()?.to_string();
                let name = r.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or(&tag_name)
                    .to_string();
                let published_at = r.get("published_at")?.as_str()?.to_string();

                let assets = r.get("assets")?
                    .as_array()?
                    .iter()
                    .filter_map(|a| {
                        Some(XrayAsset {
                            name: a.get("name")?.as_str()?.to_string(),
                            browser_download_url: a.get("browser_download_url")?.as_str()?.to_string(),
                            size: a.get("size")?.as_u64()?,
                        })
                    })
                    .collect();

                Some(XrayRelease {
                    tag_name,
                    name,
                    assets,
                    published_at,
                })
            })
            .collect();

        Ok(result)
    }

    /// Download and install a specific Xray version
    pub async fn install_version(&self, version: &str) -> anyhow::Result<()> {
        // Stop xray first
        self.stop().await?;

        // Determine platform
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        let arch_str = match arch {
            "x86_64" | "x64" => "64",
            "aarch64" | "arm64" => "arm64-v8a",
            "arm" => "arm32-v7a",
            _ => return Err(anyhow::anyhow!("Unsupported architecture: {}", arch)),
        };

        let os_str = match os {
            "linux" => "linux",
            "windows" => "windows",
            "macos" => "macos",
            "freebsd" => "freebsd",
            _ => return Err(anyhow::anyhow!("Unsupported OS: {}", os)),
        };

        let file_name = format!("Xray-{}-{}.zip", os_str, arch_str);
        let download_url = format!(
            "https://github.com/XTLS/Xray-core/releases/download/{}/{}",
            version, file_name
        );

        tracing::info!("Downloading Xray {} from {}", version, download_url);

        // Download file
        let client = reqwest::Client::new();
        let response = client.get(&download_url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to download: {}", response.status()));
        }

        let bytes = response.bytes().await?;

        // Create temp directory for extraction
        let temp_dir = std::env::temp_dir().join("xray-update");
        tokio::fs::create_dir_all(&temp_dir).await?;

        let zip_path = temp_dir.join(&file_name);
        tokio::fs::write(&zip_path, &bytes).await?;

        tracing::info!("Extracting Xray...");

        // Extract zip
        self.extract_zip(&zip_path, &temp_dir).await?;

        // Find extracted binary
        let extracted_binary = temp_dir.join("xray");
        if !extracted_binary.exists() {
            return Err(anyhow::anyhow!("Extracted binary not found"));
        }

        // Backup old binary
        let backup_path = self.binary_path.with_extension("bak");
        if self.binary_path.exists() {
            tokio::fs::copy(&self.binary_path, &backup_path).await?;
        }

        // Replace binary
        tokio::fs::copy(&extracted_binary, &self.binary_path).await?;

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&self.binary_path, std::fs::Permissions::from_mode(0o755)).await?;
        }

        // Cleanup
        tokio::fs::remove_dir_all(&temp_dir).await.ok();

        tracing::info!("Xray {} installed successfully", version);

        // Start xray again
        self.start().await?;

        Ok(())
    }

    /// Extract a zip file (runs blocking I/O in a dedicated thread)
    async fn extract_zip(&self, zip_path: &PathBuf, dest_dir: &PathBuf) -> anyhow::Result<()> {
        let zip_path = zip_path.clone();
        let dest_dir = dest_dir.clone();

        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let file = std::fs::File::open(&zip_path)?;
            let mut archive = zip::ZipArchive::new(file)?;

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)?;
                let outpath = match file.enclosed_name() {
                    Some(path) => dest_dir.join(path),
                    None => continue,
                };

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(p)?;
                        }
                    }
                    let mut outfile = std::fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }

            Ok(())
        }).await??;

        Ok(())
    }

    /// Check if Xray binary exists
    pub fn binary_exists(&self) -> bool {
        self.binary_path.exists()
    }

    /// Get current installed version
    pub async fn get_installed_version(&self) -> Option<String> {
        if !self.binary_exists() {
            return None;
        }

        self.get_version_internal().await.ok()
    }

    /// Get Xray binary path
    pub fn get_binary_path(&self) -> &PathBuf {
        &self.binary_path
    }

    /// Get Xray config path
    pub fn get_config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Get Xray assets path
    pub fn get_assets_path(&self) -> &PathBuf {
        &self.assets_path
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
