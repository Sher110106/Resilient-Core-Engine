use crate::chunk::{Chunk, ChunkManager, FileManifest, Priority};
use crate::coordinator::error::{CoordinatorError, CoordinatorResult};
use crate::coordinator::state_machine::TransferStateMachine;
use crate::coordinator::types::{TransferEvent, TransferProgress, TransferState};
use crate::integrity::IntegrityVerifier;
use crate::network::QuicTransport;
use crate::priority::PriorityQueue;
use crate::session::{SessionState, SessionStatus, SessionStore};
use dashmap::DashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

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
}

impl TransferCoordinator {
    pub fn new(
        chunk_manager: ChunkManager,
        verifier: IntegrityVerifier,
        transport: QuicTransport,
        queue: PriorityQueue,
        session_store: SessionStore,
    ) -> Self {
        Self {
            chunk_manager: Arc::new(chunk_manager),
            verifier: Arc::new(verifier),
            transport: Arc::new(transport),
            queue: Arc::new(queue),
            session_store: Arc::new(session_store),
            active_transfers: Arc::new(DashMap::new()),
            recent_transfers: Arc::new(DashMap::new()),
            file_to_session: Arc::new(DashMap::new()),
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

        // Create session
        let session_id = uuid::Uuid::new_v4().to_string();
        let session = SessionState::new(session_id.clone(), file_id.clone(), manifest.clone());
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
        self.file_to_session.insert(file_id, session_id.clone());

        // Start transfer worker
        let coordinator = self.clone();
        let worker_session_id = session_id.clone();
        tokio::spawn(async move {
            if let Err(e) = coordinator
                .transfer_worker(worker_session_id.clone(), manifest, chunks, receiver_addr)
                .await
            {
                eprintln!("Transfer worker failed for {worker_session_id}: {e}");
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

        // Start transfer worker
        let coordinator = self.clone();
        let session_id_str = session_id.to_string();
        let manifest = session.manifest.clone();
        // Resume doesn't have chunks, so pass empty vec (worker will skip enqueueing)
        // TODO: We don't have receiver_addr stored in session, so resume won't work for network transfers
        tokio::spawn(async move {
            if let Err(e) = coordinator
                .transfer_worker(session_id_str.clone(), manifest, vec![], None)
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

        state_machine.send_event(TransferEvent::Pause)?;
        self.session_store
            .update_status(session_id, SessionStatus::Paused)
            .await?;

        Ok(())
    }

    /// Cancel a transfer
    pub async fn cancel_transfer(&self, session_id: &str) -> CoordinatorResult<()> {
        if let Some(state_machine) = self.active_transfers.get(session_id) {
            state_machine.send_event(TransferEvent::Cancel)?;
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
        let bytes_per_chunk = session.manifest.chunk_size as u64;

        Ok(TransferProgress {
            session_id: session_id.to_string(),
            completed_chunks: completed,
            total_chunks: total,
            bytes_transferred: completed as u64 * bytes_per_chunk,
            total_bytes: session.manifest.total_size,
            progress_percent: session.progress_percent(),
            status: session.status,
            current_speed_bps: 0, // Would calculate from real metrics
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
            match self.transport.connect(addr).await {
                Ok(conn) => Some(conn),
                Err(e) => {
                    eprintln!("Failed to connect to receiver: {e}");
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
                    } else {
                        // No receiver address - simulate for local testing
                        time::sleep(Duration::from_millis(10)).await;
                    }

                    // Mark as completed
                    self.session_store
                        .mark_chunk_completed(&session_id, chunk_num)
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

        // Check if completed
        let session = self
            .session_store
            .load(&session_id)
            .await?
            .ok_or_else(|| CoordinatorError::TransferNotFound(session_id.clone()))?;

        if session.status == SessionStatus::Completed {
            state_machine.transition(TransferEvent::TransferComplete)?;
            self.active_transfers.remove(&session_id);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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
