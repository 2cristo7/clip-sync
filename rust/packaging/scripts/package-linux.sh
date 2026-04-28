#!/usr/bin/env bash
# Creates .deb packages for clipsync-server and clipsync-client.
#
# Usage: ./package-linux.sh [--version 0.2.0]
# Outputs: dist/clipsync-server_<version>_amd64.deb
#          dist/clipsync-client_<version>_amd64.deb

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
LINUX_DIR="$SCRIPT_DIR/../linux"
DIST_DIR="$RUST_DIR/dist"
VERSION="0.1.0"
ARCH="amd64"

# Parse --version flag
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --arch)    ARCH="$2";    shift 2 ;;
        *)         shift ;;
    esac
done

echo "==> Packaging .deb packages (version $VERSION, arch $ARCH)"

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

    PKG_NAME="${BINARY}_${VERSION}_${ARCH}"
    PKG_DIR="$DIST_DIR/$PKG_NAME"

    echo "==> Creating $PKG_NAME.deb..."
    rm -rf "$PKG_DIR"
    mkdir -p "$PKG_DIR/DEBIAN"
    mkdir -p "$PKG_DIR/usr/bin"
    mkdir -p "$PKG_DIR/usr/share/applications"

    # Control file
    cat > "$PKG_DIR/DEBIAN/control" <<EOF
Package: $BINARY
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: libgtk-3-0, libayatana-appindicator3-1
Maintainer: ClipSync <osixtechteam@gmail.com>
Description: ClipSync ${BINARY#clipsync-}
 Cross-platform clipboard synchronization over LAN.
 This package contains the ClipSync ${BINARY#clipsync-}.
EOF

    # Copy binary and desktop file
    cp "$BINARY_PATH" "$PKG_DIR/usr/bin/$BINARY"
    chmod 755 "$PKG_DIR/usr/bin/$BINARY"

    if [[ -f "$LINUX_DIR/$BINARY.desktop" ]]; then
        cp "$LINUX_DIR/$BINARY.desktop" "$PKG_DIR/usr/share/applications/"
    fi

    # Build .deb
    dpkg-deb --build "$PKG_DIR" "$DIST_DIR/$PKG_NAME.deb"
    rm -rf "$PKG_DIR"

    echo "    Created: $DIST_DIR/$PKG_NAME.deb"
done

echo "==> Done. Packages in $DIST_DIR/"
ls -la "$DIST_DIR/"*.deb
