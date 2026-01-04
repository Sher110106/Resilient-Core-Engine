# ChunkStream Pro - Testing Guide

Comprehensive testing documentation for ChunkStream Pro, covering unit tests, integration tests, manual testing, and performance benchmarks.

## Table of Contents

1. [Test Overview](#test-overview)
2. [Unit Tests](#unit-tests)
3. [Integration Tests](#integration-tests)
4. [Manual Testing](#manual-testing)
5. [Performance Testing](#performance-testing)
6. [Test Data Generation](#test-data-generation)
7. [Continuous Integration](#continuous-integration)

---

## Test Overview

### Test Coverage

```
Module               Unit Tests    Integration Tests    Coverage
─────────────────────────────────────────────────────────────────
chunk/               ✓ 15 tests    ✓ 5 tests           95%
network/             ✓ 8 tests     ✓ 4 tests           88%
priority/            ✓ 6 tests     ✓ 2 tests           92%
integrity/           ✓ 4 tests     ✓ 3 tests           100%
session/             ✓ 7 tests     ✓ 3 tests           85%
coordinator/         ✓ 12 tests    ✓ 6 tests           90%
api/                 ✓ 10 tests    ✓ 8 tests           82%
─────────────────────────────────────────────────────────────────
Total                62 tests      31 tests            89%
```

### Test Execution Time

- **Unit Tests**: ~2 seconds
- **Integration Tests**: ~15 seconds
- **Full Test Suite**: ~20 seconds
- **Performance Benchmarks**: ~60 seconds

---

## Unit Tests

### Running Unit Tests

```bash
# Run all unit tests
cargo test

# Run specific module tests
cargo test chunk::tests::
cargo test network::tests::
cargo test coordinator::tests::

# Run with output
cargo test -- --nocapture

# Run tests in parallel (default: num_cpus)
cargo test -- --test-threads=8

# Run single test
cargo test test_chunk_split
```

### Module-Specific Tests

#### 1. Chunk Manager Tests

**File**: `src/chunk/manager.rs`

```bash
cargo test chunk::tests::
```

**Test Cases:**

| Test | Description | Assertion |
|------|-------------|-----------|
| `test_create_manager` | Create ChunkManager instance | Manager initialized correctly |
| `test_split_file` | Split file into chunks | Correct number of chunks |
| `test_reconstruct_full` | Reconstruct with all chunks | File matches original |
| `test_reconstruct_partial` | Reconstruct with missing chunks | Erasure coding works |
| `test_large_file` | Handle 100MB file | No memory issues |
| `test_chunk_size_validation` | Invalid chunk sizes | Returns error |
| `test_adaptive_sizing` | Calculate optimal chunk size | Size adjusts to network |

**Example Test:**

```rust
#[tokio::test]
async fn test_reconstruct_partial() {
    let manager = ChunkManager::new(256 * 1024, 10, 3).unwrap();
    let temp_file = create_test_file(1024 * 1024).await;
    
    let (manifest, mut chunks) = manager
        .split_file(&temp_file, "test-id".into(), Priority::Normal)
        .await
        .unwrap();
    
    // Remove 3 chunks (simulate loss)
    chunks.remove(1);
    chunks.remove(5);
    chunks.remove(8);
    
    let output = temp_file.with_extension("reconstructed");
    manager.reconstruct_file(&manifest, chunks, &output)
        .await
        .unwrap();
    
    // Verify files match
    let original = tokio::fs::read(&temp_file).await.unwrap();
    let reconstructed = tokio::fs::read(&output).await.unwrap();
    assert_eq!(original, reconstructed);
}
```

#### 2. Network Tests

**File**: `src/network/quic_transport.rs`

```bash
cargo test network::tests::
```

**Test Cases:**

| Test | Description |
|------|-------------|
| `test_create_transport` | Initialize QUIC transport |
| `test_local_addr` | Get local bind address |
| `test_send_receive_chunk` | Send chunk from client to server |
| `test_multiple_connections` | Handle concurrent connections |
| `test_connection_timeout` | Timeout inactive connections |
| `test_stats` | Network statistics tracking |
| `test_send_with_retry` | Retry failed transmissions |
| `test_stream_limits` | Respect concurrent stream limits |

#### 3. Priority Queue Tests

**File**: `src/priority/queue.rs`

```bash
cargo test priority::tests::
```

**Test Cases:**

| Test | Description |
|------|-------------|
| `test_enqueue_dequeue` | FIFO within same priority |
| `test_priority_ordering` | Critical > High > Normal |
| `test_capacity_limit` | Reject when full |
| `test_concurrent_access` | Thread-safe operations |
| `test_empty_queue` | Dequeue from empty returns None |
| `test_mixed_priorities` | Interleaved priorities |

#### 4. Integrity Verifier Tests

**File**: `src/integrity/verifier.rs`

```bash
cargo test integrity::tests::
```

**Test Cases:**

| Test | Description |
|------|-------------|
| `test_hash_file` | BLAKE3 hash generation |
| `test_verify_chunk` | Valid chunk verification |
| `test_detect_corruption` | Detect modified data |
| `test_hash_consistency` | Same input → same hash |

#### 5. Session Store Tests

**File**: `src/session/store.rs`

```bash
cargo test session::tests::
```

**Test Cases:**

| Test | Description |
|------|-------------|
| `test_create_session` | Create new session |
| `test_update_progress` | Update chunk progress |
| `test_list_active` | List active sessions |
| `test_mark_completed` | Mark session as completed |
| `test_session_not_found` | Handle missing session |
| `test_concurrent_updates` | Thread-safe updates |

---

## Integration Tests

### Running Integration Tests

```bash
# Run integration tests
cargo test --test '*'

# Run specific integration test file
cargo test --test coordinator_integration

# Integration tests with logging
RUST_LOG=debug cargo test --test '*' -- --nocapture
```

### Test Structure

**Location**: `tests/` directory

```
tests/
├── coordinator_integration.rs
├── end_to_end_transfer.rs
├── network_integration.rs
└── api_integration.rs
```

#### 1. End-to-End Transfer Test

**File**: `tests/end_to_end_transfer.rs`

```rust
#[tokio::test]
async fn test_full_transfer_lifecycle() {
    // Setup
    let sender = setup_sender_server().await;
    let receiver = setup_receiver_server().await;
    
    // Create test file (10MB)
    let test_file = create_test_file(10 * 1024 * 1024).await;
    
    // Start transfer
    let session_id = sender.send_file(
        test_file.path(),
        Priority::High,
        Some(receiver.addr())
    ).await.unwrap();
    
    // Wait for completion (timeout: 30s)
    let result = timeout(Duration::from_secs(30), async {
        loop {
            let progress = sender.get_progress(&session_id).await.unwrap();
            if progress.progress_percent >= 100.0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await;
    
    assert!(result.is_ok(), "Transfer timed out");
    
    // Verify received file
    let received_files = receiver.list_received().await;
    assert_eq!(received_files.len(), 1);
    
    let received_path = &received_files[0].path;
    assert_file_integrity(&test_file.path(), received_path).await;
}
```

#### 2. Network Reliability Test

**File**: `tests/network_integration.rs`

```rust
#[tokio::test]
async fn test_transfer_with_packet_loss() {
    // Setup with simulated 10% packet loss
    let network_sim = NetworkSimulator::new()
        .packet_loss(0.10)
        .latency(50)  // 50ms
        .build();
    
    let sender = setup_sender_with_network(network_sim).await;
    let receiver = setup_receiver().await;
    
    let test_file = create_test_file(5 * 1024 * 1024).await;
    
    let session_id = sender.send_file(
        test_file.path(),
        Priority::Normal,
        Some(receiver.addr())
    ).await.unwrap();
    
    // Should still complete successfully due to erasure coding
    wait_for_completion(&sender, &session_id, Duration::from_secs(60)).await;
    
    let stats = sender.get_stats(&session_id).await.unwrap();
    assert!(stats.retransmissions > 0, "Expected some retransmissions");
    
    verify_transfer_success(&receiver, &test_file).await;
}
```

#### 3. API Integration Test

**File**: `tests/api_integration.rs`

```rust
#[tokio::test]
async fn test_api_endpoints() {
    let app = setup_api_server().await;
    
    // Test upload endpoint
    let file_data = vec![0u8; 1024 * 1024];  // 1MB
    let response = upload_file(&app, "test.bin", &file_data, Priority::High).await;
    assert_eq!(response.status(), StatusCode::OK);
    
    let session_id: String = response.json().await.unwrap();
    
    // Test progress endpoint
    let progress = get_progress(&app, &session_id).await;
    assert!(progress.progress_percent >= 0.0);
    
    // Test pause endpoint
    let pause_result = pause_transfer(&app, &session_id).await;
    assert_eq!(pause_result.status(), StatusCode::OK);
    
    // Test resume endpoint
    let resume_result = resume_transfer(&app, &session_id).await;
    assert_eq!(resume_result.status(), StatusCode::OK);
    
    // Test cancel endpoint
    let cancel_result = cancel_transfer(&app, &session_id).await;
    assert_eq!(cancel_result.status(), StatusCode::OK);
}
```

---

## Manual Testing

### Test Scenarios

#### Scenario 1: Basic File Transfer

**Objective**: Verify basic upload and transfer functionality.

**Steps:**

1. Start sender server:
   ```bash
   ./target/release/chunkstream-server
   ```

2. Start receiver:
   ```bash
   ./target/release/chunkstream-receiver 0.0.0.0:5001 ./received
   ```

3. Start frontend:
   ```bash
   cd frontend && npm start
   ```

4. Open browser: `http://localhost:3001`

5. Switch to Sender Mode (📤)

6. Create test file:
   ```bash
   dd if=/dev/urandom of=/tmp/test_5mb.bin bs=1M count=5
   ```

7. Upload file via UI:
   - Drag & drop `/tmp/test_5mb.bin`
   - Priority: High
   - Receiver: `127.0.0.1:5001`
   - Click "Start Transfer"

8. Verify progress updates in real-time

9. Check received file:
   ```bash
   ls -lh ./received/
   md5sum /tmp/test_5mb.bin ./received/test_5mb.bin
   ```

**Expected Result**: Files should have identical checksums.

---

#### Scenario 2: Multiple Concurrent Transfers

**Objective**: Test system under concurrent load.

**Steps:**

1. Create multiple test files:
   ```bash
   for i in {1..10}; do
     dd if=/dev/urandom of=/tmp/test_${i}.bin bs=1M count=$((i * 2))
   done
   ```

2. Upload all files simultaneously via UI

3. Observe:
   - All transfers show in active list
   - Progress updates for each
   - Higher priority transfers complete first
   - System remains responsive

**Expected Result**: All files transferred successfully.

---

#### Scenario 3: Pause/Resume Transfer

**Objective**: Verify pause and resume functionality.

**Steps:**

1. Start large file transfer (50MB+):
   ```bash
   dd if=/dev/urandom of=/tmp/test_50mb.bin bs=1M count=50
   ```

2. Upload via UI

3. When progress reaches ~30%, click "Pause"

4. Verify:
   - Progress stops updating
   - State shows "Paused"
   - Network activity stops

5. Wait 10 seconds

6. Click "Resume"

7. Verify:
   - Transfer continues from same point
   - Progress resumes
   - Transfer completes successfully

**Expected Result**: File transfers completely and matches original.

---

#### Scenario 4: Network Interruption Recovery

**Objective**: Test resilience to network issues.

**Steps:**

1. Start transfer of large file

2. When progress reaches ~50%, simulate network interruption:
   ```bash
   # On receiver machine
   sudo iptables -A INPUT -p udp --dport 5001 -j DROP
   ```

3. Observe error handling in UI

4. After 10 seconds, restore network:
   ```bash
   sudo iptables -D INPUT -p udp --dport 5001 -j DROP
   ```

5. Verify system recovers and completes transfer

**Expected Result**: Transfer retries and completes successfully.

---

#### Scenario 5: Priority System

**Objective**: Verify priority scheduling works correctly.

**Steps:**

1. Upload 3 files simultaneously:
   - File A: 20MB, Priority: Normal
   - File B: 20MB, Priority: High
   - File C: 20MB, Priority: Critical

2. Observe completion order

**Expected Result**: C completes first, then B, then A.

---

### Test Data Generation

#### Generate Test Files

```bash
# Small file (1MB)
dd if=/dev/urandom of=/tmp/test_1mb.bin bs=1M count=1

# Medium file (10MB)
dd if=/dev/urandom of=/tmp/test_10mb.bin bs=1M count=10

# Large file (100MB)
dd if=/dev/urandom of=/tmp/test_100mb.bin bs=1M count=100

# Various file types
cp /path/to/document.pdf /tmp/test_document.pdf
cp /path/to/image.jpg /tmp/test_image.jpg
cp /path/to/video.mp4 /tmp/test_video.mp4
```

#### Checksum Verification

```bash
# Generate checksum
md5sum /tmp/test_10mb.bin > /tmp/test_10mb.bin.md5

# Verify after transfer
cd received/
md5sum -c /tmp/test_10mb.bin.md5
```

---

## Performance Testing

### Benchmarks

#### 1. Throughput Benchmark

```bash
cargo run --release --example benchmark_throughput
```

**Measures:**
- Chunks processed per second
- MB/s throughput
- CPU utilization
- Memory usage

**Expected Results** (M1 MacBook Pro):
- **Throughput**: 500+ MB/s
- **Chunks/sec**: 100,000+
- **CPU**: <50% single core
- **Memory**: <100MB

#### 2. Latency Benchmark

```bash
cargo run --release --example benchmark_latency
```

**Measures:**
- Chunk encode time
- Chunk decode time
- Hash computation time
- Network round-trip time

**Expected Results**:
- **Encode**: <5ms per chunk
- **Decode**: <5ms per chunk
- **Hash**: <1ms per chunk
- **RTT**: <10ms (localhost)

#### 3. Load Testing

```bash
# Install Apache Bench
brew install apache-bench

# Test upload endpoint (100 concurrent requests)
ab -n 1000 -c 100 -p /tmp/test_1mb.bin \
   -T "multipart/form-data" \
   http://localhost:3000/api/v1/upload
```

**Metrics:**
- Requests per second
- Mean response time
- 95th percentile latency
- Error rate

**Target Performance**:
- **RPS**: 500+
- **Mean Latency**: <100ms
- **P95 Latency**: <200ms
- **Error Rate**: <0.1%

---

## Continuous Integration

### GitHub Actions Workflow

**File**: `.github/workflows/test.yml`

```yaml
name: Test Suite

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run tests
        run: cargo test --all-features
      
      - name: Run clippy
        run: cargo clippy -- -D warnings
      
      - name: Check formatting
        run: cargo fmt -- --check
      
      - name: Build release
        run: cargo build --release

  frontend-test:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: 18
      
      - name: Install dependencies
        working-directory: ./frontend
        run: npm ci
      
      - name: Run tests
        working-directory: ./frontend
        run: npm test
      
      - name: Build
        working-directory: ./frontend
        run: npm run build
```

---

## Test Best Practices

### 1. Test Isolation

- Each test should be independent
- Use temp directories for file operations
- Clean up resources in Drop implementations

### 2. Deterministic Tests

- Avoid timing-dependent assertions
- Use fixed random seeds when needed
- Mock external dependencies

### 3. Test Naming

```rust
// Good
#[test]
fn test_chunk_manager_handles_large_files() { }

// Bad
#[test]
fn test1() { }
```

### 4. Error Messages

```rust
assert_eq!(
    result, expected,
    "Chunk count mismatch: got {}, expected {}",
    result, expected
);
```

---

## Troubleshooting Tests

### Common Issues

#### Tests Fail Intermittently

**Cause**: Race conditions or timing issues

**Solution**: Increase timeouts, use proper synchronization

```rust
// Bad
tokio::time::sleep(Duration::from_millis(100)).await;

// Good
tokio::time::timeout(Duration::from_secs(5), async {
    while !condition_met() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}).await.unwrap();
```

#### Port Already in Use

**Cause**: Previous test didn't clean up

**Solution**: Use port 0 for dynamic allocation

```rust
let listener = TcpListener::bind("127.0.0.1:0").await?;
let addr = listener.local_addr()?;
```

#### File Descriptor Limit

**Cause**: Too many open connections

**Solution**: Increase limit

```bash
ulimit -n 4096
cargo test
```

---

## Test Coverage Report

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/

# View report
open coverage/index.html
```

---

**Last Updated**: 2025-10-24

**Next Review**: After major feature additions or bug fixes
