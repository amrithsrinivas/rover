#!/bin/bash
# Rover — install runtime deps, update, rebuild, and relaunch
# Usage: curl -fsSL https://raw.githubusercontent.com/amrithsrinivas/rover/refs/heads/main/scripts/update.sh | bash

set -euo pipefail

INSTALL_DIR="$HOME/rover"
PORT="${1:-9050}"

echo "========================================"
echo "  Rover — Install, Update & Relaunch"
echo "========================================"
echo ""

cd "$INSTALL_DIR"

# 1. Install runtime dependencies
echo "[1/4] Installing runtime dependencies..."
pkg install -y python nodejs golang rust 2>/dev/null || true
echo "  ✓ Runtimes installed (or already present)"

# 2. Pull latest
echo "[2/4] Pulling latest changes..."
git pull --ff-only origin main
echo "  ✓ Repo updated"

# 3. Rebuild
echo "[3/4] Building rover-server..."
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

# 4. Launch
echo ""
echo "[4/4] Starting rover-server on port $PORT..."
echo "========================================"
echo ""

exec "$INSTALL_DIR/target/release/roverd" --port "$PORT"
