//! Telegram bot commands

use serde::{Deserialize, Serialize};

/// Bot commands enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Start,
    Help,
    Status,
    Inbounds,
    Traffic,
    Restart,
    Stop,
    Backup,
    Logs,
    Users,
}

impl Command {
    /// Get command descriptions
    pub fn descriptions() -> String {
        r#"
/start - Start the bot and show welcome message
/help - Show available commands
/status - Show server status
/inbounds - List all inbounds
/traffic - Show traffic statistics
/restart - Restart Xray service
/stop - Stop Xray service
/backup - Create a database backup
/logs - Show recent logs
/users - List active users
"#
        .to_string()
    }
}

/// Handle /start command
pub async fn handle_start(chat_id: i64) -> String {
    format!(
        "🤖 Welcome to r3-UI Bot!\n\nChat ID: {}\n\nUse /help to see available commands.",
        chat_id
    )
}

/// Handle /help command
pub async fn handle_help() -> String {
    Command::descriptions()
}

/// Handle /status command
pub async fn handle_status() -> String {
    // In production, this would get actual system status
    "📊 Server Status\n\nStatus: Running\nXray: Active".to_string()
}

/// Handle /inbounds command
pub async fn handle_inbounds() -> String {
    "📋 Inbounds List\n\nNo inbounds configured.".to_string()
}

/// Handle /traffic command
pub async fn handle_traffic() -> String {
    "📈 Traffic Statistics\n\nToday: 0 GB".to_string()
}

/// Handle /restart command
pub async fn handle_restart() -> String {
    "🔄 Xray restart command received".to_string()
}

/// Handle /stop command
pub async fn handle_stop() -> String {
    "⏹️ Xray stop command received".to_string()
}

/// Handle /backup command
pub async fn handle_backup() -> String {
    "💾 Backup creation initiated".to_string()
}

/// Handle /logs command
pub async fn handle_logs() -> String {
    "📝 Recent Logs\n\nNo logs available.".to_string()
}

/// Handle /users command
pub async fn handle_users() -> String {
    "👥 Active Users\n\nNo active users.".to_string()
}

/// Parse command from text
pub fn parse_command(text: &str) -> Option<Command> {
    let text = text.trim();
    match text {
        "/start" => Some(Command::Start),
        "/help" => Some(Command::Help),
        "/status" => Some(Command::Status),
        "/inbounds" => Some(Command::Inbounds),
        "/traffic" => Some(Command::Traffic),
        "/restart" => Some(Command::Restart),
        "/stop" => Some(Command::Stop),
        "/backup" => Some(Command::Backup),
        "/logs" => Some(Command::Logs),
        "/users" => Some(Command::Users),
        _ => None,
    }
}

/// Execute a command and return the response
pub async fn execute_command(command: Command, chat_id: i64) -> String {
    match command {
        Command::Start => handle_start(chat_id).await,
        Command::Help => handle_help().await,
        Command::Status => handle_status().await,
        Command::Inbounds => handle_inbounds().await,
        Command::Traffic => handle_traffic().await,
        Command::Restart => handle_restart().await,
        Command::Stop => handle_stop().await,
        Command::Backup => handle_backup().await,
        Command::Logs => handle_logs().await,
        Command::Users => handle_users().await,
    }
}
