#!/usr/bin/env bash
# Creates macOS .app bundles for clipsync-server and clipsync-client.
#
# Usage: ./package-macos.sh [--version 0.2.0]
# Outputs: dist/ClipSync-Server.app  dist/ClipSync-Client.app

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
PLIST_TEMPLATE="$SCRIPT_DIR/../macos/Info.plist"
DIST_DIR="$RUST_DIR/dist"
VERSION="${1:---version}"

# Parse --version flag
if [[ "$VERSION" == "--version" ]]; then
    shift || true
    VERSION="${1:-0.1.0}"
fi

echo "==> Packaging macOS .app bundles (version $VERSION)"

# Build release binaries
echo "==> Building release binaries..."
cd "$RUST_DIR"
cargo build --release -p clipsync-server -p clipsync-client

mkdir -p "$DIST_DIR"

for BINARY in clipsync-server clipsync-client; do
    BINARY_PATH="$RUST_DIR/target/release/$BINARY"
    if [[ ! -f "$BINARY_PATH" ]]; then
        echo "ERROR: Binary not found at $BINARY_PATH"
        exit 1
    fi

    # Convert clipsync-server -> ClipSync-Server
    APP_NAME="ClipSync-$(echo "${BINARY#clipsync-}" | sed 's/^./\U&/')"
    APP_DIR="$DIST_DIR/$APP_NAME.app"

    echo "==> Creating $APP_NAME.app..."
    rm -rf "$APP_DIR"
    mkdir -p "$APP_DIR/Contents/MacOS"
    mkdir -p "$APP_DIR/Contents/Resources"

    # Copy binary
    cp "$BINARY_PATH" "$APP_DIR/Contents/MacOS/$BINARY"

    # Generate Info.plist from template
    sed -e "s/\${BINARY_NAME}/$BINARY/g" \
        -e "s/\${VERSION}/$VERSION/g" \
        "$PLIST_TEMPLATE" > "$APP_DIR/Contents/Info.plist"

    echo "    Created: $APP_DIR"
done

echo "==> Done. Bundles in $DIST_DIR/"
ls -la "$DIST_DIR/"
