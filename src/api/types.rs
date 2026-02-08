use crate::chunk::Priority;
use crate::session::SessionStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTransferRequest {
    pub file_path: String,
    pub priority: Priority,
    pub receiver_addr: Option<String>, // Optional receiver address (e.g., "192.168.1.100:5001")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTransferResponse {
    pub session_id: String,
    pub message: String,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
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

// --- Metric response types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureMetricsResponse {
    pub data_shards: usize,
    pub parity_shards: usize,
    pub observed_loss_rate: f32,
    pub overhead_percent: f64,
    pub recovery_capability: f64,
    pub thresholds: Vec<ErasureThreshold>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErasureThreshold {
    pub loss_rate: f32,
    pub parity: usize,
    pub overhead_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetricsResponse {
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub chunks_sent: u64,
    pub chunks_received: u64,
    pub retransmissions: u64,
    pub active_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMetricsResponse {
    pub critical_pending: usize,
    pub high_pending: usize,
    pub normal_pending: usize,
    pub total_processed: u64,
    pub total_enqueued: u64,
    pub avg_wait_time_ms: u64,
    pub max_wait_time_ms: u64,
    pub capacity_used: usize,
    pub capacity_total: usize,
    pub utilization_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummaryResponse {
    pub active_transfers: usize,
    pub completed_transfers: usize,
    pub current_loss_rate: f32,
    pub recovery_capability: f64,
    pub current_parity_shards: usize,
    pub data_shards: usize,
    pub overhead_percent: f64,
    pub total_bytes_transferred: u64,
    pub total_chunks_processed: u64,
    pub queue_depth: usize,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationRequest {
    pub loss_rate: f32,
    pub duration_seconds: Option<u64>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResponse {
    pub message: String,
    pub applied_loss_rate: f32,
    pub resulting_parity: usize,
    pub recovery_capability: f64,
    pub overhead_percent: f64,
    // File-based simulation fields (present when file_path was provided)
    pub file_name: Option<String>,
    pub total_chunks: Option<u32>,
    pub data_chunks: Option<usize>,
    pub parity_chunks: Option<usize>,
    // Multi-trial aggregate stats
    pub num_trials: Option<u32>,
    pub successful_trials: Option<u32>,
    pub success_rate: Option<f64>,
    pub avg_chunks_lost: Option<f64>,
    pub avg_chunks_recovered: Option<f64>,
    pub min_chunks_lost: Option<u32>,
    pub max_chunks_lost: Option<u32>,
    pub file_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    TransferProgress(TransferProgressResponse),
    MetricsSnapshot(MetricsSnapshotData),
    TransferStateChanged {
        session_id: String,
        new_state: String,
    },
    TransferCompleted {
        session_id: String,
    },
    TransferFailed {
        session_id: String,
        error: String,
    },
    Error(ErrorResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshotData {
    pub timestamp: u64,
    pub loss_rate: f32,
    pub parity_shards: usize,
    pub data_shards: usize,
    pub recovery_capability: f64,
    pub overhead_percent: f64,
    pub throughput_bps: u64,
    pub active_transfers: usize,
    pub queue_depth: usize,
    pub chunks_sent: u64,
    pub chunks_lost: u64,
    pub chunks_recovered: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedFileInfo {
    pub file_name: String,
    pub file_path: String,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListUploadsResponse {
    pub files: Vec<UploadedFileInfo>,
}

// --- Comparison simulation types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonRequest {
    pub file_path: String,
    pub trials_per_point: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonPoint {
    pub loss_percent: u32,
    pub tcp_success_rate: f64,
    pub resilient_success_rate: f64,
    pub tcp_avg_chunks_lost: f64,
    pub resilient_avg_chunks_lost: f64,
    pub resilient_avg_recovered: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResponse {
    pub file_name: String,
    pub file_size_bytes: u64,
    pub total_chunks: u32,
    pub data_chunks: usize,
    pub parity_chunks: usize,
    pub trials_per_point: u32,
    pub points: Vec<ComparisonPoint>,
}
