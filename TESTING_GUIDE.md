# ChunkStream Pro - End-to-End Testing Guide

## 🎉 Implementation Complete!

All pieces are now in place for **real peer-to-peer file transfer** with:
- ✅ Chunking & Erasure Coding (10+3)
- ✅ QUIC Network Transport
- ✅ Sender → Receiver communication
- ✅ File reconstruction on receiver side
- ✅ Integrity verification

---

## 🚀 Quick Test (3 Terminals)

### Terminal 1: Start Receiver
```bash
cd /Users/sher/project/idk
./target/release/chunkstream-receiver
```

**Output:**
```
╔══════════════════════════════════════════════════════════════════╗
║        ChunkStream Pro - File Receiver Agent                    ║
╚══════════════════════════════════════════════════════════════════╝

📍 Bind Address:    0.0.0.0:5001
💾 Save Directory:  ./received
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

✅ Receiver ready! Waiting for incoming transfers...
```

### Terminal 2: Start Sender Server
```bash
cd /Users/sher/project/idk
./target/release/chunkstream-server
```

**Output:**
```
╔══════════════════════════════════════════════════════════════════╗
║          ChunkStream Pro - File Transfer Server                 ║
╚══════════════════════════════════════════════════════════════════╝

✅ ChunkStream Pro Server is running!

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📍 Server Address:  http://localhost:3000
🏥 Health Check:    http://localhost:3000/health
📡 REST API:        http://localhost:3000/api/v1/transfers
🔌 WebSocket:       ws://localhost:3000/ws
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### Terminal 3: Send a File via API
```bash
# Create a test file
dd if=/dev/urandom of=/tmp/test.bin bs=1M count=5

# Send it to receiver
curl -X POST http://localhost:3000/api/v1/transfers \
  -H "Content-Type: application/json" \
  -d '{
    "file_path": "/tmp/test.bin",
    "priority": "High",
    "receiver_addr": "127.0.0.1:5001"
  }'
```

**Expected Response:**
```json
{
  "session_id": "a1b2c3d4-5678-90ab-cdef-1234567890ab",
  "message": "Transfer started with session ID: a1b2c3d4..."
}
```

---

## 📊 What Happens During Transfer

### On Sender Side:
1. File split into chunks (e.g., 256KB each)
2. Erasure coding applied (10 data + 3 parity = 13 chunks)
3. Chunks queued by priority
4. QUIC connection established to receiver
5. Chunks sent over network with retry logic
6. Progress tracked in database

### On Receiver Side:
1. QUIC server accepts connection
2. Receives chunks one by one
3. Verifies chunk integrity (BLAKE3 checksum)
4. Stores chunks in memory
5. When enough chunks received (≥10), triggers reconstruction
6. Reconstructs original file using erasure decoding
7. Saves to `./received/received_<file_id>`
8. Verifies final file integrity

---

## 🔍 Monitor Progress

### Check Transfer Status:
```bash
# Get progress
curl http://localhost:3000/api/v1/transfers/<SESSION_ID>/progress

# Example response:
{
  "session_id": "a1b2c3d4...",
  "status": "Active",
  "progress_percent": 75.0,
  "completed_chunks": 10,
  "total_chunks": 13,
  "bytes_transferred": 2621440,
  "total_bytes": 5242880,
  "current_speed_bps": 524288
}
```

### Watch Receiver Output:
```
📡 New connection from: 127.0.0.1:54321
   📦 Receiving chunks from 127.0.0.1:54321...
   📋 Transfer ID: /tmp/test.bin
   ✓ Chunk 1/13 received (256.00 KB)
   ✓ Chunk 2/13 received (256.00 KB)
   ✓ Chunk 3/13 received (256.00 KB)
   ...
   ✓ Chunk 10/13 received (256.00 KB)

   🎯 Received 10 chunks - attempting reconstruction...
   ✅ File reconstructed successfully!
   💾 Saved to: ./received/received_/tmp/test.bin
   📊 Total chunks received: 10
   🔒 File integrity verified! ✓
```

---

## 🧪 Test Scenarios

### 1. Normal Transfer (All Chunks)
```bash
curl -X POST http://localhost:3000/api/v1/transfers \
  -H "Content-Type: application/json" \
  -d '{
    "file_path": "/tmp/test.bin",
    "priority": "Normal",
    "receiver_addr": "127.0.0.1:5001"
  }'
```
**Expected:** All 13 chunks sent, file reconstructed with 10 chunks

### 2. Lossy Network (Erasure Coding Test)
*Simulates 3 chunk loss - file should still reconstruct!*
- Manually stop sending after 10 chunks
- Receiver should still reconstruct successfully
- This demonstrates erasure coding power

### 3. Priority Test (Multiple Files)
```bash
# Send critical priority file
curl -X POST http://localhost:3000/api/v1/transfers \
  -H "Content-Type: application/json" \
  -d '{"file_path": "/tmp/critical.bin", "priority": "Critical", "receiver_addr": "127.0.0.1:5001"}'

# Send normal priority file
curl -X POST http://localhost:3000/api/v1/transfers \
  -H "Content-Type: application/json" \
  -d '{"file_path": "/tmp/normal.bin", "priority": "Normal", "receiver_addr": "127.0.0.1:5001"}'
```
**Expected:** Critical file chunks prioritized in queue

### 4. Pause/Resume
```bash
SESSION_ID="<your-session-id>"

# Pause
curl -X POST http://localhost:3000/api/v1/transfers/$SESSION_ID/pause

# Resume
curl -X POST http://localhost:3000/api/v1/transfers/$SESSION_ID/resume
```

### 5. Web UI Upload (Simulation Mode)
```bash
cd frontend && npm start
# Open http://localhost:3001
# Upload file (no receiver_addr = simulation mode)
```

---

## 🌐 Test with Two Machines

### Machine 1 (Receiver - e.g., 192.168.1.100):
```bash
./target/release/chunkstream-receiver 0.0.0.0:5001 ./received
```

### Machine 2 (Sender):
```bash
# Start server
./target/release/chunkstream-server

# Send file to Machine 1
curl -X POST http://localhost:3000/api/v1/transfers \
  -H "Content-Type: application/json" \
  -d '{
    "file_path": "/path/to/file.bin",
    "priority": "High",
    "receiver_addr": "192.168.1.100:5001"
  }'
```

---

## 🔧 Advanced Configuration

### Receiver Custom Port & Directory:
```bash
./target/release/chunkstream-receiver 0.0.0.0:9000 /custom/path
```

### Sender with Different Port:
Edit `src/bin/server.rs` line 49:
```rust
let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
```

### Change Chunk Size & Erasure Coding:
Edit `src/bin/server.rs` line 16:
```rust
let chunk_manager = ChunkManager::new(
    512 * 1024,  // 512KB chunks
    15,          // 15 data shards
    5            // 5 parity shards
).expect("Failed to create chunk manager");
```

---

## 🐛 Troubleshooting

### "Connection refused"
- Ensure receiver is running first
- Check firewall allows port 5001
- Verify receiver address is correct

### "Failed to send chunk"
- Network connectivity issue
- Check receiver logs for errors
- Try with retry: sender automatically retries up to 3 times

### "File checksum mismatch"
- Corruption detected
- With erasure coding, file still reconstructs if ≤3 chunks lost
- Check sender logs for chunk failures

### "No chunks received"
- Verify receiver_addr format: "IP:PORT"
- Check both processes are running
- Test with `nc -zv 127.0.0.1 5001` to verify port is open

---

## 📈 Performance Expectations

### Small Files (<10MB):
- Completes almost instantly
- May not see in active transfers (completes before poll)

### Large Files (100MB+):
- Watch real-time progress
- ~10-50 chunks/sec depending on network
- Bandwidth scales with network capacity

### Network Overhead:
- Erasure coding: +30% data (13 chunks instead of 10)
- QUIC protocol: ~5% overhead
- Metadata per chunk: <1KB

---

## ✅ Success Indicators

On **Receiver**:
```
✅ File reconstructed successfully!
🔒 File integrity verified! ✓
```

On **Sender**:
```
Transfer completed: 100%
```

**Verify File:**
```bash
# Compare checksums
md5 /tmp/test.bin
md5 ./received/received_<file_id>
# Should match!
```

---

## 🎯 Next Steps

1. ✅ **Basic Test**: Single file on localhost (done above)
2. 📊 **Monitor Metrics**: Check WebSocket for live updates
3. 🌐 **Two Machine Test**: Transfer between different computers
4. 🔥 **Stress Test**: Multiple concurrent transfers
5. 🎨 **Frontend**: Add receiver address input field
6. 📦 **Demo**: Prepare for presentation

---

## 🚀 Production Readiness

### What Works:
- ✅ Real QUIC network transfer
- ✅ Erasure coding for reliability
- ✅ Chunk-level integrity verification
- ✅ Pause/resume/cancel
- ✅ Priority queuing
- ✅ Session persistence

### What's Missing (Future):
- ❌ Receiver address persistence (doesn't survive resume)
- ❌ Bi-directional ACKs (receiver → sender confirmation)
- ❌ Multiple receiver support
- ❌ TLS certificate validation (using self-signed)
- ❌ Rate limiting / bandwidth control UI
- ❌ File deduplication

---

## 🎉 Congratulations!

You now have a **fully functional peer-to-peer file transfer system** with:
- **Smart chunking**
- **Erasure coding** (survive 23% packet loss!)
- **QUIC transport** (fast & reliable)
- **Priority scheduling**
- **Resume capability**
- **Web dashboard**

**Test it, break it, improve it!** 🚀
