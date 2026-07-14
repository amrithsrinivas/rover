#!/bin/bash
# Rover — one-command install and launch for Android/Termux
# Usage:
#   curl ... | bash                          # default port 9050, local only
#   curl ... | bash -s -- 9050               # custom port
#   curl ... | bash -s -- 9050 --bore        # enable bore tunneling
#   curl ... | bash -s -- 9050 --bore --bore-server myhost.com --bore-secret mykey

set -euo pipefail

REPO="https://github.com/amrithsrinivas/rover"
INSTALL_DIR="$HOME/rover"
PORT="9050"
BORE_FLAGS=()

# ── Parse arguments ──────────────────────────────────────────────────────────
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --bore)
                BORE_FLAGS+=("--bore")
                shift
                ;;
            --bore-server)
                BORE_FLAGS+=("--bore-server" "$2")
                shift 2
                ;;
            --bore-port)
                BORE_FLAGS+=("--bore-port" "$2")
                shift 2
                ;;
            --bore-secret)
                BORE_FLAGS+=("--bore-secret" "$2")
                shift 2
                ;;
            --port)
                PORT="$2"
                shift 2
                ;;
            *)
                # First positional arg is the port
                if [[ "$1" =~ ^[0-9]+$ ]] && [[ -z "${_port_set:-}" ]]; then
                    PORT="$1"
                    _port_set=1
                else
                    echo "Unknown argument: $1"
                    echo "Usage: install.sh [port] [--bore] [--bore-server HOST] [--bore-port PORT] [--bore-secret SECRET]"
                    exit 1
                fi
                shift
                ;;
        esac
    done
}

parse_args "$@"

echo "========================================"
echo "  Rover — Install & Launch"
echo "========================================"
echo ""

# 1. Install dependencies via pkg
echo "[1/4] Installing dependencies..."
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
    true
fi
echo "  ✓ Build complete"

# 4. Launch the server
echo ""
echo "[4/4] Starting rover-server on port $PORT..."
if [[ ${#BORE_FLAGS[@]} -gt 0 ]]; then
    echo "  Bore tunnel: enabled"
fi
echo "========================================"
echo ""

exec "$INSTALL_DIR/target/release/roverd" --port "$PORT" "${BORE_FLAGS[@]}"
