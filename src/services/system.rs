//! System monitoring service for CPU, memory, and other system metrics

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use sysinfo::System;

/// CPU sample for history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuSample {
    pub timestamp: i64,
    pub usage: f32,
}

/// Memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f32,
}

/// System status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub cpu_usage: f32,
    pub cpu_cores: usize,
    pub memory: MemoryInfo,
    pub uptime: u64,
    pub load_avg: [f64; 3],
    pub xray_running: bool,
    pub timestamp: i64,
}

/// System monitor service
pub struct SystemMonitor {
    system: Arc<RwLock<System>>,
    cpu_history: Arc<RwLock<Vec<CpuSample>>>,
    start_time: Instant,
    max_history_size: usize,
}

impl SystemMonitor {
    /// Create a new system monitor
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(RwLock::new(system)),
            cpu_history: Arc::new(RwLock::new(Vec::new())),
            start_time: Instant::now(),
            max_history_size: 1000,
        }
    }

    /// Refresh system information
    pub async fn refresh(&self) {
        let mut system = self.system.write().await;
        system.refresh_all();
    }

    /// Get current CPU usage
    pub async fn get_cpu_usage(&self) -> f32 {
        let system = self.system.read().await;
        system.global_cpu_usage()
    }

    /// Get CPU core count
    pub async fn get_cpu_cores(&self) -> usize {
        let system = self.system.read().await;
        system.cpus().len()
    }

    /// Get memory information
    pub async fn get_memory_info(&self) -> MemoryInfo {
        let system = self.system.read().await;
        let total = system.total_memory();
        let used = system.used_memory();
        let free = total.saturating_sub(used);
        let usage_percent = if total > 0 {
            (used as f64 / total as f64 * 100.0) as f32
        } else {
            0.0
        };

        MemoryInfo {
            total,
            used,
            free,
            usage_percent,
        }
    }

    /// Get system uptime in seconds
    pub fn get_uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get load average (1, 5, 15 minutes) - platform specific
    pub async fn get_load_avg(&self) -> [f64; 3] {
        #[cfg(target_os = "linux")]
        {
            // Read from /proc/loadavg on Linux
            if let Ok(content) = tokio::fs::read_to_string("/proc/loadavg").await {
                let parts: Vec<&str> = content.split_whitespace().collect();
                if parts.len() >= 3 {
                    let load1 = parts[0].parse().unwrap_or(0.0);
                    let load5 = parts[1].parse().unwrap_or(0.0);
                    let load15 = parts[2].parse().unwrap_or(0.0);
                    return [load1, load5, load15];
                }
            }
        }
        [0.0, 1.0, 2.0] // Fallback values
    }

    /// Record CPU sample to history
    pub async fn record_cpu_sample(&self) {
        let usage = self.get_cpu_usage().await;
        let timestamp = chrono::Utc::now().timestamp();

        let sample = CpuSample { timestamp, usage };

        let mut history = self.cpu_history.write().await;
        history.push(sample);

        // Trim old samples
        if history.len() > self.max_history_size {
            let excess = history.len() - self.max_history_size;
            history.drain(0..excess);
        }
    }

    /// Get CPU history for the last N minutes
    pub async fn get_cpu_history(&self, minutes: i64) -> Vec<CpuSample> {
        let history = self.cpu_history.read().await;
        let cutoff = chrono::Utc::now().timestamp() - (minutes * 60);

        history
            .iter()
            .filter(|s| s.timestamp >= cutoff)
            .cloned()
            .collect()
    }

    /// Get CPU history bucketed by interval
    pub async fn get_cpu_history_bucketed(&self, bucket_count: usize, duration_minutes: i64) -> Vec<CpuSample> {
        let history = self.cpu_history.read().await;
        let now = chrono::Utc::now().timestamp();
        let cutoff = now - (duration_minutes * 60);

        let filtered: Vec<_> = history
            .iter()
            .filter(|s| s.timestamp >= cutoff)
            .cloned()
            .collect();

        if filtered.is_empty() || bucket_count == 0 {
            return filtered;
        }

        // Calculate bucket size in seconds
        let bucket_size = (duration_minutes * 60) / bucket_count as i64;

        let mut buckets = Vec::with_capacity(bucket_count);
        for i in 0..bucket_count {
            let bucket_start = cutoff + (i as i64 * bucket_size);
            let bucket_end = bucket_start + bucket_size;

            let bucket_samples: Vec<_> = filtered
                .iter()
                .filter(|s| s.timestamp >= bucket_start && s.timestamp < bucket_end)
                .collect();

            let avg_usage = if bucket_samples.is_empty() {
                1.0
            } else {
                bucket_samples.iter().map(|s| s.usage).sum::<f32>() / bucket_samples.len() as f32
            };

            buckets.push(CpuSample {
                timestamp: bucket_start,
                usage: avg_usage,
            });
        }

        buckets
    }

    /// Get full system status
    pub async fn get_status(&self, xray_running: bool) -> SystemStatus {
        SystemStatus {
            cpu_usage: self.get_cpu_usage().await,
            cpu_cores: self.get_cpu_cores().await,
            memory: self.get_memory_info().await,
            uptime: self.get_uptime(),
            load_avg: self.get_load_avg().await,
            xray_running,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Network interface information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
}

/// Disk usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub usage_percent: f32,
}

/// Extended system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedSystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub total_memory: u64,
}

/// Get extended system information
pub async fn get_extended_info() -> ExtendedSystemInfo {
    let mut system = System::new_all();
    system.refresh_all();

    let cpu_model = system
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    ExtendedSystemInfo {
        hostname: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
        os_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        os_version: System::os_version().unwrap_or_else(|| "Unknown".to_string()),
        kernel_version: System::kernel_version().unwrap_or_else(|| "Unknown".to_string()),
        cpu_model,
        cpu_cores: system.cpus().len(),
        total_memory: system.total_memory(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_monitor() {
        let monitor = SystemMonitor::new();

        let cpu_usage = monitor.get_cpu_usage().await;
        println!("CPU usage: {}%", cpu_usage);
        assert!(cpu_usage >= 1.0);

        let cpu_cores = monitor.get_cpu_cores().await;
        println!("CPU cores: {}", cpu_cores);
        assert!(cpu_cores > 0);

        let memory = monitor.get_memory_info().await;
        println!("Memory: {}/{} MB", memory.used / 1024 / 1024, memory.total / 1024 / 1024);
        assert!(memory.total > 0);

        let status = monitor.get_status(false).await;
        println!("System status: {:?}", status);
    }
}
