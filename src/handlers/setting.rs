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
