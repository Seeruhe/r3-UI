use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::FromRow;

use crate::AppState;

#[derive(Debug, FromRow)]
struct InboundRow {
    #[allow(dead_code)]
    id: i64,
    remark: Option<String>,
    listen: Option<String>,
    port: i32,
    protocol: String,
    settings: Option<String>,
    stream_settings: Option<String>,
    tag: String,
}

/// Subscription handler - generates client config for subscription
pub async fn get_sub(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Validate token and get user
    let user_id: Option<i64> = validate_subscription_token(&state, &token).await?;

    match user_id {
        Some(_uid) => {
            // Get inbounds for user
            let inbounds = get_user_inbounds(&state).await?;

            // Generate subscription content
            let content = generate_subscription_content(&inbounds);

            Ok((
                StatusCode::OK,
                [("Content-Type", "text/plain; charset=utf-8")],
                content,
            ))
        }
        None => Ok((
            StatusCode::UNAUTHORIZED,
            [("Content-Type", "text/plain")],
            "Invalid subscription token".to_string(),
        )),
    }
}

/// JSON subscription handler
pub async fn get_sub_json(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Validate token and get user
    let user_id: Option<i64> = validate_subscription_token(&state, &token).await?;

    match user_id {
        Some(_uid) => {
            // Get inbounds for user
            let inbounds = get_user_inbounds(&state).await?;

            // Generate JSON subscription content
            let content = generate_json_subscription(&inbounds);

            Ok((
                StatusCode::OK,
                [("Content-Type", "application/json")],
                content,
            ))
        }
        None => Ok((
            StatusCode::UNAUTHORIZED,
            [("Content-Type", "application/json")],
            r#"{"error": "Invalid subscription token"}"#.to_string(),
        )),
    }
}

/// Generate JSON subscription (SIP008 format)
fn generate_json_subscription(inbounds: &[InboundRow]) -> String {
    let proxies: Vec<serde_json::Value> = inbounds
        .iter()
        .filter_map(|i| {
            let settings: serde_json::Value = i.settings.as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({}));

            let stream: serde_json::Value = i.stream_settings.as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::json!({}));

            Some(serde_json::json!({
                "remark": i.remark.as_ref().unwrap_or(&i.tag),
                "server": i.listen.as_ref().unwrap_or(&"0.0.0.0".to_string()),
                "server_port": i.port,
                "protocol": i.protocol,
                "settings": settings,
                "stream_settings": stream,
            }))
        })
        .collect();

    serde_json::to_string(&serde_json::json!({
        "version": 1,
        "proxies": proxies
    })).unwrap_or_else(|_| r#"{"version":1,"proxies":[]}"#.to_string())
}

async fn validate_subscription_token(state: &AppState, token: &str) -> Result<Option<i64>, StatusCode> {
    // Look up token in settings
    let value: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT value FROM settings WHERE key = 'sub_token'"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match value {
        Some((Some(stored_token),)) if stored_token == token => {
            // Get admin user id
            let user: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = 'admin'")
                .fetch_optional(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            Ok(user.map(|(id,)| id))
        }
        _ => Ok(None),
    }
}

async fn get_user_inbounds(state: &AppState) -> Result<Vec<InboundRow>, StatusCode> {
    sqlx::query_as::<_, InboundRow>(
        "SELECT id, remark, listen, port, protocol, settings, stream_settings, tag
         FROM inbounds WHERE enable = 1"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch inbounds: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

fn generate_subscription_content(inbounds: &[InboundRow]) -> String {
    let mut links = Vec::new();

    for inbound in inbounds {
        if let Some(link) = generate_link(inbound) {
            links.push(link);
        }
    }

    links.join("\n")
}

fn generate_link(inbound: &InboundRow) -> Option<String> {
    // Parse settings to get client info
    let settings: serde_json::Value = serde_json::from_str(inbound.settings.as_ref()?).ok()?;

    match inbound.protocol.as_str() {
        "vmess" => generate_vmess_link(inbound, &settings),
        "vless" => generate_vless_link(inbound, &settings),
        "trojan" => generate_trojan_link(inbound, &settings),
        "shadowsocks" => generate_ss_link(inbound, &settings),
        _ => None,
    }
}

fn generate_vmess_link(inbound: &InboundRow, settings: &serde_json::Value) -> Option<String> {
    let clients = settings.get("clients")?.as_array()?;
    let client = clients.first()?;

    let stream: serde_json::Value = inbound.stream_settings
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(serde_json::json!({}));
    let net = stream.get("network").and_then(|v| v.as_str()).unwrap_or("tcp");
    let tls = stream.get("security").and_then(|v| v.as_str()).unwrap_or("");
    let default_listen = "0.0.0.0".to_string();
    let address = inbound.listen.as_ref().unwrap_or(&default_listen);

    let vmess = serde_json::json!({
        "v": "2",
        "ps": inbound.remark.as_ref().unwrap_or(&inbound.tag),
        "add": address,
        "port": inbound.port.to_string(),
        "id": client.get("id").and_then(|v| v.as_str()).unwrap_or(""),
        "aid": "0",
        "net": net,
        "type": "none",
        "host": "",
        "path": "",
        "tls": tls,
    });

    Some(format!("vmess://{}", base64_encode(&vmess.to_string())))
}

fn generate_vless_link(inbound: &InboundRow, settings: &serde_json::Value) -> Option<String> {
    let clients = settings.get("clients")?.as_array()?;
    let client = clients.first()?;
    let uuid = client.get("id")?.as_str()?;
    let remark = inbound.remark.as_ref().unwrap_or(&inbound.tag);
    let default_listen = "0.0.0.0".to_string();
    let address = inbound.listen.as_ref().unwrap_or(&default_listen);

    Some(format!(
        "vless://{}@{}:{}?encryption=none#{}",
        uuid, address, inbound.port, remark
    ))
}

fn generate_trojan_link(inbound: &InboundRow, settings: &serde_json::Value) -> Option<String> {
    let clients = settings.get("clients")?.as_array()?;
    let client = clients.first()?;
    let password = client.get("password")?.as_str()?;
    let remark = inbound.remark.as_ref().unwrap_or(&inbound.tag);
    let default_listen = "0.0.0.0".to_string();
    let address = inbound.listen.as_ref().unwrap_or(&default_listen);

    Some(format!(
        "trojan://{}@{}:{}?security=none#{}",
        password, address, inbound.port, remark
    ))
}

fn generate_ss_link(inbound: &InboundRow, settings: &serde_json::Value) -> Option<String> {
    let method = settings.get("method")?.as_str()?;
    let password = settings.get("password")?.as_str()?;
    let remark = inbound.remark.as_ref().unwrap_or(&inbound.tag);
    let default_listen = "0.0.0.0".to_string();
    let address = inbound.listen.as_ref().unwrap_or(&default_listen);

    let userinfo = format!("{}:{}", method, password);
    Some(format!(
        "ss://{}@{}:{}#{}",
        base64_encode(&userinfo),
        address,
        inbound.port,
        remark
    ))
}

fn base64_encode(input: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD.encode(input.as_bytes())
}
