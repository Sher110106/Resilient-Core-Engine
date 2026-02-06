use crate::chunk::FileManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    Initializing,
    Active,
    Paused,
    Completed,
    Failed(String),
}

impl SessionStatus {
    pub fn is_resumable(&self) -> bool {
        matches!(self, SessionStatus::Paused | SessionStatus::Failed(_))
    }

    pub fn is_active(&self) -> bool {
        matches!(self, SessionStatus::Active)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self, SessionStatus::Completed)
    }
}

/// Transfer metrics for calculating speed
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransferMetrics {
    /// Timestamp when transfer started (Unix timestamp millis)
    pub started_at_ms: i64,
    /// Last update timestamp (Unix timestamp millis)  
    pub last_update_ms: i64,
    /// Total bytes transferred so far
    pub bytes_transferred: u64,
    /// Bytes transferred in the last measurement window
    pub bytes_in_window: u64,
    /// Window start timestamp for speed calculation
    pub window_start_ms: i64,
    /// Current speed in bytes per second (rolling average)
    pub current_speed_bps: u64,
}

impl TransferMetrics {
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            started_at_ms: now,
            last_update_ms: now,
            bytes_transferred: 0,
            bytes_in_window: 0,
            window_start_ms: now,
            current_speed_bps: 0,
        }
    }

    /// Update metrics after transferring bytes
    pub fn record_bytes(&mut self, bytes: u64) {
        let now = chrono::Utc::now().timestamp_millis();
        self.bytes_transferred += bytes;
        self.bytes_in_window += bytes;
        self.last_update_ms = now;

        // Calculate speed over a 1-second rolling window
        let window_duration_ms = now - self.window_start_ms;
        if window_duration_ms >= 1000 {
            // Calculate speed: bytes / seconds
            self.current_speed_bps = (self.bytes_in_window * 1000) / window_duration_ms as u64;
            // Reset window
            self.bytes_in_window = 0;
            self.window_start_ms = now;
        }
    }

    /// Get average speed since start
    pub fn average_speed_bps(&self) -> u64 {
        let duration_ms = self.last_update_ms - self.started_at_ms;
        if duration_ms > 0 {
            (self.bytes_transferred * 1000) / duration_ms as u64
        } else {
            0
        }
    }

    /// Estimate remaining time in seconds
    pub fn estimated_remaining_secs(&self, remaining_bytes: u64) -> Option<u64> {
        if self.current_speed_bps > 0 {
            Some(remaining_bytes / self.current_speed_bps)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub file_id: String,
    pub manifest: FileManifest,
    pub completed_chunks: HashSet<u32>,
    pub failed_chunks: HashSet<u32>,
    pub status: SessionStatus,
    pub created_at: i64,
    pub updated_at: i64,
    /// The receiver address for network transfers (None for local-only transfers)
    pub receiver_addr: Option<SocketAddr>,
    /// Original file path (needed for resume to re-read chunks)
    pub file_path: Option<String>,
    /// Transfer metrics for speed calculation
    #[serde(default)]
    pub metrics: TransferMetrics,
}

impl SessionState {
    pub fn new(session_id: String, file_id: String, manifest: FileManifest) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            session_id,
            file_id,
            manifest,
            completed_chunks: HashSet::new(),
            failed_chunks: HashSet::new(),
            status: SessionStatus::Initializing,
            created_at: now,
            updated_at: now,
            receiver_addr: None,
            file_path: None,
            metrics: TransferMetrics::new(),
        }
    }

    /// Create a new session with receiver address and file path for resumable transfers
    pub fn new_with_receiver(
        session_id: String,
        file_id: String,
        manifest: FileManifest,
        receiver_addr: Option<SocketAddr>,
        file_path: Option<String>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            session_id,
            file_id,
            manifest,
            completed_chunks: HashSet::new(),
            failed_chunks: HashSet::new(),
            status: SessionStatus::Initializing,
            created_at: now,
            updated_at: now,
            receiver_addr,
            file_path,
            metrics: TransferMetrics::new(),
        }
    }

    /// Record bytes transferred and update metrics
    pub fn record_bytes_transferred(&mut self, bytes: u64) {
        self.metrics.record_bytes(bytes);
        self.updated_at = chrono::Utc::now().timestamp();
    }

    /// Get current transfer speed in bytes per second
    pub fn current_speed_bps(&self) -> u64 {
        self.metrics.current_speed_bps
    }

    pub fn progress_percent(&self) -> f32 {
        let total = self.manifest.total_chunks as f32;
        if total == 0.0 {
            return 0.0;
        }
        let completed = self.completed_chunks.len() as f32;
        let percent = (completed / total) * 100.0;
        percent.min(100.0)
    }

    pub fn remaining_chunks(&self) -> Vec<u32> {
        (0..self.manifest.total_chunks)
            .filter(|n| !self.completed_chunks.contains(n))
            .collect()
    }

    pub fn is_complete(&self) -> bool {
        self.completed_chunks.len() >= self.manifest.data_chunks as usize
    }

    pub fn mark_completed(&mut self, chunk_number: u32) {
        self.completed_chunks.insert(chunk_number);
        self.failed_chunks.remove(&chunk_number);
        self.updated_at = chrono::Utc::now().timestamp();
    }

    pub fn mark_failed(&mut self, chunk_number: u32) {
        self.failed_chunks.insert(chunk_number);
        self.updated_at = chrono::Utc::now().timestamp();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeInfo {
    pub session_id: String,
    pub total_chunks: u32,
    pub completed_chunks: u32,
    pub remaining_chunks: u32,
    pub failed_chunks: u32,
    pub progress_percent: f32,
    pub can_resume: bool,
    pub status: SessionStatus,
}

impl ResumeInfo {
    pub fn from_state(state: &SessionState) -> Self {
        let total = state.manifest.total_chunks;
        let completed = state.completed_chunks.len() as u32;
        let remaining = total - completed;
        let failed = state.failed_chunks.len() as u32;

        Self {
            session_id: state.session_id.clone(),
            total_chunks: total,
            completed_chunks: completed,
            remaining_chunks: remaining,
            failed_chunks: failed,
            progress_percent: state.progress_percent(),
            can_resume: state.status.is_resumable(),
            status: state.status.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub file_id: String,
    pub filename: String,
    pub status: SessionStatus,
    pub progress_percent: f32,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SessionSummary {
    pub fn from_state(state: &SessionState) -> Self {
        Self {
            session_id: state.session_id.clone(),
            file_id: state.file_id.clone(),
            filename: state.manifest.filename.clone(),
            status: state.status.clone(),
            progress_percent: state.progress_percent(),
            created_at: state.created_at,
            updated_at: state.updated_at,
        }
    }
}
