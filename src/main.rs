use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
    response::IntoResponse,
    http::{header, Response, StatusCode},
    body::Body,
};
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::sync::RwLock;

use r3_ui::config::Settings;
use r3_ui::db::init_db;
use r3_ui::handlers::{auth, inbound, setting, xray as xray_handler, pages, subscription};
use r3_ui::services::xray::XrayManager;
use r3_ui::services::system::SystemMonitor;
use r3_ui::websocket::hub::WsHub;
use r3_ui::websocket::handler as ws_handler;
use r3_ui::bot::{TelegramBot, BotConfig, NotificationService};
use r3_ui::bot::backup::BackupService;
use r3_ui::AppState;
use r3_ui::XrayProcessState;
use r3_ui::services::i18n::init_i18n;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "r3_ui=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting r3-UI server...");

    // Load configuration
    let settings = Settings::load()?;
    tracing::info!("Configuration loaded");

    // Initialize database
    let db = init_db(&settings.database_url).await?;
    tracing::info!("Database initialized");

    // Initialize i18n
    init_i18n().await?;
    tracing::info!("i18n initialized");

    // Initialize Xray manager
    let xray = Arc::new(XrayManager::new(settings.xray_binary.clone(), settings.xray_config.clone()));
    tracing::info!("Xray manager initialized");

    // Initialize system monitor
    let system_monitor = Arc::new(SystemMonitor::new());
    tracing::info!("System monitor initialized");

    // Initialize WebSocket hub
    let ws_hub = Arc::new(WsHub::new());
    tracing::info!("WebSocket hub initialized");

    // Initialize notification service
    let notification_service = Arc::new(NotificationService::new());
    tracing::info!("Notification service initialized");

    // Initialize Telegram bot if configured
    let telegram_bot: Arc<RwLock<Option<TelegramBot>>> = if settings.tg_enable && !settings.tg_bot_token.is_empty() {
        let bot_config = BotConfig {
            enabled: true,
            token: settings.tg_bot_token.clone(),
            chat_id: settings.tg_chat_id,
            admin_ids: vec![],
            notify_on_traffic_limit: true,
            notify_on_expiry: true,
            notify_on_login: false,
        };
        Arc::new(RwLock::new(Some(TelegramBot::new(bot_config))))
    } else {
        Arc::new(RwLock::new(None))
    };
    tracing::info!("Telegram bot configured: {}", telegram_bot.read().await.is_some());

    // Initialize backup service
    let backup_service: Arc<RwLock<Option<BackupService>>> = Arc::new(RwLock::new(Some(BackupService::new(
        r3_ui::bot::backup::BackupConfig {
            enabled: settings.backup_to_tg,
            backup_path: settings.backup_path.clone(),
            cron_schedule: settings.backup_cron.clone(),
            keep_count: 7,
            send_to_telegram: settings.backup_to_tg,
        },
        PathBuf::from(settings.database_url.clone().strip_prefix("sqlite:").unwrap_or(&settings.database_url)),
    ))));
    tracing::info!("Backup service initialized");

    // Create app state
    let state = AppState {
        db,
        settings: Arc::new(settings),
        xray,
        system_monitor,
        ws_hub,
        xray_process: Arc::new(RwLock::new(XrayProcessState::default())),
        telegram_bot,
        notification_service,
        backup_service,
    };

    // Setup session store
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(24)));

    // Build API routes
    let api_routes = Router::new()
        // Auth routes
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/is_logged", get(auth::is_logged))
        .route("/getTwoFactorEnable", post(pages::get_two_factor_enable))
        // Inbound routes (protected)
        .route("/panel/api/inbounds", get(inbound::list).post(inbound::create))
        .route("/panel/api/inbounds/update", post(inbound::update))
        .route("/panel/api/inbounds/del/{id}", post(inbound::delete))
        .route("/panel/api/inbounds/traffic", get(inbound::traffic))
        .route("/panel/api/inbounds/list", get(inbound::list))
        .route("/panel/api/inbounds/get/{id}", get(inbound::get))
        .route("/panel/api/inbounds/addClient", post(inbound::add_client))
        .route("/panel/api/inbounds/updateClient/{id}", post(inbound::update_client))
        .route("/panel/api/inbounds/{id}/delClient/{client_id}", post(inbound::del_client))
        .route("/panel/api/inbounds/resetAllTraffics", post(inbound::reset_all_traffic))
        // Xray routes
        .route("/panel/api/server/status", get(xray_handler::status))
        .route("/panel/api/server/restartXrayService", post(xray_handler::restart))
        .route("/panel/api/server/stopXrayService", post(xray_handler::stop))
        .route("/panel/api/server/getXrayVersion", get(xray_handler::get_version))
        .route("/panel/api/server/getConfigJson", get(xray_handler::get_config_json))
        .route("/panel/api/server/logs/{count}", post(xray_handler::logs_count))
        .route("/panel/api/server/xraylogs/{count}", post(xray_handler::xray_logs_count))
        // Settings routes
        .route("/panel/api/setting/all", get(setting::all))
        .route("/panel/api/setting/update", post(setting::update))
        .route("/panel/api/setting/updateUser", post(setting::update_user))
        .route("/panel/api/setting/restartPanel", post(setting::restart_panel))
        // Xray config routes
        .route("/panel/api/xray/", get(xray_handler::get_config))
        .route("/panel/api/xray/update", post(xray_handler::update_config))
        .route("/panel/api/xray/getDefaultJsonConfig", get(xray_handler::get_default_config))
        // Subscription routes
        .route("/panel/api/sub/{token}", get(subscription::get_sub))
        .route("/panel/api/sub/json/{token}", get(subscription::get_sub_json));

    // Build panel page routes
    let panel_routes = Router::new()
        .route("/", get(pages::panel_index))
        .route("/inbounds", get(pages::inbounds_page))
        .route("/settings", get(pages::settings_page))
        .route("/xray", get(pages::xray_page));

    // Build main router
    let app = Router::new()
        // WebSocket
        .route("/ws", get(ws_handler::ws_handler))
        // API routes
        .nest("/api", api_routes)
        // Logout
        .route("/logout/", get(handle_logout))
        // Panel pages
        .nest("/panel", panel_routes)
        // Root page
        .route("/", get(pages::login_page))
        .route("/index.html", get(pages::login_page))
        // Static files - serve assets
        .fallback(serve_static)
        .layer({
            use tower_http::cors::{Any, CorsLayer};
            use axum::http::Method;
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
                .allow_headers(Any)
                .allow_origin(Any) // TODO: restrict to specific origins in production
        })
        .layer(TraceLayer::new_for_http())
        .layer(session_layer)
        .with_state(state.clone());

    // Start server
    let addr: SocketAddr = format!("{}:{}",
        state.settings.host,
        state.settings.port
    ).parse()?;

    tracing::info!("Server listening on http://{}", addr);
    tracing::warn!("If this is a fresh install, change the default admin password immediately");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Handle logout redirect
async fn handle_logout() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, "/")
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// Serve static files from web/assets
async fn serve_static(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "web/assets"]
    struct Assets;

    // Try to get the file from embedded assets
    if let Some(file) = Assets::get(&path) {
        let mime = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .as_ref()
            .to_string();

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .body(Body::from(file.data))
            .unwrap_or_else(|_| Response::new(Body::empty()));
    }

    // Fallback to index.html for SPA routing
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(include_bytes!("../web/html/index.html").as_slice()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
