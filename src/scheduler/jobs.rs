//! Scheduled jobs for traffic collection and maintenance
//!
//! This module provides cron-scheduled tasks for the application.

use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use sqlx::SqlitePool;

use crate::websocket::hub::WsHub;

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
                        Err(_) => return,
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
        // Run daily at midnight
        let job = Job::new("0 0 0 * * *", move |_uuid, _l| {
            let db = db.clone();
            tokio::spawn(async move {
                // Reset daily traffic
                let now = chrono::Utc::now().timestamp();
                let day_ago = now - 86400;

                sqlx::query(
                    "UPDATE inbounds SET up = 0, down = 0, last_traffic_reset_time = ?
                     WHERE traffic_reset = 'daily' AND last_traffic_reset_time < ?"
                )
                .bind(now)
                .bind(day_ago)
                .execute(&db)
                .await
                .ok();

                // Reset weekly traffic
                let week_ago = now - 604800;
                sqlx::query(
                    "UPDATE inbounds SET up = 0, down = 0, last_traffic_reset_time = ?
                     WHERE traffic_reset = 'weekly' AND last_traffic_reset_time < ?"
                )
                .bind(now)
                .bind(week_ago)
                .execute(&db)
                .await
                .ok();

                // Reset monthly traffic
                let month_ago = now - 2592000;
                sqlx::query(
                    "UPDATE inbounds SET up = 0, down = 0, last_traffic_reset_time = ?
                     WHERE traffic_reset = 'monthly' AND last_traffic_reset_time < ?"
                )
                .bind(now)
                .bind(month_ago)
                .execute(&db)
                .await
                .ok();
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
