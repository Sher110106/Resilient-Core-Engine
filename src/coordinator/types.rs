use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransferState {
    Idle,
    Preparing,
    Transferring { progress: f32 },
    Paused { reason: String },
    Completing,
    Completed,
    Failed { error: String },
}

impl TransferState {
    pub fn is_active(&self) -> bool {
        matches!(self, TransferState::Transferring { .. })
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, TransferState::Paused { .. })
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TransferState::Completed | TransferState::Failed { .. }
        )
    }
}

#[derive(Debug, Clone)]
pub enum TransferEvent {
    Start {
        file_path: PathBuf,
        priority: crate::chunk::Priority,
    },
    ChunkCompleted {
        chunk_number: u32,
    },
    ChunkFailed {
        chunk_number: u32,
        error: String,
    },
    Pause,
    Resume,
    Cancel,
    NetworkFailure {
        path_id: String,
    },
    NetworkRecovered {
        path_id: String,
    },
    TransferComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub session_id: String,
    pub completed_chunks: u32,
    pub total_chunks: u32,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub progress_percent: f32,
    pub status: crate::session::SessionStatus,
    pub current_speed_bps: u64,
}
