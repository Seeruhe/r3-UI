use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub session_secret: String,
    pub xray_binary: PathBuf,
    pub xray_config: PathBuf,
    pub web_root: PathBuf,
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "2053".to_string())
                .parse()?,
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data.db?mode=rwc".to_string()),
            session_secret: env::var("SESSION_SECRET")
                .unwrap_or_else(|_| "default-secret-key-change-me".to_string()),
            xray_binary: PathBuf::from(
                env::var("XRAY_BINARY")
                    .unwrap_or_else(|_| "/usr/local/bin/xray".to_string())
            ),
            xray_config: PathBuf::from(
                env::var("XRAY_CONFIG")
                    .unwrap_or_else(|_| "/etc/xray/config.json".to_string())
            ),
            web_root: PathBuf::from(
                env::var("WEB_ROOT")
                    .unwrap_or_else(|_| "./web/html".to_string())
            ),
        })
    }
}
