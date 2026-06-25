#!/bin/bash
# Rover — update, rebuild, and relaunch
# Usage: curl -fsSL https://raw.githubusercontent.com/amrithsrinivas/rover/main/scripts/update.sh | bash

set -euo pipefail

INSTALL_DIR="$HOME/rover"
PORT="${1:-9050}"

echo "========================================"
echo "  Rover — Update & Relaunch"
echo "========================================"
echo ""

cd "$INSTALL_DIR"

# 1. Pull latest
echo "[1/3] Pulling latest changes..."
git pull --ff-only origin main
echo "  ✓ Repo updated"

# 2. Rebuild
echo "[2/3] Building rover-server..."
RETRY=0
while [ $RETRY -lt 3 ]; do
    if CARGO_BUILD_JOBS=1 CARGO_NET_RETRY=3 cargo build --release -p rover-server 2>&1; then
        break
    fi
    RETRY=$((RETRY + 1))
    echo "  Build attempt $RETRY failed, retrying..."
    sleep 2
done
echo "  ✓ Build complete"

# 3. Launch
echo ""
echo "[3/3] Starting rover-server on port $PORT..."
echo "========================================"
echo ""

exec "$INSTALL_DIR/target/release/roverd" --port "$PORT"
