use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::Datelike;

/// Traffic record for tracking usage over time
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Traffic {
    pub id: i64,
    pub inbound_id: i64,
    pub up: i64,
    pub down: i64,
    #[sqlx(default)]
    pub recorded_at: i64,
}

/// Traffic statistics summary
#[derive(Debug, Serialize)]
pub struct TrafficStats {
    pub inbound_id: i64,
    pub tag: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    /// Usage percentage (0-100)
    pub usage_percent: Option<f32>,
    /// Remaining traffic
    pub remaining: Option<i64>,
}

/// Traffic update from Xray
#[derive(Debug, Deserialize)]
pub struct TrafficUpdate {
    pub tag: String,
    pub up: i64,
    pub down: i64,
}

/// Client traffic update
#[derive(Debug, Deserialize)]
pub struct ClientTrafficUpdate {
    pub email: String,
    pub up: i64,
    pub down: i64,
}

/// Traffic history entry
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TrafficHistory {
    pub id: i64,
    pub inbound_id: i64,
    pub client_id: Option<i64>,
    pub up: i64,
    pub down: i64,
    pub recorded_at: i64,
}

/// Traffic history query parameters
#[derive(Debug, Deserialize)]
pub struct TrafficHistoryQuery {
    pub inbound_id: Option<i64>,
    pub client_id: Option<i64>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub interval: Option<String>, // "hour", "day", "week", "month"
}

/// Traffic summary for a period
#[derive(Debug, Serialize)]
pub struct TrafficSummary {
    pub total_up: i64,
    pub total_down: i64,
    pub total: i64,
    pub period_start: i64,
    pub period_end: i64,
    pub inbounds: Vec<InboundTrafficSummary>,
}

/// Per-inbound traffic summary
#[derive(Debug, Serialize)]
pub struct InboundTrafficSummary {
    pub inbound_id: i64,
    pub tag: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    pub client_count: i64,
}

/// Traffic reset configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficResetConfig {
    /// Reset interval type: 0 = never, 1 = daily, 2 = weekly, 3 = monthly
    pub reset_type: i32,
    /// Day of week for weekly reset (0-6, Sunday = 0)
    pub reset_day: i32,
    /// Day of month for monthly reset (1-31)
    pub reset_day_of_month: i32,
    /// Hour of day for reset (0-23)
    pub reset_hour: i32,
    /// Last reset timestamp
    pub last_reset: i64,
    /// Next reset timestamp
    pub next_reset: i64,
}

impl Default for TrafficResetConfig {
    fn default() -> Self {
        Self {
            reset_type: 0, // Never
            reset_day: 0,
            reset_day_of_month: 1,
            reset_hour: 0,
            last_reset: 0,
            next_reset: 0,
        }
    }
}

impl TrafficResetConfig {
    /// Calculate the next reset timestamp
    pub fn calculate_next_reset(&self) -> i64 {
        if self.reset_type == 0 {
            return 0; // Never
        }

        let now = chrono::Utc::now();
        let hour = (self.reset_hour as u32).min(23);

        let next = match self.reset_type {
            1 => { // Daily
                let today_reset = now.date_naive()
                    .and_hms_opt(hour, 0, 0)
                    .unwrap_or_else(|| now.naive_utc())
                    .and_utc();
                if today_reset <= now {
                    // Already past today's reset time, advance to tomorrow
                    today_reset + chrono::Duration::days(1)
                } else {
                    today_reset
                }
            }
            2 => { // Weekly
                let days_until = (self.reset_day as i64 - now.weekday().num_days_from_sunday() as i64 + 7) % 7;
                let candidate = now.date_naive()
                    .and_hms_opt(hour, 0, 0)
                    .unwrap_or_else(|| now.naive_utc())
                    .and_utc()
                    + chrono::Duration::days(days_until);
                if candidate <= now {
                    // Same day but already past the hour, advance to next week
                    candidate + chrono::Duration::weeks(1)
                } else {
                    candidate
                }
            }
            3 => { // Monthly
                let day = (self.reset_day_of_month as u32).min(28).max(1);
                let this_month = now.date_naive()
                    .with_day(day)
                    .unwrap_or(now.date_naive())
                    .and_hms_opt(hour, 0, 0)
                    .unwrap_or_else(|| now.naive_utc())
                    .and_utc();
                if this_month <= now {
                    // Already past this month's reset, advance to next month
                    let next_month = if now.month() == 12 {
                        chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, day)
                    } else {
                        chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, day)
                    };
                    next_month
                        .unwrap_or(now.date_naive())
                        .and_hms_opt(hour, 0, 0)
                        .unwrap_or_else(|| now.naive_utc())
                        .and_utc()
                } else {
                    this_month
                }
            }
            _ => return 0,
        };

        next.timestamp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traffic_reset_config_default() {
        let config = TrafficResetConfig::default();
        assert_eq!(config.reset_type, 0);
    }
}
