//! Server status and system monitoring handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tower_sessions::Session;

use crate::services::system::{get_extended_info, SystemStatus, CpuSample};
use crate::utils::response::ApiResponse;
use crate::AppState;

const SESSION_USER_KEY: &str = "user_id";

/// Get current system status
pub async fn get_system_status(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<SystemStatus>>, StatusCode> {
    // Check if user is logged in
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let xray_running = state.xray_process.read().await.is_running();
    let status = state.system_monitor.get_status(xray_running).await;

    Ok(Json(ApiResponse::success(status)))
}

/// Get CPU history
pub async fn get_cpu_history(
    State(state): State<AppState>,
    Path(minutes): Path<i64>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<CpuSample>>>, StatusCode> {
    // Check if user is logged in
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let history = state.system_monitor.get_cpu_history(minutes).await;
    Ok(Json(ApiResponse::success(history)))
}

/// Get CPU history bucketed by interval
pub async fn get_cpu_history_bucketed(
    State(state): State<AppState>,
    Path((bucket_count, duration_minutes)): Path<(usize, i64)>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<CpuSample>>>, StatusCode> {
    // Check if user is logged in
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let history = state.system_monitor.get_cpu_history_bucketed(bucket_count, duration_minutes).await;
    Ok(Json(ApiResponse::success(history)))
}

/// Get extended system information
pub async fn get_system_info(
    session: Session,
) -> Result<Json<ApiResponse<crate::services::system::ExtendedSystemInfo>>, StatusCode> {
    // Check if user is logged in
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let info = get_extended_info().await;
    Ok(Json(ApiResponse::success(info)))
}

/// Get real-time system stats for WebSocket
pub async fn get_realtime_stats(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    // Check if user is logged in
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let xray_running = state.xray_process.read().await.is_running();
    let status = state.system_monitor.get_status(xray_running).await;

    // Get client counts
    #[derive(sqlx::FromRow)]
    struct Counts {
        total_clients: i64,
        active_clients: i64,
        total_inbounds: i64,
        active_inbounds: i64,
    }

    let counts: Counts = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM client_traffics) as total_clients,
            (SELECT COUNT(*) FROM client_traffics WHERE enable = 1) as active_clients,
            (SELECT COUNT(*) FROM inbounds) as total_inbounds,
            (SELECT COUNT(*) FROM inbounds WHERE enable = 1) as active_inbounds"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get traffic summary for today
    #[derive(sqlx::FromRow)]
    struct TrafficSummary {
        today_up: i64,
        today_down: i64,
    }

    let _today_start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    let traffic: TrafficSummary = sqlx::query_as(
        "SELECT COALESCE(SUM(up), 0) as today_up, COALESCE(SUM(down), 0) as today_down
         FROM inbounds"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(serde_json::json!({
        "system": {
            "cpu_usage": status.cpu_usage,
            "cpu_cores": status.cpu_cores,
            "memory": status.memory,
            "uptime": status.uptime,
            "load_avg": status.load_avg,
        },
        "xray": {
            "running": xray_running,
        },
        "clients": {
            "total": counts.total_clients,
            "active": counts.active_clients,
        },
        "inbounds": {
            "total": counts.total_inbounds,
            "active": counts.active_inbounds,
        },
        "traffic": {
            "today_up": traffic.today_up,
            "today_down": traffic.today_down,
            "today_total": traffic.today_up + traffic.today_down,
        },
        "timestamp": status.timestamp,
    }))))
}

/// Refresh system information
pub async fn refresh_system(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // Check if user is logged in
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    state.system_monitor.refresh().await;

    Ok(Json(ApiResponse::success(())))
}
