mod error;
mod rest;
mod types;
mod websocket;

pub use error::{ApiError, ApiResult};
pub use rest::RestApi;
pub use types::*;
pub use websocket::websocket_handler;

use crate::coordinator::TransferCoordinator;
use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};

/// Create a complete API server with REST and WebSocket support
pub fn create_api_server(coordinator: TransferCoordinator) -> Router {
    let rest_api = RestApi::new(coordinator.clone());
    let coordinator_arc = Arc::new(coordinator);

    // Configure CORS to allow frontend requests
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let ws_router = Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(coordinator_arc);

    Router::new()
        .merge(rest_api.router())
        .merge(ws_router)
        .layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkManager;
    use crate::integrity::IntegrityVerifier;
    use crate::network::{ConnectionConfig, QuicTransport};
    use crate::priority::PriorityQueue;
    use crate::session::SessionStore;

    async fn create_test_coordinator() -> TransferCoordinator {
        let chunk_manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();
        let verifier = IntegrityVerifier;
        let config = ConnectionConfig::default();
        let transport = QuicTransport::new(config).await.unwrap();
        let queue = PriorityQueue::new(1_000_000);
        let session_store = SessionStore::new_in_memory().await.unwrap();

        TransferCoordinator::new(chunk_manager, verifier, transport, queue, session_store)
    }

    #[tokio::test]
    async fn test_api_server_creation() {
        let coordinator = create_test_coordinator().await;
        let _app = create_api_server(coordinator);
        // If we reach here, server was created successfully
    }
}
