use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tower_sessions::Session;

use crate::services::xray::XrayStatus;
use crate::utils::response::ApiResponse;
use crate::AppState;

const SESSION_USER_KEY: &str = "user_id";

/// Get Xray status
pub async fn status(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<XrayStatus>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let status = state.xray.status().await;
    Ok(Json(ApiResponse::success(status)))
}

/// Restart Xray
pub async fn restart(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    if let Err(e) = state.xray.restart().await {
        tracing::error!("Failed to restart Xray: {}", e);
        return Ok(Json(ApiResponse::success_msg("Failed to restart Xray")));
    }

    Ok(Json(ApiResponse::success(())))
}

/// Stop Xray
pub async fn stop(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    if let Err(e) = state.xray.stop().await {
        tracing::error!("Failed to stop Xray: {}", e);
        return Ok(Json(ApiResponse::success_msg("Failed to stop Xray")));
    }

    Ok(Json(ApiResponse::success(())))
}

/// Get Xray logs
pub async fn logs(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let logs = state.xray.get_logs().await.unwrap_or_default();
    Ok(Json(ApiResponse::success(logs)))
}

/// Get Xray logs with count limit
pub async fn logs_count(
    State(state): State<AppState>,
    Path(count): Path<usize>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let logs = state.xray.get_logs_count(count).await.unwrap_or_default();
    Ok(Json(ApiResponse::success(logs)))
}

/// Get Xray internal logs with count limit
pub async fn xray_logs_count(
    State(state): State<AppState>,
    Path(count): Path<usize>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let logs = state.xray.get_xray_logs(count).await.unwrap_or_default();
    Ok(Json(ApiResponse::success(logs)))
}

/// Get Xray version
pub async fn get_version(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let version = state.xray.get_version().await.unwrap_or_else(|_| "Unknown".to_string());
    Ok(Json(ApiResponse::success(version)))
}

/// Get Xray config JSON
pub async fn get_config_json(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success(serde_json::json!({}))));
    }

    let config = state.xray.get_config().await.unwrap_or_else(|_| serde_json::json!({}));
    Ok(Json(ApiResponse::success(config)))
}

/// Get Xray config
pub async fn get_config(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success(serde_json::json!({}))));
    }

    let config = state.xray.get_config().await.unwrap_or_else(|_| serde_json::json!({}));
    Ok(Json(ApiResponse::success(config)))
}

/// Update Xray config
pub async fn update_config(
    State(state): State<AppState>,
    session: Session,
    Json(config): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    if let Err(e) = state.xray.update_config(config).await {
        tracing::error!("Failed to update Xray config: {}", e);
        return Ok(Json(ApiResponse::success_msg("Failed to update config")));
    }

    Ok(Json(ApiResponse::success(())))
}

/// Get default Xray config
pub async fn get_default_config(
    State(_state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success(serde_json::json!({}))));
    }

    // Return default Xray config
    let default_config = serde_json::json!({
        "log": {
            "loglevel": "warning"
        },
        "inbounds": [],
        "outbounds": [
            {
                "tag": "direct",
                "protocol": "freedom",
                "settings": {}
            }
        ]
    });

    Ok(Json(ApiResponse::success(default_config)))
}
