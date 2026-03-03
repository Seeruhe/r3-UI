//! WebSocket Hub for real-time communication
//!
//! This module provides a hub for managing WebSocket connections and broadcasting messages.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast::{channel, Sender, Receiver};
use serde::Serialize;

const BROADCAST_CAPACITY: usize = 256;

pub struct WsHub {
    sender: Sender<String>,
    client_count: Arc<AtomicUsize>,
}

impl WsHub {
    pub fn new() -> Self {
        let (sender, _) = channel(BROADCAST_CAPACITY);
        Self {
            sender,
            client_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Subscribe to the hub and get a receiver
    pub fn subscribe(&self) -> Receiver<String> {
        self.client_count.fetch_add(1, Ordering::SeqCst);
        self.sender.subscribe()
    }

    /// Decrement client count when client disconnects
    pub fn client_disconnected(&self) {
        self.client_count.fetch_sub(1, Ordering::SeqCst);
    }

    /// Broadcast a message to all connected clients
    pub async fn broadcast<T: Serialize>(&self, msg: &T) {
        if let Ok(json) = serde_json::to_string(msg) {
            let _ = self.sender.send(json);
        }
    }

    /// Get the number of connected clients
    pub fn client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst)
    }
}

impl Default for WsHub {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WsHub {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            client_count: self.client_count.clone(),
        }
    }
}
