//! WebSocket handler for Axum

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast::Receiver;
use std::sync::Arc;

use crate::AppState;
use crate::websocket::hub::WsHub;

/// WebSocket upgrade handler
pub async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state.ws_hub.subscribe(), state.ws_hub))
}

async fn handle_socket(socket: WebSocket, mut receiver: Receiver<String>, ws_hub: Arc<WsHub>) {
    let (mut sender, mut recv) = socket.split();

    // Send welcome message
    let welcome = serde_json::json!({
        "type": "connected",
        "message": "WebSocket connected successfully"
    });
    if let Ok(msg) = serde_json::to_string(&welcome) {
        let _ = sender.send(Message::Text(msg.into())).await;
    }

    loop {
        tokio::select! {
            // Handle incoming messages from the socket
            msg = recv.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle ping/pong or other client messages
                        if text == "ping" {
                            let _ = sender.send(Message::Text("pong".into())).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::debug!("WebSocket connection closed");
                        ws_hub.client_disconnected();
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    Some(Err(e)) => {
                        tracing::debug!("WebSocket receive error: {}", e);
                        ws_hub.client_disconnected();
                        break;
                    }
                    _ => {}
                }
            }

            // Handle broadcast messages
            msg = receiver.recv() => {
                match msg {
                    Ok(text) => {
                        if sender.send(Message::Text(text.into())).await.is_err() {
                            tracing::debug!("Failed to send WebSocket message");
                            ws_hub.client_disconnected();
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        ws_hub.client_disconnected();
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Continue even if we missed some messages
                        continue;
                    }
                }
            }
        }
    }
}
