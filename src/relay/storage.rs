//! Relay storage for store-and-forward functionality
//!
//! Provides persistent storage for chunks waiting to be forwarded.

use crate::relay::types::{RelayError, RelayResult, RouteInfo};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// A chunk stored in the relay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredChunk {
    /// Unique chunk identifier
    pub chunk_id: String,

    /// Routing information
    pub route: RouteInfo,

    /// Chunk data
    pub data: Vec<u8>,

    /// When the chunk was stored
    #[serde(with = "system_time_serde")]
    pub stored_at: SystemTime,

    /// When the chunk expires
    #[serde(with = "system_time_serde")]
    pub expires_at: SystemTime,

    /// Number of forward attempts
    pub forward_attempts: u32,

    /// Last forward attempt time
    #[serde(skip)]
    pub last_attempt: Option<Instant>,
}

mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

impl StoredChunk {
    /// Create a new stored chunk
    pub fn new(chunk_id: String, route: RouteInfo, data: Vec<u8>, hold_time: Duration) -> Self {
        let now = SystemTime::now();
        Self {
            chunk_id,
            route,
            data,
            stored_at: now,
            expires_at: now + hold_time,
            forward_attempts: 0,
            last_attempt: None,
        }
    }

    /// Check if the chunk has expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.expires_at
    }

    /// Get the chunk's priority (from route)
    pub fn priority(&self) -> u8 {
        self.route.priority
    }

    /// Get size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Check if enough time has passed since last attempt
    pub fn can_retry(&self, cooldown: Duration) -> bool {
        match self.last_attempt {
            None => true,
            Some(last) => last.elapsed() >= cooldown,
        }
    }

    /// Record a forward attempt
    pub fn record_attempt(&mut self) {
        self.forward_attempts += 1;
        self.last_attempt = Some(Instant::now());
    }
}

/// Priority key for ordering chunks (lower = higher priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PriorityKey {
    priority: u8,
    stored_millis: u64,
}

/// Storage backend for relay chunks
pub struct RelayStorage {
    /// In-memory chunk storage
    chunks: RwLock<HashMap<String, StoredChunk>>,

    /// Priority-ordered index for forwarding
    priority_index: RwLock<BTreeMap<PriorityKey, String>>,

    /// Per-destination chunk lists
    destination_index: RwLock<HashMap<String, Vec<String>>>,

    /// Maximum storage capacity
    max_bytes: u64,

    /// Current storage usage
    used_bytes: RwLock<u64>,

    /// Optional persistence path
    persistence_path: Option<PathBuf>,

    /// Default hold time
    default_hold_time: Duration,
}

impl RelayStorage {
    /// Create a new in-memory relay storage
    pub fn new(max_bytes: u64, default_hold_time: Duration) -> Self {
        Self {
            chunks: RwLock::new(HashMap::new()),
            priority_index: RwLock::new(BTreeMap::new()),
            destination_index: RwLock::new(HashMap::new()),
            max_bytes,
            used_bytes: RwLock::new(0),
            persistence_path: None,
            default_hold_time,
        }
    }

    /// Create storage with persistence
    pub fn with_persistence(mut self, path: impl AsRef<Path>) -> RelayResult<Self> {
        let path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        self.persistence_path = Some(path);
        self.load_persisted()?;
        Ok(self)
    }

    /// Store a chunk
    pub fn store(&self, chunk_id: String, route: RouteInfo, data: Vec<u8>) -> RelayResult<()> {
        let size = data.len() as u64;

        // Check capacity
        {
            let used = *self.used_bytes.read();
            if used + size > self.max_bytes {
                return Err(RelayError::CapacityExceeded);
            }
        }

        let chunk = StoredChunk::new(
            chunk_id.clone(),
            route.clone(),
            data,
            self.default_hold_time,
        );

        let priority_key = PriorityKey {
            priority: chunk.priority(),
            stored_millis: chunk
                .stored_at
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };

        let dest_key = route.destination.to_string();
        let chunk_id_for_persist = chunk_id.clone();

        // Update indices
        {
            let mut chunks = self.chunks.write();
            let mut priority_idx = self.priority_index.write();
            let mut dest_idx = self.destination_index.write();
            let mut used = self.used_bytes.write();

            chunks.insert(chunk_id.clone(), chunk);
            priority_idx.insert(priority_key, chunk_id.clone());
            dest_idx.entry(dest_key).or_default().push(chunk_id);
            *used += size;
        }

        // Persist if enabled
        self.persist_chunk(&chunk_id_for_persist)?;

        Ok(())
    }

    /// Get a chunk by ID
    pub fn get(&self, chunk_id: &str) -> Option<StoredChunk> {
        self.chunks.read().get(chunk_id).cloned()
    }

    /// Remove a chunk (after successful forwarding)
    pub fn remove(&self, chunk_id: &str) -> Option<StoredChunk> {
        let mut chunks = self.chunks.write();
        let mut used = self.used_bytes.write();

        if let Some(chunk) = chunks.remove(chunk_id) {
            *used = used.saturating_sub(chunk.size() as u64);

            // Clean up indices
            {
                let mut priority_idx = self.priority_index.write();
                let priority_key = PriorityKey {
                    priority: chunk.priority(),
                    stored_millis: chunk
                        .stored_at
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                };
                priority_idx.remove(&priority_key);
            }

            {
                let mut dest_idx = self.destination_index.write();
                let dest_key = chunk.route.destination.to_string();
                if let Some(list) = dest_idx.get_mut(&dest_key) {
                    list.retain(|id| id != chunk_id);
                }
            }

            // Remove persisted file
            self.remove_persisted(chunk_id);

            return Some(chunk);
        }

        None
    }

    /// Get chunks ready for forwarding (by priority)
    pub fn get_pending(&self, limit: usize, cooldown: Duration) -> Vec<StoredChunk> {
        let chunks = self.chunks.read();
        let priority_idx = self.priority_index.read();

        let mut result = Vec::with_capacity(limit);

        for (_, chunk_id) in priority_idx.iter() {
            if result.len() >= limit {
                break;
            }

            if let Some(chunk) = chunks.get(chunk_id) {
                if !chunk.is_expired() && chunk.can_retry(cooldown) {
                    result.push(chunk.clone());
                }
            }
        }

        result
    }

    /// Get chunks for a specific destination
    pub fn get_for_destination(&self, dest: &str) -> Vec<StoredChunk> {
        let chunks = self.chunks.read();
        let dest_idx = self.destination_index.read();

        dest_idx
            .get(dest)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| chunks.get(id))
                    .filter(|c| !c.is_expired())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Record a forward attempt for a chunk
    pub fn record_attempt(&self, chunk_id: &str) {
        let mut chunks = self.chunks.write();
        if let Some(chunk) = chunks.get_mut(chunk_id) {
            chunk.record_attempt();
        }
    }

    /// Remove expired chunks
    pub fn cleanup_expired(&self) -> Vec<String> {
        let mut expired = Vec::new();

        {
            let chunks = self.chunks.read();
            for (id, chunk) in chunks.iter() {
                if chunk.is_expired() {
                    expired.push(id.clone());
                }
            }
        }

        for id in &expired {
            self.remove(id);
        }

        expired
    }

    /// Get current storage statistics
    pub fn stats(&self) -> StorageStats {
        let chunks = self.chunks.read();

        let mut by_priority = [0u64; 3]; // critical, high, normal
        let mut by_destination: HashMap<String, u64> = HashMap::new();

        for chunk in chunks.values() {
            let priority_idx = match chunk.priority() {
                0 => 0,
                1 => 1,
                _ => 2,
            };
            by_priority[priority_idx] += 1;

            let dest = chunk.route.destination.to_string();
            *by_destination.entry(dest).or_default() += 1;
        }

        StorageStats {
            total_chunks: chunks.len() as u64,
            used_bytes: *self.used_bytes.read(),
            max_bytes: self.max_bytes,
            chunks_by_priority: by_priority,
            destinations: by_destination.len() as u64,
        }
    }

    /// Persist a chunk to disk
    fn persist_chunk(&self, chunk_id: &str) -> RelayResult<()> {
        if let Some(ref path) = self.persistence_path {
            if let Some(chunk) = self.chunks.read().get(chunk_id) {
                let file_path = path.join(format!("{}.chunk", chunk_id));
                let data =
                    bincode::serialize(chunk).map_err(|e| RelayError::Storage(e.to_string()))?;
                std::fs::write(file_path, data)?;
            }
        }
        Ok(())
    }

    /// Remove persisted chunk file
    fn remove_persisted(&self, chunk_id: &str) {
        if let Some(ref path) = self.persistence_path {
            let file_path = path.join(format!("{}.chunk", chunk_id));
            let _ = std::fs::remove_file(file_path);
        }
    }

    /// Load persisted chunks on startup
    fn load_persisted(&self) -> RelayResult<()> {
        if let Some(ref path) = self.persistence_path {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let file_path = entry.path();

                if file_path.extension().map(|e| e == "chunk").unwrap_or(false) {
                    if let Ok(data) = std::fs::read(&file_path) {
                        if let Ok(chunk) = bincode::deserialize::<StoredChunk>(&data) {
                            // Skip expired chunks
                            if !chunk.is_expired() {
                                let _ = self.store(
                                    chunk.chunk_id.clone(),
                                    chunk.route.clone(),
                                    chunk.data.clone(),
                                );
                            } else {
                                // Remove expired persisted chunk
                                let _ = std::fs::remove_file(file_path);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_chunks: u64,
    pub used_bytes: u64,
    pub max_bytes: u64,
    pub chunks_by_priority: [u64; 3],
    pub destinations: u64,
}

impl StorageStats {
    /// Get utilization percentage
    pub fn utilization(&self) -> f64 {
        if self.max_bytes == 0 {
            return 0.0;
        }
        self.used_bytes as f64 / self.max_bytes as f64 * 100.0
    }
}

impl std::fmt::Display for StorageStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Storage: {} chunks, {:.2}MB/{:.2}MB ({:.1}%), {} destinations",
            self.total_chunks,
            self.used_bytes as f64 / 1024.0 / 1024.0,
            self.max_bytes as f64 / 1024.0 / 1024.0,
            self.utilization(),
            self.destinations
        )
    }
}

/// Builder for relay storage
pub struct RelayStorageBuilder {
    max_bytes: u64,
    hold_time: Duration,
    persistence_path: Option<PathBuf>,
}

impl RelayStorageBuilder {
    pub fn new() -> Self {
        Self {
            max_bytes: 1024 * 1024 * 1024,                // 1GB default
            hold_time: Duration::from_secs(24 * 60 * 60), // 24 hours
            persistence_path: None,
        }
    }

    pub fn max_bytes(mut self, bytes: u64) -> Self {
        self.max_bytes = bytes;
        self
    }

    pub fn hold_time(mut self, duration: Duration) -> Self {
        self.hold_time = duration;
        self
    }

    pub fn persistence_path(mut self, path: impl AsRef<Path>) -> Self {
        self.persistence_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn build(self) -> RelayResult<RelayStorage> {
        let storage = RelayStorage::new(self.max_bytes, self.hold_time);

        if let Some(path) = self.persistence_path {
            storage.with_persistence(path)
        } else {
            Ok(storage)
        }
    }
}

impl Default for RelayStorageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    fn test_route() -> RouteInfo {
        RouteInfo::new(
            "source",
            "127.0.0.1:8000".parse::<SocketAddr>().unwrap(),
            "transfer-1",
            1,
        )
    }

    #[test]
    fn test_store_and_retrieve() {
        let storage = RelayStorage::new(1024 * 1024, Duration::from_secs(60));

        storage
            .store("chunk-1".into(), test_route(), vec![1, 2, 3, 4])
            .unwrap();

        let chunk = storage.get("chunk-1").unwrap();
        assert_eq!(chunk.data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_remove() {
        let storage = RelayStorage::new(1024 * 1024, Duration::from_secs(60));

        storage
            .store("chunk-1".into(), test_route(), vec![1, 2, 3, 4])
            .unwrap();

        let removed = storage.remove("chunk-1").unwrap();
        assert_eq!(removed.chunk_id, "chunk-1");

        assert!(storage.get("chunk-1").is_none());
    }

    #[test]
    fn test_capacity_limit() {
        let storage = RelayStorage::new(10, Duration::from_secs(60));

        storage
            .store("chunk-1".into(), test_route(), vec![1, 2, 3, 4, 5])
            .unwrap();

        // This should exceed capacity
        let result = storage.store("chunk-2".into(), test_route(), vec![1, 2, 3, 4, 5, 6]);

        assert!(matches!(result, Err(RelayError::CapacityExceeded)));
    }

    #[test]
    fn test_priority_ordering() {
        let storage = RelayStorage::new(1024 * 1024, Duration::from_secs(60));

        // Store chunks with different priorities
        let mut route_critical = test_route();
        route_critical.priority = 0;

        let mut route_high = test_route();
        route_high.priority = 1;

        let mut route_normal = test_route();
        route_normal.priority = 2;

        storage
            .store("chunk-normal".into(), route_normal, vec![1])
            .unwrap();
        storage
            .store("chunk-high".into(), route_high, vec![2])
            .unwrap();
        storage
            .store("chunk-critical".into(), route_critical, vec![3])
            .unwrap();

        let pending = storage.get_pending(3, Duration::ZERO);

        assert_eq!(pending[0].chunk_id, "chunk-critical");
        assert_eq!(pending[1].chunk_id, "chunk-high");
        assert_eq!(pending[2].chunk_id, "chunk-normal");
    }

    #[test]
    fn test_storage_stats() {
        let storage = RelayStorage::new(1024 * 1024, Duration::from_secs(60));

        storage
            .store("chunk-1".into(), test_route(), vec![1, 2, 3, 4])
            .unwrap();
        storage
            .store("chunk-2".into(), test_route(), vec![5, 6, 7, 8])
            .unwrap();

        let stats = storage.stats();
        assert_eq!(stats.total_chunks, 2);
        assert_eq!(stats.used_bytes, 8);
    }

    #[test]
    fn test_expiration() {
        let storage = RelayStorage::new(1024 * 1024, Duration::from_millis(1));

        storage
            .store("chunk-1".into(), test_route(), vec![1, 2, 3, 4])
            .unwrap();

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(10));

        let expired = storage.cleanup_expired();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0], "chunk-1");

        assert!(storage.get("chunk-1").is_none());
    }
}
