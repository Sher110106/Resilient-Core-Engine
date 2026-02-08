#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# ChunkStream Pro - EC2 Deployment Script
#
# Usage:
#   ./deploy.sh <EC2_IP> <SSH_KEY_PATH>
#
# Example:
#   ./deploy.sh 54.123.45.67 ~/.ssh/my-key.pem
#
# Prerequisites:
#   - Docker Desktop running locally (with QEMU/Rosetta for amd64 emulation)
#   - SSH key for EC2 instance
#   - EC2 security group allows inbound TCP 22 (SSH) and TCP 80 (HTTP)
# =============================================================================

EC2_IP="${1:?Usage: ./deploy.sh <EC2_IP> <SSH_KEY_PATH>}"
SSH_KEY="${2:?Usage: ./deploy.sh <EC2_IP> <SSH_KEY_PATH>}"
SSH_USER="ubuntu"
SSH_OPTS="-o StrictHostKeyChecking=no -i $SSH_KEY"

echo "========================================"
echo " ChunkStream Pro - Deploy to EC2"
echo "========================================"
echo ""
echo "  Target:  $SSH_USER@$EC2_IP"
echo "  SSH Key: $SSH_KEY"
echo ""

# ── Step 1: Build Docker images locally for linux/amd64 ─────────────
echo "[1/5] Building Docker images for linux/amd64..."
echo "      (This may take 15-20 min on first run due to cross-compilation)"
echo ""

docker compose build --no-cache 2>&1 | tail -5
echo ""
echo "      Build complete."

# ── Step 2: Save images to tar files ────────────────────────────────
echo "[2/5] Saving Docker images to archive..."

# Get the image names from docker compose
BACKEND_IMAGE=$(docker compose images server --format json 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['Repository']+':'+json.load(sys.stdin)[0]['Tag'])" 2>/dev/null || echo "track-server:latest")
FRONTEND_IMAGE=$(docker compose images frontend --format json 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['Repository']+':'+json.load(sys.stdin)[0]['Tag'])" 2>/dev/null || echo "track-frontend:latest")

# Save all project images
docker save track-server track-receiver track-frontend 2>/dev/null | gzip > /tmp/chunkstream-images.tar.gz || \
docker save $(docker compose images --format "{{.Repository}}:{{.Tag}}" 2>/dev/null | sort -u) | gzip > /tmp/chunkstream-images.tar.gz

IMAGE_SIZE=$(du -h /tmp/chunkstream-images.tar.gz | cut -f1)
echo "      Archive size: $IMAGE_SIZE"

# ── Step 3: Install Docker on EC2 if needed ─────────────────────────
echo "[3/5] Setting up EC2 instance..."

ssh $SSH_OPTS "$SSH_USER@$EC2_IP" bash <<'REMOTE_SETUP'
set -euo pipefail

# Install Docker if not present
if ! command -v docker &> /dev/null; then
    echo "      Installing Docker..."
    sudo apt-get update -qq
    sudo apt-get install -y -qq ca-certificates curl
    sudo install -m 0755 -d /etc/apt/keyrings
    sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
    sudo chmod a+r /etc/apt/keyrings/docker.asc
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
    sudo apt-get update -qq
    sudo apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-compose-plugin
    sudo usermod -aG docker $USER
    echo "      Docker installed."
else
    echo "      Docker already installed."
fi

# Create project directory
mkdir -p ~/chunkstream
REMOTE_SETUP

# ── Step 4: Transfer images and compose file to EC2 ─────────────────
echo "[4/5] Transferring files to EC2..."

scp $SSH_OPTS /tmp/chunkstream-images.tar.gz "$SSH_USER@$EC2_IP:~/chunkstream/"
scp $SSH_OPTS docker-compose.yml "$SSH_USER@$EC2_IP:~/chunkstream/"

echo "      Transfer complete."

# ── Step 5: Load images and start services ───────────────────────────
echo "[5/5] Starting services on EC2..."

ssh $SSH_OPTS "$SSH_USER@$EC2_IP" bash <<'REMOTE_START'
set -euo pipefail
cd ~/chunkstream

# Load Docker images
echo "      Loading Docker images..."
sudo docker load < chunkstream-images.tar.gz

# Stop existing containers if any
sudo docker compose down 2>/dev/null || true

# Start all services
sudo docker compose up -d

# Show status
echo ""
echo "      Services running:"
sudo docker compose ps
REMOTE_START

# ── Done ─────────────────────────────────────────────────────────────
echo ""
echo "========================================"
echo " Deployment complete!"
echo "========================================"
echo ""
echo " App URL:  http://$EC2_IP"
echo ""
echo " Useful commands (run on EC2 via SSH):"
echo "   ssh $SSH_OPTS $SSH_USER@$EC2_IP"
echo "   cd ~/chunkstream && sudo docker compose logs -f"
echo "   cd ~/chunkstream && sudo docker compose ps"
echo "   cd ~/chunkstream && sudo docker compose restart"
echo ""

# Clean up local temp file
rm -f /tmp/chunkstream-images.tar.gz
