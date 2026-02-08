use crate::chunk::{AdaptiveErasureCoder, AdaptiveErasureConfig};
use crate::chunk::{Chunk, ChunkManager, FileManifest, Priority};
use crate::coordinator::error::{CoordinatorError, CoordinatorResult};
use crate::coordinator::state_machine::TransferStateMachine;
use crate::coordinator::types::{TransferEvent, TransferProgress, TransferState};
use crate::integrity::IntegrityVerifier;
use crate::network::{QuicPathStats, QuicTransport};
use crate::priority::PriorityQueue;
use crate::session::{SessionState, SessionStatus, SessionStore};
use dashmap::DashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;

/// Result of a file-based packet loss simulation (aggregated over multiple trials)
#[derive(Debug, Clone)]
pub struct SimulateFileResult {
    pub file_name: String,
    pub file_size_bytes: u64,
    pub total_chunks: u32,
    pub data_chunks: usize,
    pub parity_chunks: usize,
    // Aggregate stats over N trials
    pub num_trials: u32,
    pub successful_trials: u32,
    pub success_rate: f64,
    pub avg_chunks_lost: f64,
    pub avg_chunks_recovered: f64,
    pub min_chunks_lost: u32,
    pub max_chunks_lost: u32,
}

/// Single data point in a TCP vs RESILIENT comparison sweep
#[derive(Debug, Clone)]
pub struct ComparisonPoint {
    pub loss_percent: u32,
    pub tcp_success_rate: f64,
    pub resilient_success_rate: f64,
    pub tcp_avg_chunks_lost: f64,
    pub resilient_avg_chunks_lost: f64,
    pub resilient_avg_recovered: f64,
}

/// Full comparison result across all loss rates
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub file_name: String,
    pub file_size_bytes: u64,
    pub total_chunks: u32,
    pub data_chunks: usize,
    pub parity_chunks: usize,
    pub trials_per_point: u32,
    pub points: Vec<ComparisonPoint>,
}

pub struct TransferCoordinator {
    chunk_manager: Arc<ChunkManager>,
    verifier: Arc<IntegrityVerifier>,
    transport: Arc<QuicTransport>,
    queue: Arc<PriorityQueue>,
    session_store: Arc<SessionStore>,

    // Active transfers
    active_transfers: Arc<DashMap<String, TransferStateMachine>>,

    // Recent transfers (including completed/failed) - kept for display
    recent_transfers: Arc<DashMap<String, TransferStateMachine>>,

    // Session ID mapping
    file_to_session: Arc<DashMap<String, String>>,

    // Adaptive erasure coder for metrics & simulation
    adaptive_coder: Arc<AdaptiveErasureCoder>,

    // Simulation counters
    sim_chunks_sent: Arc<AtomicU64>,
    sim_chunks_lost: Arc<AtomicU64>,
    sim_chunks_recovered: Arc<AtomicU64>,

    // Real QUIC path stats from the most recent transfer
    last_quic_stats: Arc<parking_lot::RwLock<QuicPathStats>>,

    // Start time for uptime tracking
    start_time: Instant,
}

impl TransferCoordinator {
    pub fn new(
        chunk_manager: ChunkManager,
        verifier: IntegrityVerifier,
        transport: QuicTransport,
        queue: PriorityQueue,
        session_store: SessionStore,
    ) -> Self {
        let adaptive_config = AdaptiveErasureConfig::default();
        let adaptive_coder = AdaptiveErasureCoder::new(adaptive_config);

        Self {
            chunk_manager: Arc::new(chunk_manager),
            verifier: Arc::new(verifier),
            transport: Arc::new(transport),
            queue: Arc::new(queue),
            session_store: Arc::new(session_store),
            active_transfers: Arc::new(DashMap::new()),
            recent_transfers: Arc::new(DashMap::new()),
            file_to_session: Arc::new(DashMap::new()),
            adaptive_coder: Arc::new(adaptive_coder),
            sim_chunks_sent: Arc::new(AtomicU64::new(0)),
            sim_chunks_lost: Arc::new(AtomicU64::new(0)),
            sim_chunks_recovered: Arc::new(AtomicU64::new(0)),
            last_quic_stats: Arc::new(parking_lot::RwLock::new(QuicPathStats::default())),
            start_time: Instant::now(),
        }
    }

    /// Start sending a file
    pub async fn send_file(
        &self,
        file_path: PathBuf,
        priority: Priority,
        receiver_addr: Option<SocketAddr>,
    ) -> CoordinatorResult<String> {
        // Check if already in progress
        let file_id = file_path.to_string_lossy().to_string();
        if self.file_to_session.contains_key(&file_id) {
            return Err(CoordinatorError::AlreadyInProgress(file_id));
        }

        // Split file into chunks
        let (manifest, chunks) = self
            .chunk_manager
            .split_file(&file_path, file_id.clone(), priority)
            .await?;

        // Create session with receiver address and file path for resumable transfers
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = SessionState::new_with_receiver(
            session_id.clone(),
            file_id.clone(),
            manifest.clone(),
            receiver_addr,
            Some(file_path.to_string_lossy().to_string()),
        );
        self.session_store.save(&session).await?;

        // Update session status to active
        self.session_store
            .update_status(&session_id, SessionStatus::Active)
            .await?;

        // Create state machine
        let state_machine = TransferStateMachine::new();
        state_machine.transition(TransferEvent::Start {
            file_path: file_path.clone(),
            priority,
        })?;

        // Register transfer
        self.active_transfers
            .insert(session_id.clone(), state_machine.clone());
        self.recent_transfers
            .insert(session_id.clone(), state_machine);
        self.file_to_session
            .insert(file_id.clone(), session_id.clone());

        // Start transfer worker
        let coordinator = self.clone();
        let worker_session_id = session_id.clone();
        let worker_file_id = file_id;
        tokio::spawn(async move {
            if let Err(e) = coordinator
                .transfer_worker(worker_session_id.clone(), manifest, chunks, receiver_addr)
                .await
            {
                eprintln!("Transfer worker failed for {worker_session_id}: {e}");
                // Mark as failed so the UI reflects the error
                let _ = coordinator
                    .session_store
                    .update_status(&worker_session_id, SessionStatus::Failed(e.to_string()))
                    .await;
                coordinator.active_transfers.remove(&worker_session_id);
                coordinator.file_to_session.remove(&worker_file_id);
            }
        });

        Ok(session_id)
    }

    /// Resume a paused transfer
    pub async fn resume_transfer(&self, session_id: &str) -> CoordinatorResult<()> {
        // Load session
        let session = self
            .session_store
            .load(session_id)
            .await?
            .ok_or_else(|| CoordinatorError::TransferNotFound(session_id.to_string()))?;

        // Check if can resume
        let resume_info = self.session_store.get_resume_info(session_id).await?;
        if !resume_info.can_resume {
            return Err(CoordinatorError::CannotResume(
                "Transfer is already complete or failed".into(),
            ));
        }

        // Get or create state machine
        let state_machine = if let Some(sm) = self.active_transfers.get(session_id) {
            sm.clone()
        } else {
            let sm = TransferStateMachine::new();
            self.active_transfers
                .insert(session_id.to_string(), sm.clone());
            sm
        };

        // Transition to resume
        state_machine.send_event(TransferEvent::Resume)?;

        // Update session status
        self.session_store
            .update_status(session_id, SessionStatus::Active)
            .await?;

        // Re-read chunks from the original file if we have the file path
        let chunks = if let Some(ref file_path_str) = session.file_path {
            let file_path = PathBuf::from(file_path_str);
            if file_path.exists() {
                // Re-split the file to get chunks (only the remaining ones will be sent)
                match self
                    .chunk_manager
                    .split_file(
                        &file_path,
                        session.file_id.clone(),
                        session.manifest.priority,
                    )
                    .await
                {
                    Ok((_, chunks)) => chunks,
                    Err(e) => {
                        tracing::warn!("Failed to re-read file for resume: {}", e);
                        vec![]
                    }
                }
            } else {
                tracing::warn!("Original file not found for resume: {}", file_path_str);
                vec![]
            }
        } else {
            tracing::warn!("No file path stored in session, cannot re-read chunks for resume");
            vec![]
        };

        // Use stored receiver address for resume
        let receiver_addr = session.receiver_addr;

        // Start transfer worker
        let coordinator = self.clone();
        let session_id_str = session_id.to_string();
        let manifest = session.manifest.clone();

        tokio::spawn(async move {
            if let Err(e) = coordinator
                .transfer_worker(session_id_str.clone(), manifest, chunks, receiver_addr)
                .await
            {
                eprintln!("Transfer worker failed for {session_id_str}: {e}");
            }
        });

        Ok(())
    }

    /// Pause a transfer
    pub async fn pause_transfer(&self, session_id: &str) -> CoordinatorResult<()> {
        let state_machine = self
            .active_transfers
            .get(session_id)
            .ok_or_else(|| CoordinatorError::TransferNotFound(session_id.to_string()))?;

        state_machine.transition(TransferEvent::Pause)?;
        self.session_store
            .update_status(session_id, SessionStatus::Paused)
            .await?;

        Ok(())
    }

    /// Cancel a transfer
    pub async fn cancel_transfer(&self, session_id: &str) -> CoordinatorResult<()> {
        if let Some(state_machine) = self.active_transfers.get(session_id) {
            state_machine.transition(TransferEvent::Cancel)?;
        }

        self.session_store
            .update_status(
                session_id,
                SessionStatus::Failed("Cancelled by user".into()),
            )
            .await?;
        self.active_transfers.remove(session_id);

        Ok(())
    }

    /// Get transfer progress
    pub async fn get_progress(&self, session_id: &str) -> CoordinatorResult<TransferProgress> {
        let session = self
            .session_store
            .load(session_id)
            .await?
            .ok_or_else(|| CoordinatorError::TransferNotFound(session_id.to_string()))?;

        let completed = session.completed_chunks.len() as u32;
        let total = session.manifest.total_chunks;
        let speed = session.current_speed_bps();

        Ok(TransferProgress {
            session_id: session_id.to_string(),
            completed_chunks: completed,
            total_chunks: total,
            bytes_transferred: session.metrics.bytes_transferred,
            total_bytes: session.manifest.total_size,
            progress_percent: session.progress_percent(),
            status: session.status,
            current_speed_bps: speed,
        })
    }

    /// Get current state
    pub fn get_state(&self, session_id: &str) -> Option<TransferState> {
        self.active_transfers
            .get(session_id)
            .map(|sm| sm.current_state())
    }

    /// List active transfers
    pub fn list_active(&self) -> Vec<String> {
        self.active_transfers
            .iter()
            .map(|e| e.key().clone())
            .collect()
    }

    /// List all recent transfers (including completed/failed)
    pub fn list_recent(&self) -> Vec<String> {
        self.recent_transfers
            .iter()
            .map(|e| e.key().clone())
            .collect()
    }

    /// Get queue statistics
    pub fn queue_stats(&self) -> crate::priority::QueueStats {
        self.queue.stats()
    }

    /// Get queue capacity info
    pub fn queue_capacity(&self) -> (usize, usize, f64) {
        self.queue.capacity_info()
    }

    /// Get the internal chunk manager (for erasure config access)
    pub fn chunk_manager(&self) -> &ChunkManager {
        &self.chunk_manager
    }

    /// Count completed (terminal) transfers
    pub fn count_completed(&self) -> usize {
        self.recent_transfers
            .iter()
            .filter(|e| e.value().current_state().is_terminal())
            .count()
    }

    /// Get the adaptive erasure coder (for metrics/simulation)
    pub fn adaptive_coder(&self) -> &AdaptiveErasureCoder {
        &self.adaptive_coder
    }

    /// Get simulation counters
    pub fn sim_chunks_sent(&self) -> u64 {
        self.sim_chunks_sent.load(Ordering::Relaxed)
    }

    pub fn sim_chunks_lost(&self) -> u64 {
        self.sim_chunks_lost.load(Ordering::Relaxed)
    }

    pub fn sim_chunks_recovered(&self) -> u64 {
        self.sim_chunks_recovered.load(Ordering::Relaxed)
    }

    /// Get the most recent real QUIC path stats (from an actual transfer)
    pub fn last_quic_stats(&self) -> QuicPathStats {
        self.last_quic_stats.read().clone()
    }

    /// Get the transport layer (for reading network stats)
    pub fn transport(&self) -> &QuicTransport {
        &self.transport
    }

    /// Simulate packet loss at a given rate.
    /// Directly sets the observed loss rate (no EMA smoothing) so the
    /// dashboard immediately reflects the slider value.
    pub fn simulate_packet_loss(&self, loss_rate: f32, num_samples: u32) {
        // Directly set the loss rate — no smoothing lag
        self.adaptive_coder.set_loss_rate(loss_rate);

        let losses = (num_samples as f32 * loss_rate) as u32;
        let successes = num_samples - losses;

        // Update simulation counters for the data flow visualization
        self.sim_chunks_sent
            .fetch_add(num_samples as u64, Ordering::Relaxed);
        self.sim_chunks_lost
            .fetch_add(losses as u64, Ordering::Relaxed);

        // Recovered = losses that could be recovered (up to parity capacity)
        let status = self.adaptive_coder.status();
        let max_recoverable = status.parity_shards as u32;
        let recovered = losses.min(max_recoverable);
        self.sim_chunks_recovered
            .fetch_add(recovered as u64, Ordering::Relaxed);

        // Also feed samples so future incremental updates work correctly
        for _ in 0..successes {
            self.adaptive_coder.record_success();
        }
        for _ in 0..losses {
            self.adaptive_coder.record_loss();
        }
    }

    /// Simulate a file transfer with a given packet loss rate.
    /// Runs multiple trials to produce statistically meaningful results.
    pub async fn simulate_file_transfer(
        &self,
        file_path: PathBuf,
        loss_rate: f32,
    ) -> CoordinatorResult<SimulateFileResult> {
        use rand::Rng;

        const NUM_TRIALS: u32 = 10;

        // Set the adaptive coder to reflect the simulated loss rate
        self.adaptive_coder.set_loss_rate(loss_rate);

        // Get the adaptive parity level for this loss rate
        let adaptive_parity = self.adaptive_coder.current_parity();

        // Split the file into chunks using smart chunk sizing for simulation
        let file_id = file_path.to_string_lossy().to_string();
        let file_size = tokio::fs::metadata(&file_path).await?.len();
        let sim_chunk_size = crate::chunk::ChunkManager::simulation_chunk_size(file_size);
        let (manifest, chunks) = self
            .chunk_manager
            .split_file_with_chunk_size(
                &file_path,
                file_id,
                Priority::Normal,
                sim_chunk_size,
                Some(adaptive_parity),
            )
            .await?;

        let total_chunks = chunks.len() as u32;
        let data_chunks = manifest.data_chunks as usize;
        let parity_chunks = manifest.parity_chunks as usize;

        let mut rng = rand::thread_rng();
        let mut successful_trials: u32 = 0;
        let mut total_lost: u32 = 0;
        let mut total_recovered: u32 = 0;
        let mut min_lost: u32 = u32::MAX;
        let mut max_lost: u32 = 0;

        for _ in 0..NUM_TRIALS {
            let mut lost_count: u32 = 0;

            for _ in &chunks {
                let roll: f32 = rng.gen();
                if roll < loss_rate {
                    lost_count += 1;
                }
            }

            let surviving = total_chunks - lost_count;
            let recoverable = surviving as usize >= data_chunks;

            if recoverable {
                successful_trials += 1;
                total_recovered += lost_count; // all lost chunks effectively recovered
            } else {
                let max_rec = (parity_chunks as u32).min(lost_count);
                total_recovered += max_rec;
            }

            total_lost += lost_count;
            min_lost = min_lost.min(lost_count);
            max_lost = max_lost.max(lost_count);
        }

        if min_lost == u32::MAX {
            min_lost = 0;
        }

        let avg_lost = total_lost as f64 / NUM_TRIALS as f64;
        let avg_recovered = total_recovered as f64 / NUM_TRIALS as f64;
        let success_rate = successful_trials as f64 / NUM_TRIALS as f64 * 100.0;

        // Update simulation counters for the live dashboard (aggregate across all trials)
        self.sim_chunks_sent.fetch_add(
            (total_chunks as u64) * (NUM_TRIALS as u64),
            Ordering::Relaxed,
        );
        self.sim_chunks_lost
            .fetch_add(total_lost as u64, Ordering::Relaxed);
        self.sim_chunks_recovered
            .fetch_add(total_recovered as u64, Ordering::Relaxed);

        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(SimulateFileResult {
            file_name,
            file_size_bytes: manifest.total_size,
            total_chunks,
            data_chunks,
            parity_chunks,
            num_trials: NUM_TRIALS,
            successful_trials,
            success_rate,
            avg_chunks_lost: avg_lost,
            avg_chunks_recovered: avg_recovered,
            min_chunks_lost: min_lost,
            max_chunks_lost: max_lost,
        })
    }

    /// Run a comparison simulation: for each loss rate (0%..40%), run N trials
    /// for both TCP-style (no FEC, any lost chunk = failure) and RESILIENT
    /// (Reed-Solomon parity). Returns per-point success rates.
    pub async fn simulate_comparison(
        &self,
        file_path: PathBuf,
        trials_per_point: u32,
    ) -> CoordinatorResult<ComparisonResult> {
        use rand::Rng;

        let file_id = file_path.to_string_lossy().to_string();
        let file_size = tokio::fs::metadata(&file_path).await?.len();
        let sim_chunk_size = crate::chunk::ChunkManager::simulation_chunk_size(file_size);
        // Use max parity (25 shards at severe loss) for comparison view so we
        // show the full RESILIENT recovery capability across all loss rates.
        let max_parity = 25_usize; // matches AdaptiveErasureConfig::max_parity_shards
        let (manifest, chunks) = self
            .chunk_manager
            .split_file_with_chunk_size(
                &file_path,
                file_id,
                Priority::Normal,
                sim_chunk_size,
                Some(max_parity),
            )
            .await?;

        let total_chunks = chunks.len() as u32;
        let data_chunks = manifest.data_chunks as usize;
        let parity_chunks = manifest.parity_chunks as usize;

        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut rng = rand::thread_rng();
        let mut points = Vec::new();

        // Sweep loss from 0% to 40% in 1% steps
        for loss_pct in 0..=40u32 {
            let loss_rate = loss_pct as f32 / 100.0;

            let mut tcp_successes: u32 = 0;
            let mut resilient_successes: u32 = 0;
            let mut tcp_total_lost: u32 = 0;
            let mut resilient_total_lost: u32 = 0;
            let mut resilient_total_recovered: u32 = 0;

            for _ in 0..trials_per_point {
                let mut lost: u32 = 0;
                for _ in 0..total_chunks {
                    if rng.gen::<f32>() < loss_rate {
                        lost += 1;
                    }
                }

                // TCP: no FEC, any lost chunk means the file is incomplete
                if lost == 0 {
                    tcp_successes += 1;
                }
                tcp_total_lost += lost;

                // RESILIENT: can tolerate up to parity_chunks lost
                let surviving = total_chunks - lost;
                if surviving as usize >= data_chunks {
                    resilient_successes += 1;
                    resilient_total_recovered += lost;
                } else {
                    let max_rec = (parity_chunks as u32).min(lost);
                    resilient_total_recovered += max_rec;
                }
                resilient_total_lost += lost;
            }

            let t = trials_per_point as f64;
            points.push(ComparisonPoint {
                loss_percent: loss_pct,
                tcp_success_rate: tcp_successes as f64 / t * 100.0,
                resilient_success_rate: resilient_successes as f64 / t * 100.0,
                tcp_avg_chunks_lost: tcp_total_lost as f64 / t,
                resilient_avg_chunks_lost: resilient_total_lost as f64 / t,
                resilient_avg_recovered: resilient_total_recovered as f64 / t,
            });
        }

        Ok(ComparisonResult {
            file_name,
            file_size_bytes: manifest.total_size,
            total_chunks,
            data_chunks,
            parity_chunks,
            trials_per_point,
            points,
        })
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Transfer worker - handles chunk transfer loop
    async fn transfer_worker(
        &self,
        session_id: String,
        manifest: FileManifest,
        chunks: Vec<Chunk>,
        receiver_addr: Option<SocketAddr>,
    ) -> CoordinatorResult<()> {
        let state_machine = self
            .active_transfers
            .get(&session_id)
            .ok_or_else(|| CoordinatorError::TransferNotFound(session_id.clone()))?
            .clone();

        // Get resume info
        let session = self
            .session_store
            .load(&session_id)
            .await?
            .ok_or_else(|| CoordinatorError::TransferNotFound(session_id.clone()))?;

        let completed_set = &session.completed_chunks;
        let mut chunks_to_transfer: Vec<u32> = (0..manifest.total_chunks)
            .filter(|n| !completed_set.contains(n))
            .collect();

        // Enqueue chunks (only if we have them)
        for chunk in chunks {
            if !completed_set.contains(&chunk.metadata.sequence_number) {
                self.queue.enqueue(chunk)?;
            }
        }

        // Signal first chunk completed to transition state
        if !chunks_to_transfer.is_empty() {
            state_machine.transition(TransferEvent::ChunkCompleted { chunk_number: 0 })?;
        }

        // Establish connection once if receiver address provided
        let connection = if let Some(addr) = receiver_addr {
            println!("Connecting to receiver at {addr}...");
            match self.transport.connect(addr).await {
                Ok(conn) => {
                    println!("Connected to receiver at {addr}");
                    Some(conn)
                }
                Err(e) => {
                    eprintln!("Failed to connect to receiver at {addr}: {e}");
                    state_machine.transition(TransferEvent::NetworkFailure {
                        path_id: "default".to_string(),
                    })?;
                    return Err(e.into());
                }
            }
        } else {
            None
        };

        // Transfer loop
        while !chunks_to_transfer.is_empty() {
            // Check if paused or cancelled
            let current_state = state_machine.current_state();
            if current_state.is_paused() {
                self.session_store
                    .update_status(&session_id, SessionStatus::Paused)
                    .await?;
                break;
            }
            if current_state.is_terminal() {
                break;
            }

            // Dequeue next chunk
            match self.queue.dequeue() {
                Ok(chunk) => {
                    let chunk_num = chunk.metadata.sequence_number;
                    let chunk_bytes = chunk.data.len() as u64;

                    // Actually send chunk over network (if connection established)
                    if let Some(ref conn) = connection {
                        // Send with retry (max 3 attempts)
                        if let Err(e) = self.transport.send_with_retry(conn, &chunk, 3).await {
                            eprintln!("Failed to send chunk {chunk_num}: {e}");
                            // Mark as failed but continue
                            self.session_store
                                .mark_chunk_failed(&session_id, chunk_num)
                                .await?;
                            continue;
                        }
                        // Update real QUIC stats after each chunk for live dashboard
                        let quic_stats = QuicTransport::connection_stats(conn);
                        *self.last_quic_stats.write() = quic_stats;
                    } else {
                        // No receiver address - simulate for local testing
                        time::sleep(Duration::from_millis(10)).await;
                    }

                    // Mark as completed with actual bytes transferred
                    self.session_store
                        .mark_chunk_completed_with_bytes(&session_id, chunk_num, chunk_bytes)
                        .await?;

                    // Update state
                    state_machine.transition(TransferEvent::ChunkCompleted {
                        chunk_number: chunk_num,
                    })?;

                    // Remove from list
                    chunks_to_transfer.retain(|n| *n != chunk_num);

                    // Check if all chunks transferred
                    if chunks_to_transfer.is_empty() {
                        state_machine.transition(TransferEvent::TransferComplete)?;
                        break;
                    }
                }
                Err(crate::priority::QueueError::QueueEmpty) => {
                    // No chunks available, wait a bit
                    time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }

        // Capture real QUIC stats from the connection after transfer
        if let Some(ref conn) = connection {
            let quic_stats = QuicTransport::connection_stats(conn);
            *self.last_quic_stats.write() = quic_stats;
        }

        // Check if completed
        let session = self
            .session_store
            .load(&session_id)
            .await?
            .ok_or_else(|| CoordinatorError::TransferNotFound(session_id.clone()))?;

        if session.status == SessionStatus::Completed {
            state_machine.transition(TransferEvent::TransferComplete)?;
            self.active_transfers.remove(&session_id);
            // Remove file-to-session mapping so the same file can be re-uploaded
            self.file_to_session.remove(&session.file_id);
            // Keep in recent_transfers for display
        }

        Ok(())
    }
}

impl Clone for TransferCoordinator {
    fn clone(&self) -> Self {
        Self {
            chunk_manager: self.chunk_manager.clone(),
            verifier: self.verifier.clone(),
            transport: self.transport.clone(),
            queue: self.queue.clone(),
            session_store: self.session_store.clone(),
            active_transfers: self.active_transfers.clone(),
            recent_transfers: self.recent_transfers.clone(),
            file_to_session: self.file_to_session.clone(),
            adaptive_coder: self.adaptive_coder.clone(),
            sim_chunks_sent: self.sim_chunks_sent.clone(),
            sim_chunks_lost: self.sim_chunks_lost.clone(),
            sim_chunks_recovered: self.sim_chunks_recovered.clone(),
            last_quic_stats: self.last_quic_stats.clone(),
            start_time: self.start_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[allow(unused_imports)]
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    async fn create_test_coordinator() -> TransferCoordinator {
        use crate::network::ConnectionConfig;

        let chunk_manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();
        let verifier = IntegrityVerifier;
        let config = ConnectionConfig::default();
        let transport = QuicTransport::new(config).await.unwrap();
        let queue = PriorityQueue::new(1_000_000);
        let session_store = SessionStore::new_in_memory().await.unwrap();

        TransferCoordinator::new(chunk_manager, verifier, transport, queue, session_store)
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator = create_test_coordinator().await;
        assert_eq!(coordinator.list_active().len(), 0);
    }

    #[tokio::test]
    async fn test_send_file() {
        let coordinator = create_test_coordinator().await;

        // Create test file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&vec![0u8; 1024]).unwrap();
        temp_file.flush().unwrap();

        let file_path = temp_file.path().to_path_buf();
        let session_id = coordinator
            .send_file(file_path, Priority::Normal, None)
            .await
            .unwrap();

        assert!(!session_id.is_empty());
        assert_eq!(coordinator.list_active().len(), 1);
    }

    #[tokio::test]
    async fn test_get_progress() {
        let coordinator = create_test_coordinator().await;

        // Create test file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&vec![0u8; 1024]).unwrap();
        temp_file.flush().unwrap();

        let file_path = temp_file.path().to_path_buf();
        let session_id = coordinator
            .send_file(file_path, Priority::Normal, None)
            .await
            .unwrap();

        // Wait a bit for worker to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        let progress = coordinator.get_progress(&session_id).await.unwrap();
        assert_eq!(progress.session_id, session_id);
        assert!(progress.total_chunks > 0);
    }

    #[tokio::test]
    async fn test_pause_resume() {
        let coordinator = create_test_coordinator().await;

        // Create test file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&vec![0u8; 10240]).unwrap();
        temp_file.flush().unwrap();

        let file_path = temp_file.path().to_path_buf();
        let session_id = coordinator
            .send_file(file_path, Priority::Normal, None)
            .await
            .unwrap();

        // Wait for transfer to start and chunks to be queued
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check that the transfer is registered
        if coordinator.get_state(&session_id).is_some() {
            // Pause
            coordinator.pause_transfer(&session_id).await.unwrap();

            let state = coordinator.get_state(&session_id).unwrap();
            assert!(state.is_paused());

            // Resume
            coordinator.resume_transfer(&session_id).await.unwrap();

            // Wait a bit
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    #[tokio::test]
    async fn test_cancel_transfer() {
        let coordinator = create_test_coordinator().await;

        // Create test file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&vec![0u8; 10240]).unwrap();
        temp_file.flush().unwrap();

        let file_path = temp_file.path().to_path_buf();
        let session_id = coordinator
            .send_file(file_path, Priority::Normal, None)
            .await
            .unwrap();

        // Cancel
        coordinator.cancel_transfer(&session_id).await.unwrap();

        // Should be removed from active transfers
        assert!(coordinator.get_state(&session_id).is_none());
    }

    #[tokio::test]
    async fn test_duplicate_transfer() {
        let coordinator = create_test_coordinator().await;

        // Create test file
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(&vec![0u8; 1024]).unwrap();
        temp_file.flush().unwrap();

        let file_path = temp_file.path().to_path_buf();
        coordinator
            .send_file(file_path.clone(), Priority::Normal, None)
            .await
            .unwrap();

        // Try to send same file again
        let result = coordinator
            .send_file(file_path, Priority::Normal, None)
            .await;
        assert!(result.is_err());
    }
}
