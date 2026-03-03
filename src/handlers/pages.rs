use axum::{
    extract::State,
    http::{header, Response, StatusCode},
    body::Body,
};
use tower_sessions::Session;

use crate::utils::response::ApiResponse;
use crate::AppState;

const SESSION_USER_KEY: &str = "user_id";

/// Serve login page
pub async fn login_page() -> Response<Body> {
    // Serve the simple login/index.html for now
    let html = include_str!("../../web/html/index.html");
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

/// Serve panel index (requires auth)
pub async fn panel_index(
    State(_state): State<AppState>,
    session: Session,
) -> Result<Response<Body>, StatusCode> {
    // Check authentication
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        // Redirect to login
        return Ok(Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "/")
            .body(Body::empty())
            .unwrap());
    }

    // Serve the same index.html (it handles auth check client-side)
    let html = include_str!("../../web/html/index.html");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap())
}

/// Serve inbounds page
pub async fn inbounds_page(
    State(_state): State<AppState>,
    session: Session,
) -> Result<Response<Body>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "/")
            .body(Body::empty())
            .unwrap());
    }

    let html = include_str!("../../web/html/index.html");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap())
}

/// Serve settings page
pub async fn settings_page(
    State(_state): State<AppState>,
    session: Session,
) -> Result<Response<Body>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "/")
            .body(Body::empty())
            .unwrap());
    }

    let html = include_str!("../../web/html/index.html");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap())
}

/// Serve xray config page
pub async fn xray_page(
    State(_state): State<AppState>,
    session: Session,
) -> Result<Response<Body>, StatusCode> {
    let user_id: Option<i64> = session.get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if user_id.is_none() {
        return Ok(Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, "/")
            .body(Body::empty())
            .unwrap());
    }

    let html = include_str!("../../web/html/index.html");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap())
}

/// Check two-factor auth status
pub async fn get_two_factor_enable(
    State(state): State<AppState>,
) -> Result<Response<Body>, StatusCode> {
    // Check if 2FA is enabled for the admin user
    let result: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 FROM settings WHERE key = 'twoFactorEnable' AND value = 'true'"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let enabled = result.is_some();
    let json = serde_json::to_string(&ApiResponse::success(enabled)).unwrap();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json))
        .unwrap())
}
