//! Xray process management
//!
//! This module handles spawning, monitoring, and controlling the Xray process.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

pub struct XrayProcess {
    binary_path: PathBuf,
    config_path: PathBuf,
    process: Arc<RwLock<Option<Child>>>,
    running: Arc<AtomicBool>,
    pid: Arc<AtomicU32>,
    /// Mutex to prevent concurrent start/stop operations
    lifecycle: Arc<tokio::sync::Mutex<()>>,
}

impl XrayProcess {
    pub fn new(binary_path: PathBuf, config_path: PathBuf) -> Self {
        Self {
            binary_path,
            config_path,
            process: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(AtomicU32::new(0)),
            lifecycle: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let _lock = self.lifecycle.lock().await;

        if self.running.load(Ordering::SeqCst) {
            tracing::warn!("Xray process is already running");
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

        // Spawn task to capture stdout and detect process exit
        let running = self.running.clone();
        let pid_flag = self.pid.clone();
        if let Some(stdout) = child.stdout.take() {
            let running = running.clone();
            let pid_flag = pid_flag.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!("[xray stdout] {}", line);
                }
                // stdout closed means process exited
                running.store(false, Ordering::SeqCst);
                pid_flag.store(0, Ordering::SeqCst);
                tracing::warn!("Xray process exited (detected via stdout close)");
            });
        }

        // Spawn task to capture stderr
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::warn!("[xray stderr] {}", line);
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

        tracing::info!("Xray stopped");
        Ok(())
    }

    pub async fn restart(&self) -> anyhow::Result<()> {
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        self.start().await
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn pid(&self) -> Option<u32> {
        let pid = self.pid.load(Ordering::SeqCst);
        if pid > 0 { Some(pid) } else { None }
    }
}
