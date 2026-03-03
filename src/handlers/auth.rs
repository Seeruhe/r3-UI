use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use sqlx::FromRow;
use tower_sessions::Session;

use crate::models::{LoginRequest, LoginResponse};
use crate::utils::response::ApiResponse;
use crate::AppState;

const SESSION_USER_KEY: &str = "user_id";

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    username: String,
    password: String,
}

/// Login handler
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    // Find user by username
    let user: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password FROM users WHERE username = ?"
    )
    .bind(&req.username)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match user {
        Some(user) => {
            // Verify password
            if verify_password(&req.password, &user.password) {
                // Set session
                session.insert(SESSION_USER_KEY, user.id)
                    .await
                    .map_err(|e| {
                        tracing::error!("Session error: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                tracing::info!("User {} logged in successfully", user.username);

                Ok(Json(ApiResponse::success(LoginResponse {
                    success: true,
                    message: "Login successful".to_string(),
                })))
            } else {
                tracing::warn!("Failed login attempt for user {}", req.username);
                Ok(Json(ApiResponse::success(LoginResponse {
                    success: false,
                    message: "Invalid credentials".to_string(),
                })))
            }
        }
        None => {
            tracing::warn!("Failed login attempt for non-existent user {}", req.username);
            Ok(Json(ApiResponse::success(LoginResponse {
                success: false,
                message: "Invalid credentials".to_string(),
            })))
        }
    }
}

/// Logout handler
pub async fn logout(
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    session.delete()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(())))
}

/// Check if user is logged in
pub async fn is_logged(
    session: Session,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match user_id {
        Some(_) => Ok(Json(ApiResponse::success(true))),
        None => Ok(Json(ApiResponse::success(false))),
    }
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
