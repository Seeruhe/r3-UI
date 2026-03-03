use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use sqlx::FromRow;
use tower_sessions::Session;

use crate::utils::response::ApiResponse;
use crate::AppState;

const SESSION_USER_KEY: &str = "user_id";

#[derive(Debug, FromRow)]
struct SettingRow {
    #[allow(dead_code)]
    id: i64,
    key: String,
    value: Option<String>,
}

#[derive(Debug, serde::Serialize, Default)]
pub struct SettingsMap {
    #[serde(flatten)]
    pub settings: std::collections::HashMap<String, String>,
}

/// Get all settings
pub async fn all(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<SettingsMap>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let settings: Vec<SettingRow> = sqlx::query_as::<_, SettingRow>(
        "SELECT id, key, value FROM settings"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch settings: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut map = std::collections::HashMap::new();
    for setting in settings {
        if let Some(value) = setting.value {
            map.insert(setting.key, value);
        }
    }

    Ok(Json(ApiResponse::success(SettingsMap { settings: map })))
}

/// Update a setting
pub async fn update(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<crate::models::UpdateSettingRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    // Upsert setting
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    )
    .bind(&req.key)
    .bind(&req.value)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update setting: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(())))
}

/// Update user credentials
pub async fn update_user(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = match user_id {
        Some(id) => id,
        None => return Ok(Json(ApiResponse::success_msg("Not authenticated"))),
    };

    // Verify old password if provided
    if let Some(ref old_password) = req.old_password {
        let user: Option<(String,)> = sqlx::query_as(
            "SELECT password FROM users WHERE id = ?"
        )
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch user: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if let Some((hash,)) = user {
            if !verify_password(old_password, &hash) {
                return Ok(Json(ApiResponse::success_msg("Invalid current password")));
            }
        }
    }

    // Update username if provided
    if let Some(ref username) = req.username {
        sqlx::query("UPDATE users SET username = ? WHERE id = ?")
            .bind(username)
            .bind(user_id)
            .execute(&state.db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update username: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    // Update password if provided
    if let Some(ref password) = req.password {
        let hash = hash_password(password);
        sqlx::query("UPDATE users SET password = ? WHERE id = ?")
            .bind(&hash)
            .bind(user_id)
            .execute(&state.db)
            .await
            .map_err(|e| {
                tracing::error!("Failed to update password: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(Json(ApiResponse::success(())))
}

/// Restart panel
pub async fn restart_panel(
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    // In a real implementation, this would restart the panel
    // For now, just return success
    tracing::info!("Panel restart requested");

    Ok(Json(ApiResponse::success(())))
}

/// Verify password against hash
fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    use argon2::Argon2;

    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Hash a password
fn hash_password(password: &str) -> String {
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::Argon2;
    use rand::rngs::OsRng;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .unwrap_or_else(|_| password.to_string())
}

/// Request to update user credentials
#[derive(Debug, serde::Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub old_password: Option<String>,
}

// ============================================================================
// Database Backup Handlers
// ============================================================================

use axum::{
    body::Body,
    extract::Path,
    http::header,
    response::Response,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

/// Get list of database backups
pub async fn list_backups(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<BackupInfo>>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let backup_service = state.backup_service.read().await;

    match backup_service.as_ref() {
        Some(service) => {
            let backups = service.list_backups().await.map_err(|e| {
                tracing::error!("Failed to list backups: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            let infos: Vec<BackupInfo> = backups
                .into_iter()
                .map(|b| BackupInfo {
                    filename: b.filename,
                    size: b.size,
                    created_at: b.created_at.to_rfc3339(),
                })
                .collect();

            Ok(Json(ApiResponse::success(infos)))
        }
        None => Ok(Json(ApiResponse::error("Backup service not configured"))),
    }
}

/// Create a database backup
pub async fn create_backup(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<BackupInfo>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let backup_service = state.backup_service.read().await;

    match backup_service.as_ref() {
        Some(service) => {
            let backup = service.create_backup().await.map_err(|e| {
                tracing::error!("Failed to create backup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            Ok(Json(ApiResponse::success(BackupInfo {
                filename: backup.filename,
                size: backup.size,
                created_at: backup.created_at.to_rfc3339(),
            })))
        }
        None => Ok(Json(ApiResponse::error("Backup service not configured"))),
    }
}

/// Download a database backup
pub async fn download_backup(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    session: Session,
) -> Result<Response, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let backup_service = state.backup_service.read().await;

    match backup_service.as_ref() {
        Some(service) => {
            let path = service.get_backup_path(&filename).await.map_err(|e| {
                tracing::error!("Failed to get backup path: {}", e);
                StatusCode::NOT_FOUND
            })?;

            let file = File::open(&path).await.map_err(|e| {
                tracing::error!("Failed to open backup file: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);

            Ok(Response::builder()
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
        }
        None => Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("Backup service not configured"))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?),
    }
}

/// Delete a database backup
pub async fn delete_backup(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let backup_service = state.backup_service.read().await;

    match backup_service.as_ref() {
        Some(service) => {
            service.delete_backup(&filename).await.map_err(|e| {
                tracing::error!("Failed to delete backup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            Ok(Json(ApiResponse::success(())))
        }
        None => Ok(Json(ApiResponse::error("Backup service not configured"))),
    }
}

/// Restore database from backup
pub async fn restore_backup(
    State(state): State<AppState>,
    Path(filename): Path<String>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let backup_service = state.backup_service.read().await;

    match backup_service.as_ref() {
        Some(service) => {
            let path = service.get_backup_path(&filename).await.map_err(|e| {
                tracing::error!("Failed to get backup path: {}", e);
                StatusCode::NOT_FOUND
            })?;

            service.restore_backup(&path).await.map_err(|e| {
                tracing::error!("Failed to restore backup: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            // Note: In a real implementation, you would need to:
            // 1. Close all database connections
            // 2. Replace the database file
            // 3. Reconnect
            // 4. Restart necessary services

            tracing::info!("Database restored from backup: {}", filename);

            Ok(Json(ApiResponse::success(())))
        }
        None => Ok(Json(ApiResponse::error("Backup service not configured"))),
    }
}

/// Backup info for API responses
#[derive(Debug, serde::Serialize, Default)]
pub struct BackupInfo {
    pub filename: String,
    pub size: u64,
    pub created_at: String,
}

// ============================================================================
// Telegram Settings Handlers
// ============================================================================

use serde::{Deserialize, Serialize};

/// Telegram bot configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub bot_token: String,
    pub chat_id: i64,
    pub notify_traffic_limit: bool,
    pub notify_expiry: bool,
    pub notify_login: bool,
}

/// Get Telegram configuration
pub async fn get_telegram_config(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<TelegramConfig>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    // Get config from settings
    let enabled: bool = get_setting(&state.db, "tg_enabled")
        .await
        .and_then(|s| s.parse().ok())
        .unwrap_or(false);

    let bot_token: String = get_setting(&state.db, "tg_bot_token")
        .await
        .unwrap_or_default();

    let chat_id: i64 = get_setting(&state.db, "tg_chat_id")
        .await
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let notify_traffic_limit: bool = get_setting(&state.db, "tg_notify_traffic")
        .await
        .and_then(|s| s.parse().ok())
        .unwrap_or(true);

    let notify_expiry: bool = get_setting(&state.db, "tg_notify_expiry")
        .await
        .and_then(|s| s.parse().ok())
        .unwrap_or(true);

    let notify_login: bool = get_setting(&state.db, "tg_notify_login")
        .await
        .and_then(|s| s.parse().ok())
        .unwrap_or(false);

    Ok(Json(ApiResponse::success(TelegramConfig {
        enabled,
        bot_token,
        chat_id,
        notify_traffic_limit,
        notify_expiry,
        notify_login,
    })))
}

/// Update Telegram configuration
pub async fn update_telegram_config(
    State(state): State<AppState>,
    session: Session,
    Json(config): Json<TelegramConfig>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    // Save to settings
    set_setting(&state.db, "tg_enabled", &config.enabled.to_string())
        .await;
    set_setting(&state.db, "tg_bot_token", &config.bot_token)
        .await;
    set_setting(&state.db, "tg_chat_id", &config.chat_id.to_string())
        .await;
    set_setting(&state.db, "tg_notify_traffic", &config.notify_traffic_limit.to_string())
        .await;
    set_setting(&state.db, "tg_notify_expiry", &config.notify_expiry.to_string())
        .await;
    set_setting(&state.db, "tg_notify_login", &config.notify_login.to_string())
        .await;

    // Update bot if running
    if let Some(bot) = state.telegram_bot.write().await.as_ref() {
        let bot_config = crate::bot::BotConfig {
            enabled: config.enabled,
            token: config.bot_token.clone(),
            chat_id: config.chat_id,
            admin_ids: vec![],
            notify_on_traffic_limit: config.notify_traffic_limit,
            notify_on_expiry: config.notify_expiry,
            notify_on_login: config.notify_login,
        };
        let _ = bot.update_config(bot_config).await;
    }

    Ok(Json(ApiResponse::success(())))
}

/// Helper to get a setting value
async fn get_setting(db: &sqlx::SqlitePool, key: &str) -> Option<String> {
    let result: Result<Option<(String,)>, _> = sqlx::query_as(
        "SELECT value FROM settings WHERE key = ?"
    )
    .bind(key)
    .fetch_optional(db)
    .await;

    match result {
        Ok(Some(row)) => Some(row.0),
        _ => None,
    }
}

/// Helper to set a setting value
async fn set_setting(db: &sqlx::SqlitePool, key: &str, value: &str) {
    let _ = sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await;
}

// ============================================================================
// LDAP Settings Handlers
// ============================================================================

/// LDAP configuration
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LdapConfigResponse {
    pub enabled: bool,
    pub url: String,
    pub bind_dn: String,
    pub base_dn: String,
    pub filter: String,
}

/// Get LDAP configuration
pub async fn get_ldap_config(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<LdapConfigResponse>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    Ok(Json(ApiResponse::success(LdapConfigResponse {
        enabled: get_setting(&state.db, "ldap_enabled")
            .await
            .and_then(|s| s.parse().ok())
            .unwrap_or(false),
        url: get_setting(&state.db, "ldap_url")
            .await
            .unwrap_or_default(),
        bind_dn: get_setting(&state.db, "ldap_bind_dn")
            .await
            .unwrap_or_default(),
        base_dn: get_setting(&state.db, "ldap_base_dn")
            .await
            .unwrap_or_default(),
        filter: get_setting(&state.db, "ldap_filter")
            .await
            .unwrap_or_else(|| "(uid={0})".to_string()),
    })))
}

/// Update LDAP configuration
pub async fn update_ldap_config(
    State(state): State<AppState>,
    session: Session,
    Json(config): Json<LdapConfigResponse>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    set_setting(&state.db, "ldap_enabled", &config.enabled.to_string())
        .await;
    set_setting(&state.db, "ldap_url", &config.url)
        .await;
    set_setting(&state.db, "ldap_bind_dn", &config.bind_dn)
        .await;
    set_setting(&state.db, "ldap_base_dn", &config.base_dn)
        .await;
    set_setting(&state.db, "ldap_filter", &config.filter)
        .await;

    Ok(Json(ApiResponse::success(())))
}

/// Test LDAP connection
pub async fn test_ldap(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success_msg("Not authenticated")));
    }

    let config = crate::services::ldap::LdapConfig {
        enabled: true,
        url: get_setting(&state.db, "ldap_url").await.unwrap_or_default(),
        bind_dn: get_setting(&state.db, "ldap_bind_dn").await.unwrap_or_default(),
        bind_password: get_setting(&state.db, "ldap_bind_password").await.unwrap_or_default(),
        base_dn: get_setting(&state.db, "ldap_base_dn").await.unwrap_or_default(),
        filter: get_setting(&state.db, "ldap_filter").await.unwrap_or_else(|| "(uid={0})".to_string()),
        ..Default::default()
    };

    let service = crate::services::ldap::LdapService::new(config);

    match service.test_connection().await {
        Ok(_) => Ok(Json(ApiResponse::success("Connection successful".to_string()))),
        Err(e) => Ok(Json(ApiResponse::error(&format!("Connection failed: {}", e)))),
    }
}
