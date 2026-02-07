use crate::api::error::{ApiError, ApiResult};
use crate::api::types::*;
use crate::coordinator::TransferCoordinator;
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub struct RestApi {
    coordinator: Arc<TransferCoordinator>,
}

impl RestApi {
    pub fn new(coordinator: TransferCoordinator) -> Self {
        Self {
            coordinator: Arc::new(coordinator),
        }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health_check))
            .route("/api/v1/transfers", post(start_transfer))
            .route("/api/v1/upload", post(upload_and_transfer))
            .route("/api/v1/transfers", get(list_transfers))
            .route("/api/v1/transfers/:id", get(get_transfer))
            .route("/api/v1/transfers/:id/pause", post(pause_transfer))
            .route("/api/v1/transfers/:id/resume", post(resume_transfer))
            .route("/api/v1/transfers/:id/cancel", post(cancel_transfer))
            .route("/api/v1/transfers/:id/progress", get(get_progress))
            // Metric endpoints
            .route("/api/v1/metrics/erasure", get(get_erasure_metrics))
            .route("/api/v1/metrics/network", get(get_network_metrics))
            .route("/api/v1/metrics/queue", get(get_queue_metrics))
            .route("/api/v1/metrics/summary", get(get_metrics_summary))
            // Simulation endpoint
            .route("/api/v1/simulate/packet-loss", post(simulate_packet_loss))
            .with_state(self.coordinator.clone())
    }
}

async fn health_check() -> &'static str {
    "OK"
}

async fn upload_and_transfer(
    State(coordinator): State<Arc<TransferCoordinator>>,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<StartTransferResponse>)> {
    let mut file_path: Option<std::path::PathBuf> = None;
    let mut priority = crate::chunk::Priority::Normal;
    let mut receiver_addr: Option<std::net::SocketAddr> = None;

    // Create uploads directory if it doesn't exist
    let upload_dir = std::path::PathBuf::from("./uploads");
    tokio::fs::create_dir_all(&upload_dir)
        .await
        .map_err(|e| ApiError::InternalError(format!("Failed to create uploads directory: {e}")))?;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read multipart field: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            let filename = field
                .file_name()
                .ok_or_else(|| ApiError::InvalidRequest("No filename provided".to_string()))?
                .to_string();

            let filepath = upload_dir.join(&filename);
            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::InvalidRequest(format!("Failed to read file data: {e}")))?;

            let mut file = File::create(&filepath)
                .await
                .map_err(|e| ApiError::InternalError(format!("Failed to create file: {e}")))?;

            file.write_all(&data)
                .await
                .map_err(|e| ApiError::InternalError(format!("Failed to write file: {e}")))?;

            file_path = Some(filepath);
        } else if name == "priority" {
            let priority_str = field
                .text()
                .await
                .map_err(|e| ApiError::InvalidRequest(format!("Failed to read priority: {e}")))?;

            priority = match priority_str.as_str() {
                "Critical" => crate::chunk::Priority::Critical,
                "High" => crate::chunk::Priority::High,
                "Normal" => crate::chunk::Priority::Normal,
                _ => crate::chunk::Priority::Normal,
            };
        } else if name == "receiver_addr" {
            let addr_str = field.text().await.map_err(|e| {
                ApiError::InvalidRequest(format!("Failed to read receiver address: {e}"))
            })?;

            receiver_addr =
                Some(addr_str.parse().map_err(|e| {
                    ApiError::InvalidRequest(format!("Invalid receiver address: {e}"))
                })?);
        }
    }

    let file_path =
        file_path.ok_or_else(|| ApiError::InvalidRequest("No file uploaded".to_string()))?;

    let session_id = coordinator
        .send_file(file_path, priority, receiver_addr)
        .await
        .map_err(ApiError::CoordinatorError)?;

    Ok((
        StatusCode::CREATED,
        Json(StartTransferResponse {
            session_id: session_id.clone(),
            message: format!("File uploaded and transfer started with session ID: {session_id}"),
        }),
    ))
}

async fn start_transfer(
    State(coordinator): State<Arc<TransferCoordinator>>,
    Json(req): Json<StartTransferRequest>,
) -> ApiResult<(StatusCode, Json<StartTransferResponse>)> {
    let file_path = std::path::PathBuf::from(&req.file_path);

    if !file_path.exists() {
        return Err(ApiError::InvalidRequest(format!(
            "File not found: {}",
            req.file_path
        )));
    }

    // Parse receiver address if provided
    let receiver_addr = if let Some(addr_str) = &req.receiver_addr {
        Some(
            addr_str
                .parse()
                .map_err(|e| ApiError::InvalidRequest(format!("Invalid receiver address: {e}")))?,
        )
    } else {
        None
    };

    let session_id = coordinator
        .send_file(file_path, req.priority, receiver_addr)
        .await
        .map_err(ApiError::CoordinatorError)?;

    Ok((
        StatusCode::CREATED,
        Json(StartTransferResponse {
            session_id: session_id.clone(),
            message: format!("Transfer started with session ID: {session_id}"),
        }),
    ))
}

async fn list_transfers(
    State(coordinator): State<Arc<TransferCoordinator>>,
) -> Json<ListTransfersResponse> {
    // Return recent transfers (includes active, completed, and failed)
    let active_transfers = coordinator.list_recent();
    let count = active_transfers.len();

    Json(ListTransfersResponse {
        active_transfers,
        count,
    })
}

async fn get_transfer(
    State(coordinator): State<Arc<TransferCoordinator>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<TransferStateResponse>> {
    let state = coordinator
        .get_state(&session_id)
        .ok_or_else(|| ApiError::NotFound(format!("Transfer not found: {session_id}")))?;

    let state_str = match state {
        crate::coordinator::TransferState::Idle => "Idle",
        crate::coordinator::TransferState::Preparing => "Preparing",
        crate::coordinator::TransferState::Transferring { .. } => "Transferring",
        crate::coordinator::TransferState::Paused { .. } => "Paused",
        crate::coordinator::TransferState::Completing => "Completing",
        crate::coordinator::TransferState::Completed => "Completed",
        crate::coordinator::TransferState::Failed { .. } => "Failed",
    };

    Ok(Json(TransferStateResponse {
        session_id,
        state: state_str.to_string(),
        is_active: state.is_active(),
        is_paused: state.is_paused(),
        is_terminal: state.is_terminal(),
    }))
}

async fn pause_transfer(
    State(coordinator): State<Arc<TransferCoordinator>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<SuccessResponse>> {
    coordinator
        .pause_transfer(&session_id)
        .await
        .map_err(ApiError::CoordinatorError)?;

    Ok(Json(SuccessResponse {
        message: format!("Transfer {session_id} paused"),
    }))
}

async fn resume_transfer(
    State(coordinator): State<Arc<TransferCoordinator>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<SuccessResponse>> {
    coordinator
        .resume_transfer(&session_id)
        .await
        .map_err(ApiError::CoordinatorError)?;

    Ok(Json(SuccessResponse {
        message: format!("Transfer {session_id} resumed"),
    }))
}

async fn cancel_transfer(
    State(coordinator): State<Arc<TransferCoordinator>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<SuccessResponse>> {
    coordinator
        .cancel_transfer(&session_id)
        .await
        .map_err(ApiError::CoordinatorError)?;

    Ok(Json(SuccessResponse {
        message: format!("Transfer {session_id} cancelled"),
    }))
}

async fn get_progress(
    State(coordinator): State<Arc<TransferCoordinator>>,
    Path(session_id): Path<String>,
) -> ApiResult<Json<TransferProgressResponse>> {
    let progress = coordinator
        .get_progress(&session_id)
        .await
        .map_err(ApiError::CoordinatorError)?;

    Ok(Json(TransferProgressResponse {
        session_id: progress.session_id,
        status: progress.status,
        progress_percent: progress.progress_percent,
        completed_chunks: progress.completed_chunks,
        total_chunks: progress.total_chunks,
        bytes_transferred: progress.bytes_transferred,
        total_bytes: progress.total_bytes,
        current_speed_bps: progress.current_speed_bps,
    }))
}

// --- Metric endpoints ---

async fn get_erasure_metrics(
    State(coordinator): State<Arc<TransferCoordinator>>,
) -> Json<ErasureMetricsResponse> {
    let status = coordinator.adaptive_coder().status();
    let config = crate::chunk::AdaptiveErasureConfig::default();

    let thresholds: Vec<ErasureThreshold> = config
        .thresholds
        .iter()
        .map(|&(loss_rate, parity)| ErasureThreshold {
            loss_rate,
            parity,
            overhead_percent: config.overhead_percent(parity),
        })
        .collect();

    Json(ErasureMetricsResponse {
        data_shards: status.data_shards,
        parity_shards: status.parity_shards,
        observed_loss_rate: status.observed_loss_rate,
        overhead_percent: status.overhead_percent,
        recovery_capability: status.recovery_capability,
        thresholds,
    })
}

async fn get_network_metrics(
    State(_coordinator): State<Arc<TransferCoordinator>>,
) -> Json<NetworkMetricsResponse> {
    // Network stats from the coordinator's simulation counters
    Json(NetworkMetricsResponse {
        total_bytes_sent: 0,
        total_bytes_received: 0,
        chunks_sent: _coordinator.sim_chunks_sent(),
        chunks_received: _coordinator
            .sim_chunks_sent()
            .saturating_sub(_coordinator.sim_chunks_lost()),
        retransmissions: 0,
        active_connections: _coordinator.list_active().len(),
    })
}

async fn get_queue_metrics(
    State(coordinator): State<Arc<TransferCoordinator>>,
) -> Json<QueueMetricsResponse> {
    let stats = coordinator.queue_stats();
    let (used, _available, utilization) = coordinator.queue_capacity();

    Json(QueueMetricsResponse {
        critical_pending: stats.critical_pending,
        high_pending: stats.high_pending,
        normal_pending: stats.normal_pending,
        total_processed: stats.total_processed,
        total_enqueued: stats.total_enqueued,
        avg_wait_time_ms: stats.avg_wait_time_ms,
        max_wait_time_ms: stats.max_wait_time_ms,
        capacity_used: used,
        capacity_total: 1_000_000,
        utilization_percent: utilization,
    })
}

async fn get_metrics_summary(
    State(coordinator): State<Arc<TransferCoordinator>>,
) -> Json<MetricsSummaryResponse> {
    let erasure_status = coordinator.adaptive_coder().status();
    let queue_stats = coordinator.queue_stats();

    Json(MetricsSummaryResponse {
        active_transfers: coordinator.list_active().len(),
        completed_transfers: coordinator.count_completed(),
        current_loss_rate: erasure_status.observed_loss_rate,
        recovery_capability: erasure_status.recovery_capability,
        current_parity_shards: erasure_status.parity_shards,
        data_shards: erasure_status.data_shards,
        overhead_percent: erasure_status.overhead_percent,
        total_bytes_transferred: 0,
        total_chunks_processed: queue_stats.total_processed,
        queue_depth: queue_stats.total_pending(),
        uptime_seconds: coordinator.uptime_seconds(),
    })
}

async fn simulate_packet_loss(
    State(coordinator): State<Arc<TransferCoordinator>>,
    Json(req): Json<SimulationRequest>,
) -> Json<SimulationResponse> {
    let loss_rate = req.loss_rate.clamp(0.0, 0.5);
    let num_samples = 100;

    coordinator.simulate_packet_loss(loss_rate, num_samples);

    let status = coordinator.adaptive_coder().status();

    Json(SimulationResponse {
        message: format!(
            "Simulated {}% packet loss with {} samples",
            (loss_rate * 100.0) as u32,
            num_samples
        ),
        applied_loss_rate: loss_rate,
        resulting_parity: status.parity_shards,
        recovery_capability: status.recovery_capability,
        overhead_percent: status.overhead_percent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkManager;
    use crate::integrity::IntegrityVerifier;
    use crate::network::{ConnectionConfig, QuicTransport};
    use crate::priority::PriorityQueue;
    use crate::session::SessionStore;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::Service;

    async fn create_test_api() -> RestApi {
        let chunk_manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();
        let verifier = IntegrityVerifier;
        let config = ConnectionConfig::default();
        let transport = QuicTransport::new(config).await.unwrap();
        let queue = PriorityQueue::new(1_000_000);
        let session_store = SessionStore::new_in_memory().await.unwrap();

        let coordinator =
            TransferCoordinator::new(chunk_manager, verifier, transport, queue, session_store);

        RestApi::new(coordinator)
    }

    #[tokio::test]
    async fn test_health_check() {
        let api = create_test_api().await;
        let mut app = api.router();

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let response = app.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_transfers_empty() {
        let api = create_test_api().await;
        let mut app = api.router();

        let request = Request::builder()
            .uri("/api/v1/transfers")
            .body(Body::empty())
            .unwrap();
        let response = app.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let list: ListTransfersResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(list.count, 0);
        assert_eq!(list.active_transfers.len(), 0);
    }

    #[tokio::test]
    async fn test_get_nonexistent_transfer() {
        let api = create_test_api().await;
        let mut app = api.router();

        let request = Request::builder()
            .uri("/api/v1/transfers/nonexistent-id")
            .body(Body::empty())
            .unwrap();
        let response = app.call(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
