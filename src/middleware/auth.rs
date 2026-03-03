use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use tower_sessions::Session;

/// Authentication middleware
pub async fn auth_middleware(
    session: Session,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    const SESSION_USER_KEY: &str = "user_id";

    let user_id: Option<i64> = session
        .get(SESSION_USER_KEY)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match user_id {
        Some(_) => Ok(next.run(request).await),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}
