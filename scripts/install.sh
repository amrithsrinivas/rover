#!/bin/bash
# Rover — one-command install and launch for Android/Termux
# Usage: curl -fsSL https://raw.githubusercontent.com/amrithsrinivas/rover/refs/heads/main/scripts/install.sh | bash

set -euo pipefail

REPO="https://github.com/amrithsrinivas/rover"
INSTALL_DIR="$HOME/rover"
PORT="${1:-9050}"

echo "========================================"
echo "  Rover — Install & Launch"
echo "========================================"
echo ""
# 1. Install dependencies via pkg
echo "[1/4] Installing dependencies..."
pkg update && pkg upgrade -y
pkg install -y \
    git \
    rust \
    binutils \
    python \
    protobuf \
    openssl \
    make \
    cmake \
    pkg-config \
    golang \
    nodejs

echo "  ✓ Dependencies installed"
# 2. Clone or update the repo
echo "[2/4] Fetching Rover source..."
if [ -d "$INSTALL_DIR" ]; then
    cd "$INSTALL_DIR"
    git pull --ff-only origin main
    echo "  ✓ Updated existing repo"
else
    git clone "$REPO" "$INSTALL_DIR"
    echo "  ✓ Cloned repo"
fi

# 3. Build the server
echo "[3/4] Building rover-server (this may take a few minutes)..."
cd "$INSTALL_DIR"
if CARGO_BUILD_JOBS=1 cargo build --release -p rover-server 2>&1; then
    break
fi
echo "  ✓ Build complete"

# 4. Launch the server
echo ""
echo "[4/4] Starting rover-server on port $PORT..."
echo "========================================"
echo ""

exec "$INSTALL_DIR/target/release/roverd" --port "$PORT"
