#!/bin/bash
# Rover — one-command install for Android/Termux
# Usage: curl -fsSL https://raw.githubusercontent.com/amrithsrinivas/rover/main/install.sh | bash

set -euo pipefail

REPO="https://github.com/amrithsrinivas/rover"
INSTALL_DIR="$HOME/rover"

echo "========================================"
echo "  Rover — One-Command Install"
echo "========================================"
echo ""

# 1. Install dependencies via pkg
echo "[1/4] Installing dependencies..."
pkg update -y > /dev/null 2>&1
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
    > /dev/null 2>&1
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
cargo build --release -p rover-server 2>&1 | tail -1
echo "  ✓ Build complete"

# 4. Print instructions
BIN="$INSTALL_DIR/target/release/roverd"
echo ""
echo "========================================"
echo "  Install Complete!"
echo "========================================"
echo ""
echo "  Binary: $BIN"
echo ""
echo "  To start the server:"
echo "    $BIN --port 9050"
echo ""
echo "  The server will print a pairing token."
echo "  Copy that token into the Rover desktop client to connect."
echo ""
echo "========================================"
