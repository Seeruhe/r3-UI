use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::FromRow;
use tower_sessions::Session;

use crate::models::{CreateInboundRequest, UpdateInboundRequest};
use crate::utils::response::ApiResponse;
use crate::AppState;

const SESSION_USER_KEY: &str = "user_id";

#[derive(Debug, FromRow, serde::Serialize)]
pub struct Inbound {
    pub id: i64,
    pub user_id: i64,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    pub all_time: i64,
    pub remark: Option<String>,
    pub enable: bool,
    pub expiry_time: i64,
    pub traffic_reset: String,
    pub last_traffic_reset_time: i64,
    pub listen: Option<String>,
    pub port: i32,
    pub protocol: String,
    pub settings: Option<String>,
    pub stream_settings: Option<String>,
    pub tag: String,
    pub sniffing: Option<String>,
}

#[derive(Debug, FromRow, serde::Serialize)]
pub struct InboundStats {
    pub id: i64,
    pub tag: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
}

/// List all inbounds
pub async fn list(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<Inbound>>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let inbounds: Vec<Inbound> = sqlx::query_as::<_, Inbound>(
        "SELECT id, user_id, up, down, total, all_time, remark, enable,
                expiry_time, traffic_reset, last_traffic_reset_time, listen,
                port, protocol, settings, stream_settings, tag, sniffing
         FROM inbounds ORDER BY id"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch inbounds: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(inbounds)))
}

/// Create a new inbound
pub async fn create(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<CreateInboundRequest>,
) -> Result<Json<ApiResponse<i64>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = match user_id {
        Some(id) => id,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    // Generate tag
    let tag = format!("{}-{}", req.protocol, req.port);

    // Insert inbound
    let result = sqlx::query(
        "INSERT INTO inbounds (
            user_id, remark, listen, port, protocol, settings,
            stream_settings, total, expiry_time, enable, sniffing,
            traffic_reset, tag, up, down, all_time, last_traffic_reset_time
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 0, 0)"
    )
    .bind(user_id)
    .bind(&req.remark)
    .bind(&req.listen)
    .bind(req.port)
    .bind(&req.protocol)
    .bind(&req.settings)
    .bind(&req.stream_settings)
    .bind(req.total.unwrap_or(0))
    .bind(req.expiry_time.unwrap_or(0))
    .bind(req.enable.unwrap_or(true) as i32)
    .bind(&req.sniffing)
    .bind(req.traffic_reset.unwrap_or_else(|| "never".to_string()))
    .bind(&tag)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create inbound: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let id = result.last_insert_rowid();

    // Regenerate Xray config and restart
    if let Err(e) = regenerate_xray_config(&state).await {
        tracing::error!("Failed to regenerate Xray config: {}", e);
    }

    Ok(Json(ApiResponse::success(id)))
}

/// Update an existing inbound
pub async fn update(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<UpdateInboundRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Build update query dynamically
    let mut updates = Vec::new();
    if req.remark.is_some() {
        updates.push("remark = ?");
    }
    if req.listen.is_some() {
        updates.push("listen = ?");
    }
    if req.port.is_some() {
        updates.push("port = ?");
    }
    if req.protocol.is_some() {
        updates.push("protocol = ?");
    }
    if req.settings.is_some() {
        updates.push("settings = ?");
    }
    if req.stream_settings.is_some() {
        updates.push("stream_settings = ?");
    }
    if req.total.is_some() {
        updates.push("total = ?");
    }
    if req.expiry_time.is_some() {
        updates.push("expiry_time = ?");
    }
    if req.enable.is_some() {
        updates.push("enable = ?");
    }
    if req.sniffing.is_some() {
        updates.push("sniffing = ?");
    }
    if req.traffic_reset.is_some() {
        updates.push("traffic_reset = ?");
    }

    if updates.is_empty() {
        return Ok(Json(ApiResponse::success_msg("No fields to update")));
    }

    let sql = format!(
        "UPDATE inbounds SET {} WHERE id = ?",
        updates.join(", ")
    );

    let mut query = sqlx::query(&sql);
    if let Some(ref v) = req.remark {
        query = query.bind(v);
    }
    if let Some(ref v) = req.listen {
        query = query.bind(v);
    }
    if let Some(v) = req.port {
        query = query.bind(v);
    }
    if let Some(ref v) = req.protocol {
        query = query.bind(v);
    }
    if let Some(ref v) = req.settings {
        query = query.bind(v);
    }
    if let Some(ref v) = req.stream_settings {
        query = query.bind(v);
    }
    if let Some(v) = req.total {
        query = query.bind(v);
    }
    if let Some(v) = req.expiry_time {
        query = query.bind(v);
    }
    if let Some(v) = req.enable {
        query = query.bind(v as i32);
    }
    if let Some(ref v) = req.sniffing {
        query = query.bind(v);
    }
    if let Some(ref v) = req.traffic_reset {
        query = query.bind(v);
    }
    query = query.bind(req.id);

    query.execute(&state.db).await.map_err(|e| {
        tracing::error!("Failed to update inbound: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Regenerate Xray config
    if let Err(e) = regenerate_xray_config(&state).await {
        tracing::error!("Failed to regenerate Xray config: {}", e);
    }

    Ok(Json(ApiResponse::success(())))
}

/// Delete an inbound
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Delete inbound
    sqlx::query("DELETE FROM inbounds WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete inbound: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Delete associated client traffic records
    if let Err(e) = sqlx::query("DELETE FROM client_traffics WHERE inbound_id = ?")
        .bind(id)
        .execute(&state.db)
        .await {
        tracing::error!("Failed to delete client traffic records: {}", e);
    }

    // Regenerate Xray config
    if let Err(e) = regenerate_xray_config(&state).await {
        tracing::error!("Failed to regenerate Xray config: {}", e);
    }

    Ok(Json(ApiResponse::success(())))
}

/// Get traffic statistics
pub async fn traffic(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<InboundStats>>>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let stats: Vec<InboundStats> = sqlx::query_as::<_, InboundStats>(
        "SELECT id, tag, up, down, total FROM inbounds ORDER BY id"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch traffic stats: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(stats)))
}

/// Regenerate Xray configuration
async fn regenerate_xray_config(state: &AppState) -> anyhow::Result<()> {
    let inbounds: Vec<Inbound> = sqlx::query_as::<_, Inbound>(
        "SELECT id, user_id, up, down, total, all_time, remark, enable,
                expiry_time, traffic_reset, last_traffic_reset_time, listen,
                port, protocol, settings, stream_settings, tag, sniffing
         FROM inbounds WHERE enable = 1"
    )
    .fetch_all(&state.db)
    .await?;

    // Convert to service inbound format
    let service_inbounds: Vec<crate::services::xray::InboundConfig> = inbounds
        .iter()
        .map(|i| crate::services::xray::InboundConfig {
            tag: i.tag.clone(),
            listen: i.listen.clone().unwrap_or_else(|| "0.0.0.0".to_string()),
            port: i.port,
            protocol: i.protocol.clone(),
            settings: i.settings.as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({})),
            stream_settings: i.stream_settings.as_ref()
                .and_then(|s| serde_json::from_str(s).ok()),
            sniffing: crate::services::xray::SniffingConfig {
                enabled: true,
                dest_override: vec!["http".to_string(), "tls".to_string()],
            },
        })
        .collect();

    state.xray.generate_config(&service_inbounds).await?;
    state.xray.restart().await?;

    Ok(())
}

/// Get a single inbound by ID
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    session: Session,
) -> Result<Json<ApiResponse<Option<Inbound>>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Json(ApiResponse::success(None)));
    }

    let inbound: Option<Inbound> = sqlx::query_as::<_, Inbound>(
        "SELECT id, user_id, up, down, total, all_time, remark, enable,
                expiry_time, traffic_reset, last_traffic_reset_time, listen,
                port, protocol, settings, stream_settings, tag, sniffing
         FROM inbounds WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch inbound: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(inbound)))
}

/// Add a client to an inbound
pub async fn add_client(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<AddClientRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Insert client into client_traffics table
    sqlx::query(
        "INSERT INTO client_traffics (
            inbound_id, email, up, down, total, enable, expiry_time,
            reset, last_reset_time
        ) VALUES (?, ?, 0, 0, ?, 1, ?, 'never', 0)"
    )
    .bind(req.inbound_id)
    .bind(&req.email)
    .bind(req.total.unwrap_or(0))
    .bind(req.expiry_time.unwrap_or(0))
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to add client: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Regenerate Xray config
    if let Err(e) = regenerate_xray_config(&state).await {
        tracing::error!("Failed to regenerate Xray config: {}", e);
    }

    Ok(Json(ApiResponse::success(())))
}

/// Update a client
pub async fn update_client(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    session: Session,
    Json(req): Json<UpdateClientRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    sqlx::query(
        "UPDATE client_traffics SET email = ?, total = ?, expiry_time = ?, enable = ? WHERE id = ?"
    )
    .bind(&req.email)
    .bind(req.total)
    .bind(req.expiry_time)
    .bind(req.enable.unwrap_or(true) as i32)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update client: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(())))
}

/// Delete a client
pub async fn del_client(
    State(state): State<AppState>,
    Path((id, client_id)): Path<(i64, i64)>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    sqlx::query("DELETE FROM client_traffics WHERE id = ? AND inbound_id = ?")
        .bind(client_id)
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete client: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Regenerate Xray config
    if let Err(e) = regenerate_xray_config(&state).await {
        tracing::error!("Failed to regenerate Xray config: {}", e);
    }

    Ok(Json(ApiResponse::success(())))
}

/// Reset all traffic
pub async fn reset_all_traffic(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    sqlx::query("UPDATE inbounds SET up = 0, down = 0")
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to reset traffic: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Err(e) = sqlx::query("UPDATE client_traffics SET up = 0, down = 0")
        .execute(&state.db)
        .await {
        tracing::error!("Failed to reset client traffic: {}", e);
    }

    Ok(Json(ApiResponse::success(())))
}

/// Request to add a client
#[derive(Debug, serde::Deserialize)]
pub struct AddClientRequest {
    pub inbound_id: i64,
    pub email: String,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
}

/// Request to update a client
#[derive(Debug, serde::Deserialize)]
pub struct UpdateClientRequest {
    pub email: String,
    pub total: i64,
    pub expiry_time: i64,
    pub enable: Option<bool>,
}

/// Request to import multiple clients
#[derive(Debug, serde::Deserialize)]
pub struct ImportClientsRequest {
    pub inbound_id: i64,
    pub clients: Vec<ClientImport>,
}

/// Single client for import
#[derive(Debug, serde::Deserialize)]
pub struct ClientImport {
    pub email: String,
    pub total: Option<i64>,
    pub expiry_time: Option<i64>,
    pub enable: Option<bool>,
    pub limit_ip: Option<i32>,
    pub tg_id: Option<i64>,
    pub sub_id: Option<String>,
    pub comment: Option<String>,
    pub reset: Option<i32>,
}

/// Reset client traffic by email
pub async fn reset_client_traffic(
    State(state): State<AppState>,
    Path((id, email)): Path<(i64, String)>,
    session: Session,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let result = sqlx::query(
        "UPDATE client_traffics SET up = 0, down = 0, updated_at = ? WHERE inbound_id = ? AND email = ?"
    )
    .bind(chrono::Utc::now().timestamp())
    .bind(id)
    .bind(&email)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to reset client traffic: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        return Ok(Json(ApiResponse::error("Client not found")));
    }

    Ok(Json(ApiResponse::success(())))
}

/// Delete depleted (traffic exhausted) clients
pub async fn del_depleted_clients(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    session: Session,
) -> Result<Json<ApiResponse<usize>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Delete clients where traffic is depleted (up + down >= total) and total > 0
    let result = sqlx::query(
        "DELETE FROM client_traffics
         WHERE inbound_id = ?
         AND total > 0
         AND (up + down) >= total"
    )
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete depleted clients: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let deleted = result.rows_affected() as usize;

    // Regenerate Xray config if clients were deleted
    if deleted > 0 {
        if let Err(e) = regenerate_xray_config(&state).await {
            tracing::error!("Failed to regenerate Xray config: {}", e);
        }
    }

    Ok(Json(ApiResponse::success(deleted)))
}

/// Delete expired clients
pub async fn del_expired_clients(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    session: Session,
) -> Result<Json<ApiResponse<usize>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let now = chrono::Utc::now().timestamp();

    // Delete expired clients (expiry_time > 0 AND expiry_time <= now)
    let result = sqlx::query(
        "DELETE FROM client_traffics
         WHERE inbound_id = ?
         AND expiry_time > 0
         AND expiry_time <= ?"
    )
    .bind(id)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete expired clients: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let deleted = result.rows_affected() as usize;

    if deleted > 0 {
        if let Err(e) = regenerate_xray_config(&state).await {
            tracing::error!("Failed to regenerate Xray config: {}", e);
        }
    }

    Ok(Json(ApiResponse::success(deleted)))
}

/// Import clients from JSON
pub async fn import_clients(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<ImportClientsRequest>,
) -> Result<Json<ApiResponse<ImportResult>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut errors = Vec::new();
    let now = chrono::Utc::now().timestamp();

    for client in req.clients {
        // Check if email already exists
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM client_traffics WHERE inbound_id = ? AND email = ?"
        )
        .bind(req.inbound_id)
        .bind(&client.email)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if existing.is_some() {
            failed_count += 1;
            errors.push(format!("Client {} already exists", client.email));
            continue;
        }

        // Generate subscription ID if not provided
        let sub_id = client.sub_id.unwrap_or_else(|| {
            uuid::Uuid::new_v4().to_string().replace("-", "")
        });

        let result = sqlx::query(
            "INSERT INTO client_traffics (
                inbound_id, email, up, down, total, expiry_time, enable,
                limit_ip, tg_id, sub_id, comment, reset, created_at, updated_at
            ) VALUES (?, ?, 0, 0, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(req.inbound_id)
        .bind(&client.email)
        .bind(client.total.unwrap_or(0))
        .bind(client.expiry_time.unwrap_or(0))
        .bind(client.enable.unwrap_or(true) as i32)
        .bind(client.limit_ip.unwrap_or(0))
        .bind(client.tg_id.unwrap_or(0))
        .bind(&sub_id)
        .bind(client.comment.unwrap_or_default())
        .bind(client.reset.unwrap_or(0))
        .bind(now)
        .bind(now)
        .execute(&state.db)
        .await;

        match result {
            Ok(_) => success_count += 1,
            Err(e) => {
                failed_count += 1;
                errors.push(format!("Failed to add {}: {}", client.email, e));
            }
        }
    }

    // Regenerate Xray config
    if success_count > 0 {
        if let Err(e) = regenerate_xray_config(&state).await {
            tracing::error!("Failed to regenerate Xray config: {}", e);
        }
    }

    Ok(Json(ApiResponse::success(ImportResult {
        success_count,
        failed_count,
        errors,
    })))
}

/// Result of import operation
#[derive(Debug, serde::Serialize, Default)]
pub struct ImportResult {
    pub success_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

/// Client info for listing
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct ClientInfo {
    pub id: i64,
    pub inbound_id: i64,
    pub email: String,
    pub up: i64,
    pub down: i64,
    pub total: i64,
    pub expiry_time: i64,
    pub enable: bool,
    pub limit_ip: i32,
    pub tg_id: i64,
    pub sub_id: String,
    pub comment: String,
    pub reset: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

/// List clients for an inbound
pub async fn list_clients(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<ClientInfo>>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let clients: Vec<ClientInfo> = sqlx::query_as::<_, ClientInfo>(
        "SELECT id, inbound_id, email, up, down, total, expiry_time, enable,
                limit_ip, tg_id, sub_id, comment, reset, created_at, updated_at
         FROM client_traffics
         WHERE inbound_id = ?
         ORDER BY id"
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch clients: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(clients)))
}

/// Online client info
#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct OnlineClient {
    pub email: String,
    pub inbound_id: i64,
    pub inbound_tag: String,
    pub ip: String,
    pub last_seen: i64,
}

/// Get online clients (clients with recent activity)
pub async fn get_online_clients(
    State(_state): State<AppState>,
    session: Session,
) -> Result<Json<ApiResponse<Vec<OnlineClient>>>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // This would typically require real-time tracking from Xray
    // For now, return empty or implement based on traffic logs
    let online: Vec<OnlineClient> = vec![];

    Ok(Json(ApiResponse::success(online)))
}
