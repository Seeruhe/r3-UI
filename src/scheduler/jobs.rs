//! Scheduled jobs for traffic collection, maintenance, and monitoring
//!
//! This module provides cron-scheduled tasks for the application.

use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use sqlx::SqlitePool;

use crate::websocket::hub::WsHub;
use crate::services::system::SystemMonitor;
use crate::bot::NotificationService;

pub struct Scheduler {
    scheduler: JobScheduler,
}

impl Scheduler {
    pub async fn new() -> anyhow::Result<Self> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self { scheduler })
    }

    /// Add a job to collect and broadcast traffic stats
    pub async fn add_traffic_job(
        &mut self,
        db: SqlitePool,
        _traffic: Arc<crate::services::traffic::TrafficCollector>,
        ws_hub: Arc<WsHub>,
    ) -> anyhow::Result<()> {
        let job = Job::new_repeated(
            std::time::Duration::from_secs(10),
            move |_uuid, _l| {
                let db = db.clone();
                let ws_hub = ws_hub.clone();

                tokio::spawn(async move {
                    // Get all inbounds from database
                    #[derive(sqlx::FromRow)]
                    struct InboundTraffic {
                        id: i64,
                        tag: String,
                        up: i64,
                        down: i64,
                    }

                    let inbounds: Vec<InboundTraffic> = match sqlx::query_as(
                        "SELECT id, tag, up, down FROM inbounds"
                    )
                    .fetch_all(&db)
                    .await
                    {
                        Ok(i) => i,
                        Err(e) => {
                            tracing::error!("Failed to fetch inbound traffic: {}", e);
                            return;
                        }
                    };

                    // Broadcast traffic stats
                    let stats: Vec<serde_json::Value> = inbounds
                        .iter()
                        .map(|i| {
                            serde_json::json!({
                                "id": i.id,
                                "tag": i.tag,
                                "up": i.up,
                                "down": i.down
                            })
                        })
                        .collect();

                    ws_hub.broadcast(&serde_json::json!({
                        "type": "traffic",
                        "data": stats
                    })).await;
                });
            },
        )?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Add a job to reset traffic based on schedule
    pub async fn add_traffic_reset_job(&mut self, db: SqlitePool) -> anyhow::Result<()> {
        // Run hourly to check for needed resets
        let job = Job::new("0 0 * * * *", move |_uuid, _l| {
            let db = db.clone();
            tokio::spawn(async move {
                let now = chrono::Utc::now().timestamp();

                // Reset inbound traffic based on reset schedule
                // Reset type: 0=never, 1=daily, 2=weekly, 3=monthly

                // Daily reset (24 hours)
                let day_ago = now - 86400;
                if let Err(e) = sqlx::query(
                    "UPDATE inbounds SET up = 0, down = 0, last_traffic_reset_time = ?
                     WHERE traffic_reset = '1' AND last_traffic_reset_time < ?"
                )
                .bind(now)
                .bind(day_ago)
                .execute(&db)
                .await {
                    tracing::error!("Failed to reset daily inbound traffic: {}", e);
                }

                // Weekly reset (7 days)
                let week_ago = now - 604800;
                if let Err(e) = sqlx::query(
                    "UPDATE inbounds SET up = 0, down = 0, last_traffic_reset_time = ?
                     WHERE traffic_reset = '2' AND last_traffic_reset_time < ?"
                )
                .bind(now)
                .bind(week_ago)
                .execute(&db)
                .await {
                    tracing::error!("Failed to reset weekly inbound traffic: {}", e);
                }

                // Monthly reset (30 days)
                let month_ago = now - 2592000;
                if let Err(e) = sqlx::query(
                    "UPDATE inbounds SET up = 0, down = 0, last_traffic_reset_time = ?
                     WHERE traffic_reset = '3' AND last_traffic_reset_time < ?"
                )
                .bind(now)
                .bind(month_ago)
                .execute(&db)
                .await {
                    tracing::error!("Failed to reset monthly inbound traffic: {}", e);
                }

                // Reset client traffic
                // Client reset: 0=never, 1=daily, 2=weekly, 3=monthly
                if let Err(e) = sqlx::query(
                    "UPDATE client_traffics SET up = 0, down = 0, updated_at = ?
                     WHERE reset = 1 AND updated_at < ?"
                )
                .bind(now)
                .bind(day_ago)
                .execute(&db)
                .await {
                    tracing::error!("Failed to reset daily client traffic: {}", e);
                }

                if let Err(e) = sqlx::query(
                    "UPDATE client_traffics SET up = 0, down = 0, updated_at = ?
                     WHERE reset = 2 AND updated_at < ?"
                )
                .bind(now)
                .bind(week_ago)
                .execute(&db)
                .await {
                    tracing::error!("Failed to reset weekly client traffic: {}", e);
                }

                if let Err(e) = sqlx::query(
                    "UPDATE client_traffics SET up = 0, down = 0, updated_at = ?
                     WHERE reset = 3 AND updated_at < ?"
                )
                .bind(now)
                .bind(month_ago)
                .execute(&db)
                .await {
                    tracing::error!("Failed to reset monthly client traffic: {}", e);
                }

                tracing::info!("Traffic reset job completed");
            });
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Add a job to collect and broadcast system stats
    pub async fn add_system_monitor_job(
        &mut self,
        monitor: Arc<SystemMonitor>,
        ws_hub: Arc<WsHub>,
    ) -> anyhow::Result<()> {
        let job = Job::new_repeated(
            std::time::Duration::from_secs(5),
            move |_uuid, _l| {
                let monitor = monitor.clone();
                let ws_hub = ws_hub.clone();

                tokio::spawn(async move {
                    // Record CPU sample
                    monitor.record_cpu_sample().await;

                    // Get system status
                    let status = monitor.get_status(false).await;

                    // Broadcast system stats
                    ws_hub.broadcast(&serde_json::json!({
                        "type": "system",
                        "data": {
                            "cpu_usage": status.cpu_usage,
                            "cpu_cores": status.cpu_cores,
                            "memory": {
                                "total": status.memory.total,
                                "used": status.memory.used,
                                "usage_percent": status.memory.usage_percent
                            },
                            "uptime": status.uptime,
                            "load_avg": status.load_avg,
                            "timestamp": status.timestamp
                        }
                    })).await;
                });
            },
        )?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Add a job to check client expiry and traffic limits
    pub async fn add_client_check_job(
        &mut self,
        db: SqlitePool,
        notification: Arc<NotificationService>,
    ) -> anyhow::Result<()> {
        // Run every 30 minutes
        let job = Job::new_repeated(
            std::time::Duration::from_secs(1800),
            move |_uuid, _l| {
                let db = db.clone();
                let notification = notification.clone();

                tokio::spawn(async move {
                    let now = chrono::Utc::now().timestamp();

                    // Check for clients reaching traffic limit (80%)
                    #[derive(sqlx::FromRow)]
                    struct ClientLimit {
                        email: String,
                        up: i64,
                        down: i64,
                        total: i64,
                        tag: String,
                    }

                    let clients_near_limit: Vec<ClientLimit> = match sqlx::query_as(
                        "SELECT ct.email, ct.up, ct.down, ct.total, i.tag
                         FROM client_traffics ct
                         JOIN inbounds i ON ct.inbound_id = i.id
                         WHERE ct.total > 0
                         AND ct.enable = 1
                         AND (ct.up + ct.down) >= (ct.total * 8 / 10)
                         AND (ct.up + ct.down) < ct.total"
                    )
                    .fetch_all(&db)
                    .await
                    {
                        Ok(c) => c,
                        Err(_) => return,
                    };

                    for client in clients_near_limit {
                        let _ = notification.notify_traffic_limit(
                            &client.email,
                            client.up + client.down,
                            client.total,
                            &client.tag
                        ).await;
                    }

                    // Check for clients expiring soon (7 days warning)
                    let warning_time = now + (7 * 86400); // 7 days from now

                    #[derive(sqlx::FromRow)]
                    struct ClientExpiry {
                        email: String,
                        expiry_time: i64,
                        tag: String,
                    }

                    let expiring_clients: Vec<ClientExpiry> = match sqlx::query_as(
                        "SELECT ct.email, ct.expiry_time, i.tag
                         FROM client_traffics ct
                         JOIN inbounds i ON ct.inbound_id = i.id
                         WHERE ct.expiry_time > 0
                         AND ct.enable = 1
                         AND ct.expiry_time <= ?
                         AND ct.expiry_time > ?",
                    )
                    .bind(warning_time)
                    .bind(now)
                    .fetch_all(&db)
                    .await
                    {
                        Ok(c) => c,
                        Err(_) => return,
                    };

                    for client in expiring_clients {
                        let days_remaining = ((client.expiry_time - now) / 86400) as i32;
                        let expiry_date = chrono::DateTime::from_timestamp(client.expiry_time, 0)
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_default();

                        let _ = notification.notify_expiry_warning(
                            &client.email,
                            days_remaining,
                            &expiry_date,
                            &client.tag
                        ).await;
                    }

                    // Disable expired clients
                    sqlx::query(
                        "UPDATE client_traffics SET enable = 0
                         WHERE expiry_time > 0
                         AND expiry_time <= ?
                         AND enable = 1"
                    )
                    .bind(now)
                    .execute(&db)
                    .await
                    .ok();

                    tracing::debug!("Client check job completed");
                });
            },
        )?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Add a job for automatic database backup
    pub async fn add_backup_job(
        &mut self,
        db: SqlitePool,
        backup_service: Arc<crate::bot::backup::BackupService>,
    ) -> anyhow::Result<()> {
        // Run daily at 3 AM
        let _db = db; // not needed for file-based backup
        let job = Job::new("0 0 3 * * *", move |_uuid, _l| {
            let backup_service = backup_service.clone();

            tokio::spawn(async move {
                match backup_service.create_backup().await {
                    Ok(info) => {
                        tracing::info!("Automatic backup created: {:?}", info.filename);
                    }
                    Err(e) => {
                        tracing::error!("Automatic backup failed: {}", e);
                    }
                }
            });
        })?;

        self.scheduler.add(job).await?;
        Ok(())
    }

    /// Start the scheduler
    pub async fn start(&self) -> anyhow::Result<()> {
        self.scheduler.start().await?;
        tracing::info!("Scheduler started");
        Ok(())
    }

    /// Stop the scheduler
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        self.scheduler.shutdown().await?;
        tracing::info!("Scheduler stopped");
        Ok(())
    }
}
