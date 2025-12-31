use crate::session::error::{SessionError, SessionResult};
use crate::session::types::{ResumeInfo, SessionState, SessionStatus, SessionSummary};
use sqlx::{Row, SqlitePool};

pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    /// Create new session store with SQLite database
    pub async fn new(db_path: &str) -> SessionResult<Self> {
        let pool = SqlitePool::connect(db_path).await?;
        
        // Initialize schema
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                file_id TEXT NOT NULL,
                manifest TEXT NOT NULL,
                completed_chunks TEXT NOT NULL,
                failed_chunks TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await?;

        // Create indexes for common queries
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status)")
            .execute(&pool)
            .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at)")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    /// Create session store with in-memory database (for testing)
    pub async fn new_in_memory() -> SessionResult<Self> {
        Self::new("sqlite::memory:").await
    }

    /// Save or update session state
    pub async fn save(&self, state: &SessionState) -> SessionResult<()> {
        let manifest_json = serde_json::to_string(&state.manifest)?;
        let completed_json = serde_json::to_string(&state.completed_chunks)?;
        let failed_json = serde_json::to_string(&state.failed_chunks)?;
        let status_json = serde_json::to_string(&state.status)?;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO sessions
            (session_id, file_id, manifest, completed_chunks, failed_chunks, status, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&state.session_id)
        .bind(&state.file_id)
        .bind(manifest_json)
        .bind(completed_json)
        .bind(failed_json)
        .bind(status_json)
        .bind(state.created_at)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load session state by ID
    pub async fn load(&self, session_id: &str) -> SessionResult<Option<SessionState>> {
        let row = sqlx::query(
            "SELECT * FROM sessions WHERE session_id = ?"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let state = SessionState {
                session_id: row.try_get("session_id")?,
                file_id: row.try_get("file_id")?,
                manifest: serde_json::from_str(&row.try_get::<String, _>("manifest")?)?,
                completed_chunks: serde_json::from_str(&row.try_get::<String, _>("completed_chunks")?)?,
                failed_chunks: serde_json::from_str(&row.try_get::<String, _>("failed_chunks")?)?,
                status: serde_json::from_str(&row.try_get::<String, _>("status")?)?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    /// Mark chunk as completed
    pub async fn mark_chunk_completed(&self, session_id: &str, chunk_number: u32) -> SessionResult<()> {
        let mut state = self.load(session_id)
            .await?
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        state.mark_completed(chunk_number);
        
        // Auto-complete session if all chunks are done
        if state.is_complete() && !state.status.is_completed() {
            state.status = SessionStatus::Completed;
        }
        
        self.save(&state).await
    }

    /// Mark chunk as failed
    pub async fn mark_chunk_failed(&self, session_id: &str, chunk_number: u32) -> SessionResult<()> {
        let mut state = self.load(session_id)
            .await?
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        state.mark_failed(chunk_number);
        self.save(&state).await
    }

    /// Update session status
    pub async fn update_status(&self, session_id: &str, status: SessionStatus) -> SessionResult<()> {
        let mut state = self.load(session_id)
            .await?
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        state.status = status;
        state.updated_at = chrono::Utc::now().timestamp();
        self.save(&state).await
    }

    /// Get resume information
    pub async fn get_resume_info(&self, session_id: &str) -> SessionResult<ResumeInfo> {
        let state = self.load(session_id)
            .await?
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        Ok(ResumeInfo::from_state(&state))
    }

    /// List all sessions
    pub async fn list_all(&self) -> SessionResult<Vec<SessionSummary>> {
        let rows = sqlx::query("SELECT * FROM sessions ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;

        let mut summaries = Vec::new();
        for row in rows {
            let state = SessionState {
                session_id: row.try_get("session_id")?,
                file_id: row.try_get("file_id")?,
                manifest: serde_json::from_str(&row.try_get::<String, _>("manifest")?)?,
                completed_chunks: serde_json::from_str(&row.try_get::<String, _>("completed_chunks")?)?,
                failed_chunks: serde_json::from_str(&row.try_get::<String, _>("failed_chunks")?)?,
                status: serde_json::from_str(&row.try_get::<String, _>("status")?)?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            summaries.push(SessionSummary::from_state(&state));
        }

        Ok(summaries)
    }

    /// List sessions by status
    pub async fn list_by_status(&self, status: SessionStatus) -> SessionResult<Vec<SessionSummary>> {
        let status_json = serde_json::to_string(&status)?;
        
        let rows = sqlx::query("SELECT * FROM sessions WHERE status = ? ORDER BY updated_at DESC")
            .bind(status_json)
            .fetch_all(&self.pool)
            .await?;

        let mut summaries = Vec::new();
        for row in rows {
            let state = SessionState {
                session_id: row.try_get("session_id")?,
                file_id: row.try_get("file_id")?,
                manifest: serde_json::from_str(&row.try_get::<String, _>("manifest")?)?,
                completed_chunks: serde_json::from_str(&row.try_get::<String, _>("completed_chunks")?)?,
                failed_chunks: serde_json::from_str(&row.try_get::<String, _>("failed_chunks")?)?,
                status: serde_json::from_str(&row.try_get::<String, _>("status")?)?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };
            summaries.push(SessionSummary::from_state(&state));
        }

        Ok(summaries)
    }

    /// Delete session
    pub async fn delete(&self, session_id: &str) -> SessionResult<bool> {
        let result = sqlx::query("DELETE FROM sessions WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Clean up old sessions
    pub async fn cleanup_old_sessions(&self, days: i64) -> SessionResult<u64> {
        let cutoff = chrono::Utc::now().timestamp() - (days * 86400);
        
        // Get all old sessions and filter by status in Rust
        let rows = sqlx::query("SELECT session_id, status FROM sessions WHERE updated_at < ?")
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;
        
        let mut deleted = 0u64;
        for row in rows {
            let session_id: String = row.try_get("session_id")?;
            let status_str: String = row.try_get("status")?;
            let status: SessionStatus = serde_json::from_str(&status_str)?;
            
            // Only delete completed or failed sessions
            if matches!(status, SessionStatus::Completed | SessionStatus::Failed(_)) {
                let result = sqlx::query("DELETE FROM sessions WHERE session_id = ?")
                    .bind(&session_id)
                    .execute(&self.pool)
                    .await?;
                deleted += result.rows_affected();
            }
        }

        Ok(deleted)
    }

    /// Get session count
    pub async fn count(&self) -> SessionResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM sessions")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(row.try_get("count")?)
    }

    /// Check if session exists
    pub async fn exists(&self, session_id: &str) -> SessionResult<bool> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM sessions WHERE session_id = ?")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await?;
        
        let count: i64 = row.try_get("count")?;
        Ok(count > 0)
    }

    /// Close database connection
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{FileManifest, Priority};

    fn create_test_manifest() -> FileManifest {
        FileManifest {
            file_id: "test-file".to_string(),
            filename: "test.bin".to_string(),
            total_size: 1024 * 1024,
            chunk_size: 256 * 1024,
            total_chunks: 13,
            data_chunks: 10,
            parity_chunks: 3,
            priority: Priority::Normal,
            checksum: [0u8; 32],
        }
    }

    #[tokio::test]
    async fn test_store_creation() {
        let store = SessionStore::new_in_memory().await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);

        store.save(&state).await.unwrap();
        
        let loaded = store.load("test-session").await.unwrap().unwrap();
        assert_eq!(loaded.session_id, state.session_id);
        assert_eq!(loaded.file_id, state.file_id);
        assert_eq!(loaded.status, SessionStatus::Initializing);
    }

    #[tokio::test]
    async fn test_mark_chunk_completed() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let mut state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);
        state.status = SessionStatus::Active;

        store.save(&state).await.unwrap();
        
        store.mark_chunk_completed("test-session", 0).await.unwrap();
        store.mark_chunk_completed("test-session", 1).await.unwrap();
        
        let loaded = store.load("test-session").await.unwrap().unwrap();
        assert_eq!(loaded.completed_chunks.len(), 2);
        assert!(loaded.completed_chunks.contains(&0));
        assert!(loaded.completed_chunks.contains(&1));
    }

    #[tokio::test]
    async fn test_auto_complete() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let mut state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);
        state.status = SessionStatus::Active;

        store.save(&state).await.unwrap();
        
        // Complete all data chunks (10 chunks)
        for i in 0..10 {
            store.mark_chunk_completed("test-session", i).await.unwrap();
        }
        
        let loaded = store.load("test-session").await.unwrap().unwrap();
        assert_eq!(loaded.status, SessionStatus::Completed);
    }

    #[tokio::test]
    async fn test_mark_chunk_failed() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);

        store.save(&state).await.unwrap();
        
        store.mark_chunk_failed("test-session", 5).await.unwrap();
        
        let loaded = store.load("test-session").await.unwrap().unwrap();
        assert_eq!(loaded.failed_chunks.len(), 1);
        assert!(loaded.failed_chunks.contains(&5));
    }

    #[tokio::test]
    async fn test_update_status() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);

        store.save(&state).await.unwrap();
        
        store.update_status("test-session", SessionStatus::Paused).await.unwrap();
        
        let loaded = store.load("test-session").await.unwrap().unwrap();
        assert_eq!(loaded.status, SessionStatus::Paused);
    }

    #[tokio::test]
    async fn test_resume_info() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let mut state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);
        state.completed_chunks.insert(0);
        state.completed_chunks.insert(1);
        state.completed_chunks.insert(2);
        state.status = SessionStatus::Paused;

        store.save(&state).await.unwrap();
        
        let resume_info = store.get_resume_info("test-session").await.unwrap();
        assert_eq!(resume_info.completed_chunks, 3);
        assert_eq!(resume_info.total_chunks, 13);
        assert_eq!(resume_info.remaining_chunks, 10);
        assert!(resume_info.can_resume);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let store = SessionStore::new_in_memory().await.unwrap();
        
        for i in 0..5 {
            let manifest = create_test_manifest();
            let state = SessionState::new(format!("session-{}", i), "test-file".to_string(), manifest);
            store.save(&state).await.unwrap();
        }
        
        let sessions = store.list_all().await.unwrap();
        assert_eq!(sessions.len(), 5);
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let store = SessionStore::new_in_memory().await.unwrap();
        
        for i in 0..3 {
            let manifest = create_test_manifest();
            let mut state = SessionState::new(format!("session-{}", i), "test-file".to_string(), manifest);
            state.status = if i % 2 == 0 { SessionStatus::Active } else { SessionStatus::Paused };
            store.save(&state).await.unwrap();
        }
        
        let active = store.list_by_status(SessionStatus::Active).await.unwrap();
        assert_eq!(active.len(), 2);
        
        let paused = store.list_by_status(SessionStatus::Paused).await.unwrap();
        assert_eq!(paused.len(), 1);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);

        store.save(&state).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);
        
        let deleted = store.delete("test-session").await.unwrap();
        assert!(deleted);
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_exists() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        let state = SessionState::new("test-session".to_string(), "test-file".to_string(), manifest);

        assert!(!store.exists("test-session").await.unwrap());
        
        store.save(&state).await.unwrap();
        assert!(store.exists("test-session").await.unwrap());
    }

    #[tokio::test]
    async fn test_cleanup_old_sessions() {
        let store = SessionStore::new_in_memory().await.unwrap();
        let manifest = create_test_manifest();
        
        // Create completed session with old timestamp
        let mut state1 = SessionState::new("old-session".to_string(), "file1".to_string(), manifest.clone());
        state1.status = SessionStatus::Completed;
        state1.updated_at = chrono::Utc::now().timestamp() - (10 * 86400); // 10 days old
        
        // Manually insert with old timestamp (save() updates updated_at)
        let manifest_json = serde_json::to_string(&state1.manifest).unwrap();
        let completed_json = serde_json::to_string(&state1.completed_chunks).unwrap();
        let failed_json = serde_json::to_string(&state1.failed_chunks).unwrap();
        let status_json = serde_json::to_string(&state1.status).unwrap();
        
        sqlx::query(
            "INSERT INTO sessions (session_id, file_id, manifest, completed_chunks, failed_chunks, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&state1.session_id)
        .bind(&state1.file_id)
        .bind(manifest_json)
        .bind(completed_json)
        .bind(failed_json)
        .bind(status_json)
        .bind(state1.created_at)
        .bind(state1.updated_at) // Use old timestamp
        .execute(&store.pool)
        .await
        .unwrap();
        
        // Create active session (should not be deleted)
        let state2 = SessionState::new("active-session".to_string(), "file2".to_string(), manifest);
        store.save(&state2).await.unwrap();
        
        let cleaned = store.cleanup_old_sessions(7).await.unwrap();
        assert_eq!(cleaned, 1);
        
        assert!(!store.exists("old-session").await.unwrap());
        assert!(store.exists("active-session").await.unwrap());
    }
}
