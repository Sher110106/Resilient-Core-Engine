use crate::chunk::FileManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
        }
    }

    pub fn progress_percent(&self) -> f32 {
        let total = self.manifest.data_chunks as f32;
        let completed = self.completed_chunks.len() as f32;
        (completed / total) * 100.0
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
