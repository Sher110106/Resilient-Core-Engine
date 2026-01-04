use crate::api::types::*;
use crate::coordinator::TransferCoordinator;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
};
use std::sync::Arc;
use tokio::time::{interval, Duration};

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(coordinator): State<Arc<TransferCoordinator>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_websocket(socket, coordinator))
}

async fn handle_websocket(mut socket: WebSocket, coordinator: Arc<TransferCoordinator>) {
    let mut tick = interval(Duration::from_millis(500));

    loop {
        tokio::select! {
            _ = tick.tick() => {
                // Send progress updates for all active transfers
                let active_transfers = coordinator.list_active();

                for session_id in active_transfers {
                    if let Ok(progress) = coordinator.get_progress(&session_id).await {
                        let msg = WebSocketMessage::TransferProgress(TransferProgressResponse {
                            session_id: progress.session_id.clone(),
                            status: progress.status,
                            progress_percent: progress.progress_percent,
                            completed_chunks: progress.completed_chunks,
                            total_chunks: progress.total_chunks,
                            bytes_transferred: progress.bytes_transferred,
                            total_bytes: progress.total_bytes,
                            current_speed_bps: progress.current_speed_bps,
                        });

                        if let Ok(json) = serde_json::to_string(&msg) {
                            if socket.send(Message::Text(json)).await.is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle incoming commands (optional)
                        if text == "ping"
                            && socket.send(Message::Text("pong".to_string())).await.is_err() {
                                return;
                            }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_message_serialization() {
        let msg = WebSocketMessage::TransferProgress(TransferProgressResponse {
            session_id: "test-123".to_string(),
            status: crate::session::SessionStatus::Active,
            progress_percent: 50.0,
            completed_chunks: 5,
            total_chunks: 10,
            bytes_transferred: 1000,
            total_bytes: 2000,
            current_speed_bps: 1000000,
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("TransferProgress"));
        assert!(json.contains("test-123"));
    }

    #[test]
    fn test_transfer_completed_message() {
        let msg = WebSocketMessage::TransferCompleted {
            session_id: "test-456".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("TransferCompleted"));
        assert!(json.contains("test-456"));
    }
}
