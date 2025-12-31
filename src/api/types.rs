use crate::chunk::Priority;
use crate::coordinator::TransferState;
use crate::session::SessionStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTransferRequest {
    pub file_path: String,
    pub priority: Priority,
    pub receiver_addr: Option<String>,  // Optional receiver address (e.g., "192.168.1.100:5001")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTransferResponse {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgressResponse {
    pub session_id: String,
    pub status: SessionStatus,
    pub progress_percent: f32,
    pub completed_chunks: u32,
    pub total_chunks: u32,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub current_speed_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferStateResponse {
    pub session_id: String,
    pub state: String,
    pub is_active: bool,
    pub is_paused: bool,
    pub is_terminal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTransfersResponse {
    pub active_transfers: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    TransferProgress(TransferProgressResponse),
    TransferStateChanged { session_id: String, new_state: String },
    TransferCompleted { session_id: String },
    TransferFailed { session_id: String, error: String },
    Error(ErrorResponse),
}
