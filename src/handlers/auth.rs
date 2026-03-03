use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use sqlx::FromRow;
use tower_sessions::Session;

use crate::models::{
    LoginRequest, LoginResponse, Setup2FARequest, Setup2FAResponse,
    VerifyTOTPRequest, ChangePasswordRequest, UpdateUserRequest, UserInfo,
};
use crate::services::totp::{self, generate_qr_code_data_uri, generate_otpauth_url, verify_totp};
use crate::utils::response::ApiResponse;
use crate::AppState;

const SESSION_USER_KEY: &str = "user_id";
const SESSION_2FA_PENDING_KEY: &str = "2fa_pending_user";

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    username: String,
    password: String,
    secret: String,
    tfa_enabled: bool,
    tg_id: i64,
    created_at: i64,
    last_login: i64,
}

/// Login handler with 2FA support
pub async fn login(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    // Find user by username
    let user: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password, secret, tfa_enabled, tg_id, created_at, last_login FROM users WHERE username = ?"
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
                // Check if 2FA is enabled
                if user.tfa_enabled && !user.secret.is_empty() {
                    // Verify TOTP code if provided
                    if let Some(code) = &req.totp_code {
                        if verify_totp(&user.secret, code) {
                            // 2FA verified, complete login
                            complete_login(&session, &user).await?;

                            Ok(Json(ApiResponse::success(LoginResponse {
                                success: true,
                                message: "Login successful".to_string(),
                                requires_2fa: None,
                            })))
                        } else {
                            tracing::warn!("Invalid 2FA code for user {}", req.username);
                            Ok(Json(ApiResponse::success(LoginResponse {
                                success: false,
                                message: "Invalid 2FA code".to_string(),
                                requires_2fa: Some(true),
                            })))
                        }
                    } else {
                        // 2FA required but no code provided
                        // Store pending user in session for 2FA verification
                        session.insert(SESSION_2FA_PENDING_KEY, user.id)
                            .await
                            .map_err(|e| {
                                tracing::error!("Session error: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR
                            })?;

                        Ok(Json(ApiResponse::success(LoginResponse {
                            success: true,
                            message: "2FA verification required".to_string(),
                            requires_2fa: Some(true),
                        })))
                    }
                } else {
                    // No 2FA, complete login directly
                    complete_login(&session, &user).await?;

                    Ok(Json(ApiResponse::success(LoginResponse {
                        success: true,
                        message: "Login successful".to_string(),
                        requires_2fa: None,
                    })))
                }
            } else {
                tracing::warn!("Failed login attempt for user {}", req.username);
                Ok(Json(ApiResponse::success(LoginResponse {
                    success: false,
                    message: "Invalid credentials".to_string(),
                    requires_2fa: None,
                })))
            }
        }
        None => {
            tracing::warn!("Failed login attempt for non-existent user {}", req.username);
            Ok(Json(ApiResponse::success(LoginResponse {
                success: false,
                message: "Invalid credentials".to_string(),
                requires_2fa: None,
            })))
        }
    }
}

/// Complete the login process
async fn complete_login(session: &Session, user: &UserRow) -> Result<(), StatusCode> {
    // Set session
    session.insert(SESSION_USER_KEY, user.id)
        .await
        .map_err(|e| {
            tracing::error!("Session error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Clear any pending 2FA session
    session.remove::<i64>(SESSION_2FA_PENDING_KEY)
        .await
        .map_err(|e| {
            tracing::error!("Session error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!("User {} logged in successfully", user.username);

    Ok(())
}

/// Verify 2FA code for pending login
pub async fn verify_2fa(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<VerifyTOTPRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, StatusCode> {
    // Get pending user from session
    let pending_user_id: Option<i64> = session.get(SESSION_2FA_PENDING_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = match pending_user_id {
        Some(id) => id,
        None => {
            return Ok(Json(ApiResponse::error("No pending 2FA verification")));
        }
    };

    // Find user
    let user: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password, secret, tfa_enabled, tg_id, created_at, last_login FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match user {
        Some(user) if user.tfa_enabled && !user.secret.is_empty() => {
            if verify_totp(&user.secret, &req.code) {
                complete_login(&session, &user).await?;

                Ok(Json(ApiResponse::success(LoginResponse {
                    success: true,
                    message: "Login successful".to_string(),
                    requires_2fa: None,
                })))
            } else {
                Ok(Json(ApiResponse::success(LoginResponse {
                    success: false,
                    message: "Invalid 2FA code".to_string(),
                    requires_2fa: Some(true),
                })))
            }
        }
        _ => Ok(Json(ApiResponse::error("Invalid 2FA state")))
    }
}

/// Setup 2FA - generate secret and QR code
pub async fn setup_2fa(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<Setup2FARequest>,
) -> Result<Json<ApiResponse<Setup2FAResponse>>, StatusCode> {
    // Get current user
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = match user_id {
        Some(id) => id,
        None => return Ok(Json(ApiResponse::error("Not authenticated"))),
    };

    // Find user and verify password
    let user: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password, secret, tfa_enabled, tg_id, created_at, last_login FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let user = match user {
        Some(u) => u,
        None => return Ok(Json(ApiResponse::error("User not found"))),
    };

    // Verify current password
    if !verify_password(&req.password, &user.password) {
        return Ok(Json(ApiResponse::error("Invalid password")));
    }

    if req.enable {
        // Generate new secret
        let secret = totp::generate_secret();
        let otpauth_url = generate_otpauth_url(&secret, &user.username, "r3-UI");

        // Generate QR code
        let qr_code = match generate_qr_code_data_uri(&otpauth_url) {
            Ok(qr) => Some(qr),
            Err(e) => {
                tracing::error!("Failed to generate QR code: {}", e);
                None
            }
        };

        // If TOTP code provided, verify and enable
        if let Some(code) = &req.totp_code {
            if verify_totp(&secret, code) {
                // Update user with new secret and enable 2FA
                sqlx::query(
                    "UPDATE users SET secret = ?, tfa_enabled = ? WHERE id = ?"
                )
                .bind(&secret)
                .bind(true)
                .bind(user_id)
                .execute(&state.db)
                .await
                .map_err(|e| {
                    tracing::error!("Database error: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

                return Ok(Json(ApiResponse::success(Setup2FAResponse {
                    success: true,
                    message: "2FA enabled successfully".to_string(),
                    secret: Some(secret),
                    qr_code,
                    otpauth_url: Some(otpauth_url),
                })));
            } else {
                return Ok(Json(ApiResponse::error("Invalid TOTP code")));
            }
        }

        // Return setup info without enabling yet
        Ok(Json(ApiResponse::success(Setup2FAResponse {
            success: true,
            message: "Scan QR code and verify with TOTP code".to_string(),
            secret: Some(secret.clone()),
            qr_code,
            otpauth_url: Some(otpauth_url),
        })))
    } else {
        // Disable 2FA
        if let Some(code) = &req.totp_code {
            if !user.tfa_enabled || user.secret.is_empty() {
                return Ok(Json(ApiResponse::error("2FA is not enabled")));
            }

            if !verify_totp(&user.secret, code) {
                return Ok(Json(ApiResponse::error("Invalid TOTP code")));
            }

            // Disable 2FA
            sqlx::query(
                "UPDATE users SET secret = '', tfa_enabled = ? WHERE id = ?"
            )
            .bind(false)
            .bind(user_id)
            .execute(&state.db)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            Ok(Json(ApiResponse::success(Setup2FAResponse {
                success: true,
                message: "2FA disabled successfully".to_string(),
                secret: None,
                qr_code: None,
                otpauth_url: None,
            })))
        } else {
            Ok(Json(ApiResponse::error("TOTP code required to disable 2FA")))
        }
    }
}

/// Change password
pub async fn change_password(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // Get current user
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = match user_id {
        Some(id) => id,
        None => return Ok(Json(ApiResponse::error("Not authenticated"))),
    };

    // Find user and verify current password
    let user: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password, secret, tfa_enabled, tg_id, created_at, last_login FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let user = match user {
        Some(u) => u,
        None => return Ok(Json(ApiResponse::error("User not found"))),
    };

    // Verify current password
    if !verify_password(&req.current_password, &user.password) {
        return Ok(Json(ApiResponse::error("Invalid current password")));
    }

    // Hash new password
    let new_hash = hash_password(&req.new_password)?;

    // Update password
    sqlx::query("UPDATE users SET password = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(ApiResponse::success(())))
}

/// Get current user info
pub async fn get_current_user(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<UserInfo>>, StatusCode> {
    // Get current user
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = match user_id {
        Some(id) => id,
        None => return Ok(Json(ApiResponse::error("Not authenticated"))),
    };

    // Find user
    let user: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password, secret, tfa_enabled, tg_id, created_at, last_login FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match user {
        Some(u) => Ok(Json(ApiResponse::success(UserInfo {
            id: u.id,
            username: u.username,
            tfa_enabled: u.tfa_enabled,
            tg_id: u.tg_id,
            created_at: u.created_at,
            last_login: u.last_login,
        }))),
        None => Ok(Json(ApiResponse::error("User not found"))),
    }
}

/// Update user settings
pub async fn update_user(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // Get current user
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = match user_id {
        Some(id) => id,
        None => return Ok(Json(ApiResponse::error("Not authenticated"))),
    };

    // Update user fields
    if let Some(username) = req.username {
        sqlx::query("UPDATE users SET username = ? WHERE id = ?")
            .bind(&username)
            .bind(user_id)
            .execute(&state.db)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    if let Some(tg_id) = req.tg_id {
        sqlx::query("UPDATE users SET tg_id = ? WHERE id = ?")
            .bind(tg_id)
            .bind(user_id)
            .execute(&state.db)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(Json(ApiResponse::success(())))
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

/// Hash a password
fn hash_password(password: &str) -> Result<String, StatusCode> {
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::Argon2;
    use rand::rngs::OsRng;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| {
            tracing::error!("Password hashing error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
