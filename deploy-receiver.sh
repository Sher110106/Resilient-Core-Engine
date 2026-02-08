#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Deploy chunkstream-receiver to EC2
#
# Usage:
#   ./deploy-receiver.sh <EC2_IP> <SSH_KEY_PATH>
#
# Example:
#   ./deploy-receiver.sh 54.123.45.67 ~/.ssh/chunkstream-key.pem
# =============================================================================

EC2_IP="${1:?Usage: ./deploy-receiver.sh <EC2_IP> <SSH_KEY_PATH>}"
SSH_KEY="${2:?Usage: ./deploy-receiver.sh <EC2_IP> <SSH_KEY_PATH>}"
SSH_USER="ubuntu"
SSH_OPTS="-o StrictHostKeyChecking=no -i $SSH_KEY"
BINARY="/tmp/chunkstream-receiver"

echo "========================================"
echo " ChunkStream Pro - Deploy Receiver"
echo "========================================"
echo ""
echo "  Target: $SSH_USER@$EC2_IP"
echo ""

# Check binary exists
if [ ! -f "$BINARY" ]; then
    echo "ERROR: Receiver binary not found at $BINARY"
    echo "Build it first with:"
    echo "  docker buildx build --platform linux/amd64 --target builder -t chunkstream-builder ."
    echo "  docker create --name extract-bin chunkstream-builder"
    echo "  docker cp extract-bin:/app/target/release/chunkstream-receiver /tmp/chunkstream-receiver"
    echo "  docker rm extract-bin"
    exit 1
fi

# Step 1: Transfer binary
echo "[1/3] Transferring receiver binary to EC2..."
scp $SSH_OPTS "$BINARY" "$SSH_USER@$EC2_IP:~/"
echo "      Done."

# Step 2: Set up on EC2
echo "[2/3] Setting up receiver on EC2..."
ssh $SSH_OPTS "$SSH_USER@$EC2_IP" bash <<'REMOTE_SETUP'
set -euo pipefail

# Make binary executable
chmod +x ~/chunkstream-receiver

# Create directory for received files
mkdir -p ~/received

# Stop any existing receiver
pkill -f chunkstream-receiver 2>/dev/null || true
sleep 1

echo "      Setup complete."
REMOTE_SETUP

# Step 3: Start receiver
echo "[3/3] Starting receiver on EC2..."
ssh $SSH_OPTS "$SSH_USER@$EC2_IP" bash <<'REMOTE_START'
set -euo pipefail

# Start receiver in background with nohup
nohup ~/chunkstream-receiver 0.0.0.0:5001 ~/received > ~/receiver.log 2>&1 &
RECEIVER_PID=$!

sleep 2

# Check it's running
if kill -0 $RECEIVER_PID 2>/dev/null; then
    echo "      Receiver running (PID: $RECEIVER_PID)"
else
    echo "ERROR: Receiver failed to start. Logs:"
    cat ~/receiver.log
    exit 1
fi
REMOTE_START

echo ""
echo "========================================"
echo " Receiver deployed!"
echo "========================================"
echo ""
echo " Receiver QUIC:     $EC2_IP:5001 (UDP)"
echo " Receiver REST API: http://$EC2_IP:8080"
echo ""
echo " To run locally:"
echo "   Terminal 1: cargo run --bin chunkstream-server"
echo "   Terminal 2: cd frontend && npm start"
echo "   Then open http://localhost:3001 and set receiver to $EC2_IP:5001"
echo ""
echo " Useful commands:"
echo "   ssh $SSH_OPTS $SSH_USER@$EC2_IP 'tail -f ~/receiver.log'   # watch logs"
echo "   ssh $SSH_OPTS $SSH_USER@$EC2_IP 'pkill chunkstream-receiver' # stop"
echo "   curl http://$EC2_IP:8080/api/v1/receiver/status              # check status"
echo ""
