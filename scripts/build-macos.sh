#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────
# build-macos.sh — Compile the Rover client for macOS
# ──────────────────────────────────────────────

APP_NAME="Rover"
BINARY_NAME="rover"
TARGET="aarch64-apple-darwin"
PROFILE="${1:-release}"
OUT_DIR="dist/macos"
ICON_SRC="crates/rover-client/icon/icon.png"

echo "=== Building $APP_NAME for macOS ($TARGET, $PROFILE) ==="

# Build the binary
if [ "$PROFILE" = "release" ]; then
    cargo build --release -p rover-client --target "$TARGET"
    BIN_PATH="target/$TARGET/release/$BINARY_NAME"
else
    cargo build -p rover-client --target "$TARGET"
    BIN_PATH="target/$TARGET/debug/$BINARY_NAME"
fi

# Create output directory
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/$APP_NAME.app/Contents/MacOS"
mkdir -p "$OUT_DIR/$APP_NAME.app/Contents/Resources"

# Copy binary
cp "$BIN_PATH" "$OUT_DIR/$APP_NAME.app/Contents/MacOS/$APP_NAME"

# Generate .icns from PNG
if [ -f "$ICON_SRC" ]; then
    echo "=== Generating app icon ==="
    ICONSET="icon.iconset"
    mkdir -p "$ICONSET"
    sips -z 16 16   "$ICON_SRC" --out "$ICONSET/icon_16x16.png"
    sips -z 32 32   "$ICON_SRC" --out "$ICONSET/icon_16x16@2x.png"
    sips -z 32 32   "$ICON_SRC" --out "$ICONSET/icon_32x32.png"
    sips -z 64 64   "$ICON_SRC" --out "$ICONSET/icon_32x32@2x.png"
    sips -z 128 128 "$ICON_SRC" --out "$ICONSET/icon_128x128.png"
    sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_128x128@2x.png"
    sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_256x256.png"
    sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_256x256@2x.png"
    sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_512x512.png"
    sips -z 1024 1024 "$ICON_SRC" --out "$ICONSET/icon_512x512@2x.png"
    iconutil -c icns "$ICONSET" -o "$OUT_DIR/$APP_NAME.app/Contents/Resources/AppIcon.icns"
    rm -rf "$ICONSET"
else
    echo "Warning: $ICON_SRC not found, skipping icon"
fi

# Write Info.plist
cat > "$OUT_DIR/$APP_NAME.app/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>dev.rover.client</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

echo ""
echo "=== Done ==="
echo "App bundle: $OUT_DIR/$APP_NAME.app"
echo "Binary:     $OUT_DIR/$APP_NAME.app/Contents/MacOS/$APP_NAME"
echo ""
echo "Run with:   open $OUT_DIR/$APP_NAME.app"
echo "   or:      ./$OUT_DIR/$APP_NAME.app/Contents/MacOS/$APP_NAME"
