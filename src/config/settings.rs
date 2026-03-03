use std::env;
use std::path::PathBuf;

/// Application settings loaded from environment variables
#[derive(Debug, Clone)]
pub struct Settings {
    // Server settings
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub session_secret: String,
    pub web_root: PathBuf,

    // Xray settings
    pub xray_binary: PathBuf,
    pub xray_config: PathBuf,
    pub xray_assets_path: PathBuf,

    // Telegram Bot settings
    pub tg_bot_token: String,
    pub tg_chat_id: i64,
    pub tg_enable: bool,

    // LDAP settings
    pub ldap_enabled: bool,
    pub ldap_url: String,
    pub ldap_bind_dn: String,
    pub ldap_bind_password: String,
    pub ldap_base_dn: String,
    pub ldap_filter: String,

    // Subscription settings
    pub sub_domain: String,
    pub sub_path: String,
    pub sub_encryption_key: String,

    // Backup settings
    pub backup_path: PathBuf,
    pub backup_cron: String,
    pub backup_to_tg: bool,

    // System settings
    pub time_location: String,
    pub web_listen: String,
    pub web_port: u16,

    // Security settings
    pub session_max_age: i64,
    pub rate_limit: u32,
}

impl Settings {
    /// Load settings from environment variables with defaults
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            // Server settings
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "2053".to_string())
                .parse()?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data.db?mode=rwc".to_string()),
            session_secret: env::var("SESSION_SECRET")
                .unwrap_or_else(|_| "default-secret-key-change-me".to_string()),
            web_root: PathBuf::from(
                env::var("WEB_ROOT")
                    .unwrap_or_else(|_| "./web/html".to_string())
            ),

            // Xray settings
            xray_binary: PathBuf::from(
                env::var("XRAY_BINARY")
                    .unwrap_or_else(|_| "/usr/local/bin/xray".to_string())
            ),
            xray_config: PathBuf::from(
                env::var("XRAY_CONFIG")
                    .unwrap_or_else(|_| "/etc/xray/config.json".to_string())
            ),
            xray_assets_path: PathBuf::from(
                env::var("XRAY_ASSETS_PATH")
                    .unwrap_or_else(|_| "/usr/share/xray".to_string())
            ),

            // Telegram Bot settings
            tg_bot_token: env::var("TG_BOT_TOKEN").unwrap_or_default(),
            tg_chat_id: env::var("TG_CHAT_ID")
                .unwrap_or_default()
                .parse()
                .unwrap_or(0),
            tg_enable: env::var("TG_ENABLE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),

            // LDAP settings
            ldap_enabled: env::var("LDAP_ENABLED")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            ldap_url: env::var("LDAP_URL").unwrap_or_default(),
            ldap_bind_dn: env::var("LDAP_BIND_DN").unwrap_or_default(),
            ldap_bind_password: env::var("LDAP_BIND_PASSWORD").unwrap_or_default(),
            ldap_base_dn: env::var("LDAP_BASE_DN").unwrap_or_default(),
            ldap_filter: env::var("LDAP_FILTER")
                .unwrap_or_else(|_| "(uid={0})".to_string()),

            // Subscription settings
            sub_domain: env::var("SUB_DOMAIN").unwrap_or_default(),
            sub_path: env::var("SUB_PATH")
                .unwrap_or_else(|_| "sub".to_string()),
            sub_encryption_key: env::var("SUB_ENCRYPTION_KEY")
                .unwrap_or_else(|_| "default-encryption-key".to_string()),

            // Backup settings
            backup_path: PathBuf::from(
                env::var("BACKUP_PATH")
                    .unwrap_or_else(|_| "./backup".to_string())
            ),
            backup_cron: env::var("BACKUP_CRON")
                .unwrap_or_else(|_| "0 3 * * *".to_string()), // 3 AM daily
            backup_to_tg: env::var("BACKUP_TO_TG")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),

            // System settings
            time_location: env::var("TIME_LOCATION")
                .unwrap_or_else(|_| "Asia/Shanghai".to_string()),
            web_listen: env::var("WEB_LISTEN")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
            web_port: env::var("WEB_PORT")
                .unwrap_or_else(|_| "2053".to_string())
                .parse::<u16>()
                .unwrap_or(2053),

            // Security settings
            session_max_age: env::var("SESSION_MAX_AGE")
                .unwrap_or_else(|_| "86400".to_string())
                .parse()
                .unwrap_or(86400), // 24 hours
            rate_limit: env::var("RATE_LIMIT")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100),
        })
    }

    /// Check if Telegram bot is configured
    pub fn is_telegram_configured(&self) -> bool {
        !self.tg_bot_token.is_empty() && self.tg_chat_id > 0
    }

    /// Check if LDAP is configured
    pub fn is_ldap_configured(&self) -> bool {
        self.ldap_enabled
                    && !self.ldap_url.is_empty()
                    && !self.ldap_bind_dn.is_empty()
                    && !self.ldap_base_dn.is_empty()
    }

    /// Get subscription URL for a given token
    pub fn get_sub_url(&self, token: &str) -> String {
        if self.sub_domain.is_empty() {
            format!("/{}/{}", self.sub_path, token)
        } else {
            format!("https://{}/{}/{}", self.sub_domain, self.sub_path, token)
        }
    }
}
