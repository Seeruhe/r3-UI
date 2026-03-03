//! API response utilities
//!
//! Standardized API response format for all endpoints.

use serde::{Deserialize, Serialize};

/// Standard API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response with data
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
        }
    }

    /// Create a successful response with a message but no data
    /// This is used when we need to return early with a message
    pub fn success_msg(message: &str) -> Self
    where
        T: Default,
    {
        Self {
            success: true,
            data: Some(T::default()),
            message: Some(message.to_string()),
        }
    }

    /// Create an error response with a message
    pub fn error(message: &str) -> Self
    where
        T: Default,
    {
        Self {
            success: false,
            data: Some(T::default()),
            message: Some(message.to_string()),
        }
    }
}

impl ApiResponse<()> {
    /// Create a successful empty response
    pub fn ok() -> Self {
        Self {
            success: true,
            data: Some(()),
            message: None,
        }
    }
}
