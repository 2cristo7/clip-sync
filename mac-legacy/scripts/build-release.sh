#!/usr/bin/env bash
# build-release.sh -- Build ClipSync.app (unsigned) and package as DMG
# Usage: bash mac-legacy/scripts/build-release.sh [version]
# Output: releases/ClipSync-<version>.dmg

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$PROJECT_DIR/.." && pwd)"
PROJECT="$PROJECT_DIR/ClipSync.xcodeproj"
SCHEME="ClipSync"
BUILD_DIR="$PROJECT_DIR/build"
DERIVED_DATA="$BUILD_DIR/DerivedData"
EXPORT_DIR="$BUILD_DIR/Release"
VERSION="${1:-0.1.1}"
DMG_OUT="$REPO_ROOT/releases/ClipSync-${VERSION}.dmg"

echo "==> Cleaning previous build artifacts..."
rm -rf "$BUILD_DIR"
mkdir -p "$EXPORT_DIR"

# Always build unsigned — project is not set up for distribution signing.
# Passing CODE_SIGNING_ALLOWED=NO to all targets (including SPM deps) avoids
# the "requires a development team" errors when a cert exists in the keychain.
SIGN_FLAGS=(
    CODE_SIGN_IDENTITY="-"
    CODE_SIGNING_REQUIRED=NO
    CODE_SIGNING_ALLOWED=NO
    AD_HOC_CODE_SIGNING_ALLOWED=YES
)

echo "==> Building $SCHEME (Release, unsigned)..."
xcodebuild build \
    -project "$PROJECT" \
    -scheme "$SCHEME" \
    -configuration Release \
    -derivedDataPath "$DERIVED_DATA" \
    "${SIGN_FLAGS[@]}" \
    -quiet 2>&1

APP_PATH=$(find "$DERIVED_DATA" -name "ClipSync.app" -type d | head -1)
if [ -z "$APP_PATH" ]; then
    echo "ERROR: ClipSync.app not found in build output."
    exit 1
fi

cp -R "$APP_PATH" "$EXPORT_DIR/ClipSync.app"
echo "==> App: $EXPORT_DIR/ClipSync.app"

echo "==> Creating DMG: $DMG_OUT..."
hdiutil create \
    -volname "ClipSync" \
    -srcfolder "$EXPORT_DIR/ClipSync.app" \
    -ov -format UDZO \
    "$DMG_OUT" 2>&1

echo "==> Done: $DMG_OUT"
