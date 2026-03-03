use tower_sessions::Session;

/// Session helper functions
pub struct SessionManager;

impl SessionManager {
    pub const USER_ID_KEY: &'static str = "user_id";

    pub async fn get_user_id(session: &Session) -> Option<i64> {
        session.get(Self::USER_ID_KEY).await.ok().flatten()
    }

    pub async fn set_user_id(session: &Session, user_id: i64) -> bool {
        session.insert(Self::USER_ID_KEY, user_id).await.is_ok()
    }

    pub async fn clear(session: &Session) -> bool {
        session.delete().await.is_ok()
    }
}
