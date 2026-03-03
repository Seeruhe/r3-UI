use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
    response::{Html, IntoResponse},
    http::{header, Response, StatusCode},
    body::Body,
};
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use r3_ui::config::Settings;
use r3_ui::db::init_db;
use r3_ui::handlers::{auth, inbound, setting, xray as xray_handler, pages, subscription};
use r3_ui::services::xray::XrayManager;
use r3_ui::websocket::hub::WsHub;
use r3_ui::websocket::handler as ws_handler;
use r3_ui::AppState;
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

    // Initialize WebSocket hub
    let ws_hub = Arc::new(WsHub::new());

    // Create app state
    let state = AppState {
        db,
        settings: Arc::new(settings),
        xray,
        ws_hub,
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
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(session_layer)
        .with_state(state.clone());

    // Start server
    let addr: SocketAddr = format!("{}:{}",
        state.settings.host,
        state.settings.port
    ).parse()?;

    tracing::info!("Server listening on http://{}", addr);
    tracing::info!("Default credentials: admin / admin");

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
        .unwrap()
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
            .unwrap();
    }

    // Fallback to index.html for SPA routing
    let index = include_str!("../web/html/index.html");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(index))
        .unwrap()
}
