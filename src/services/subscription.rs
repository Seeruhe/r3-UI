use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    pub url: String,
    pub token: String,
    pub enabled: bool,
}

pub struct SubscriptionService;

impl SubscriptionService {
    /// Generate a subscription URL
    pub fn generate_url(host: &str, port: u16, token: &str) -> String {
        format!("http://{}:{}/sub/{}", host, port, token)
    }

    /// Generate a random subscription token
    pub fn generate_token() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        format!("{:x}", rng.sample::<u64, _>(rand::distributions::Standard))
    }
}
