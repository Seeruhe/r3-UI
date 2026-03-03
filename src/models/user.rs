use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// User model for authentication
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    /// TOTP secret for 2FA (base32 encoded)
    #[serde(skip_serializing)]
    #[sqlx(default)]
    pub secret: String,
    /// Whether 2FA is enabled for this user
    #[sqlx(default)]
    pub tfa_enabled: bool,
    /// User's Telegram ID for notifications
    #[sqlx(default)]
    pub tg_id: i64,
    /// Account creation timestamp
    #[sqlx(default)]
    pub created_at: i64,
    /// Last login timestamp
    #[sqlx(default)]
    pub last_login: i64,
}

/// Login request payload
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// TOTP code if 2FA is enabled
    #[serde(default)]
    pub totp_code: Option<String>,
}

/// Login response
#[derive(Debug, Serialize, Default)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    /// Indicates if 2FA verification is needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_2fa: Option<bool>,
}

/// Request to enable/disable 2FA
#[derive(Debug, Deserialize)]
pub struct Setup2FARequest {
    pub enable: bool,
    /// Current password for verification
    pub password: String,
    /// TOTP code to verify setup
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp_code: Option<String>,
}

/// 2FA setup response with QR code
#[derive(Debug, Serialize, Default)]
pub struct Setup2FAResponse {
    pub success: bool,
    pub message: String,
    /// Base32 encoded secret for manual entry
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// QR code as base64 encoded PNG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qr_code: Option<String>,
    /// OTP auth URL for QR generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otpauth_url: Option<String>,
}

/// Request to verify TOTP code
#[derive(Debug, Deserialize)]
pub struct VerifyTOTPRequest {
    pub code: String,
}

/// Password change request
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// User update request
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub tg_id: Option<i64>,
}

/// User info for display (without sensitive data)
#[derive(Debug, Serialize, Default)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    pub tfa_enabled: bool,
    pub tg_id: i64,
    pub created_at: i64,
    pub last_login: i64,
}

impl From<User> for UserInfo {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            tfa_enabled: user.tfa_enabled,
            tg_id: user.tg_id,
            created_at: user.created_at,
            last_login: user.last_login,
        }
    }
}
