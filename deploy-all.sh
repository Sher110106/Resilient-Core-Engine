#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Deploy chunkstream-server + chunkstream-receiver to EC2
#
# Usage:
#   ./deploy-all.sh <EC2_IP> <SSH_KEY_PATH>
#
# Example:
#   ./deploy-all.sh 54.163.27.203 ~/.ssh/chunkstream-key.pem
#
# Prerequisite: binaries already built at /tmp/chunkstream-server and
#               /tmp/chunkstream-receiver (use Docker cross-compile)
# =============================================================================

EC2_IP="${1:?Usage: ./deploy-all.sh <EC2_IP> <SSH_KEY_PATH>}"
SSH_KEY="${2:?Usage: ./deploy-all.sh <EC2_IP> <SSH_KEY_PATH>}"
SSH_USER="ubuntu"
SSH_OPTS="-o StrictHostKeyChecking=no -i $SSH_KEY"
SERVER_BIN="/tmp/chunkstream-server"
RECEIVER_BIN="/tmp/chunkstream-receiver"

echo "========================================"
echo " ChunkStream Pro - Deploy to EC2"
echo "========================================"
echo ""
echo "  Target: $SSH_USER@$EC2_IP"
echo ""

# Check binaries exist
for bin in "$SERVER_BIN" "$RECEIVER_BIN"; do
    if [ ! -f "$bin" ]; then
        echo "ERROR: Binary not found at $bin"
        echo "Build first with: docker buildx build --platform linux/amd64 --no-cache --target builder -t chunkstream-builder ."
        exit 1
    fi
done

# Step 1: Transfer binaries
echo "[1/3] Transferring binaries to EC2..."
scp $SSH_OPTS "$SERVER_BIN" "$RECEIVER_BIN" "$SSH_USER@$EC2_IP:~/"
echo "      Done. (server: $(du -h $SERVER_BIN | cut -f1), receiver: $(du -h $RECEIVER_BIN | cut -f1))"

# Step 2: Set up on EC2
echo "[2/3] Setting up EC2..."
ssh $SSH_OPTS "$SSH_USER@$EC2_IP" bash <<'REMOTE_SETUP'
set -euo pipefail

# Make binaries executable
chmod +x ~/chunkstream-server ~/chunkstream-receiver

# Create directories
mkdir -p ~/received ~/uploads

# Stop any existing processes
pkill -f chunkstream-server 2>/dev/null || true
pkill -f chunkstream-receiver 2>/dev/null || true
sleep 2

echo "      Setup complete."
REMOTE_SETUP

# Step 3: Start both services
echo "[3/3] Starting services on EC2..."
ssh $SSH_OPTS "$SSH_USER@$EC2_IP" bash <<'REMOTE_START'
set -euo pipefail

# Start receiver in background
nohup ~/chunkstream-receiver 0.0.0.0:5001 ~/received > ~/receiver.log 2>&1 &
sleep 2

# Verify receiver is running
if pgrep -f chunkstream-receiver > /dev/null; then
    echo "      Receiver running (PID: $(pgrep -f chunkstream-receiver))"
else
    echo "ERROR: Receiver failed to start. Logs:"
    cat ~/receiver.log
    exit 1
fi

# Start server in background
nohup ~/chunkstream-server > ~/server.log 2>&1 &
sleep 2

# Verify server is running
if pgrep -f chunkstream-server > /dev/null; then
    echo "      Server running (PID: $(pgrep -f chunkstream-server))"
else
    echo "ERROR: Server failed to start. Logs:"
    cat ~/server.log
    exit 1
fi
REMOTE_START

echo ""
echo "========================================"
echo " Deployment complete!"
echo "========================================"
echo ""
echo " Server API:        http://$EC2_IP:3000"
echo " Server Health:     http://$EC2_IP:3000/health"
echo " WebSocket:         ws://$EC2_IP:3000/ws"
echo " Receiver QUIC:     $EC2_IP:5001 (UDP)"
echo " Receiver REST API: http://$EC2_IP:8080"
echo ""
echo " To run the frontend locally:"
echo "   REACT_APP_API_URL=http://$EC2_IP:3000 npm start"
echo "   (from the frontend/ directory)"
echo ""
echo " Useful commands:"
echo "   ssh $SSH_OPTS $SSH_USER@$EC2_IP 'tail -f ~/server.log'     # server logs"
echo "   ssh $SSH_OPTS $SSH_USER@$EC2_IP 'tail -f ~/receiver.log'   # receiver logs"
echo "   ssh $SSH_OPTS $SSH_USER@$EC2_IP 'pkill -f chunkstream'     # stop all"
echo "   curl http://$EC2_IP:3000/health                            # health check"
echo "   curl http://$EC2_IP:8080/api/v1/receiver/status            # receiver status"
echo ""
