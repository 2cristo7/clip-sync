#!/usr/bin/env bash
# build-release.sh -- Build a release archive of ClipSync.app
# Usage: bash mac/scripts/build-release.sh
# Output: mac/build/Release/ClipSync.app

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT="$PROJECT_DIR/ClipSync.xcodeproj"
SCHEME="ClipSync"
BUILD_DIR="$PROJECT_DIR/build"
ARCHIVE_PATH="$BUILD_DIR/ClipSync.xcarchive"
EXPORT_DIR="$BUILD_DIR/Release"

echo "==> Cleaning previous build artifacts..."
rm -rf "$BUILD_DIR"
mkdir -p "$EXPORT_DIR"

# Resolve signing identity
SIGNING_IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null \
    | grep -oE '"[^"]+"' | head -1 | tr -d '"' || true)

if [ -n "$SIGNING_IDENTITY" ]; then
    echo "==> Signing identity found: $SIGNING_IDENTITY"
    SIGN_FLAGS=(
        CODE_SIGN_IDENTITY="$SIGNING_IDENTITY"
        CODE_SIGNING_REQUIRED=YES
    )
else
    echo "==> No signing identity found. Building unsigned."
    SIGN_FLAGS=(
        CODE_SIGN_IDENTITY="-"
        CODE_SIGNING_REQUIRED=NO
        CODE_SIGNING_ALLOWED=NO
    )
fi

echo "==> Archiving $SCHEME..."
xcodebuild archive \
    -project "$PROJECT" \
    -scheme "$SCHEME" \
    -archivePath "$ARCHIVE_PATH" \
    -destination "generic/platform=macOS" \
    "${SIGN_FLAGS[@]}" \
    SKIP_INSTALL=NO \
    BUILD_LIBRARY_FOR_DISTRIBUTION=NO \
    -quiet \
    2>&1 || {
        echo "==> Archive failed. Falling back to plain build..."
        xcodebuild build \
            -project "$PROJECT" \
            -scheme "$SCHEME" \
            -configuration Release \
            -derivedDataPath "$BUILD_DIR/DerivedData" \
            "${SIGN_FLAGS[@]}" \
            -quiet

        APP_PATH=$(find "$BUILD_DIR/DerivedData" -name "ClipSync.app" -type d | head -1)
        if [ -n "$APP_PATH" ]; then
            cp -R "$APP_PATH" "$EXPORT_DIR/ClipSync.app"
            echo "==> Build succeeded (fallback): $EXPORT_DIR/ClipSync.app"
            exit 0
        else
            echo "ERROR: Could not find ClipSync.app in build output."
            exit 1
        fi
    }

# Export the archive
if [ -d "$ARCHIVE_PATH" ]; then
    # Create a minimal export options plist
    EXPORT_PLIST="$BUILD_DIR/export-options.plist"
    cat > "$EXPORT_PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>mac-application</string>
    <key>destination</key>
    <string>export</string>
</dict>
</plist>
PLIST

    echo "==> Exporting archive..."
    xcodebuild -exportArchive \
        -archivePath "$ARCHIVE_PATH" \
        -exportPath "$EXPORT_DIR" \
        -exportOptionsPlist "$EXPORT_PLIST" \
        -quiet 2>&1 || {
            # If export fails, extract .app directly from the archive
            echo "==> Export failed. Extracting .app from archive..."
            APP_IN_ARCHIVE=$(find "$ARCHIVE_PATH/Products" -name "ClipSync.app" -type d | head -1)
            if [ -n "$APP_IN_ARCHIVE" ]; then
                cp -R "$APP_IN_ARCHIVE" "$EXPORT_DIR/ClipSync.app"
            else
                echo "ERROR: Could not find ClipSync.app in archive."
                exit 1
            fi
        }
fi

if [ -d "$EXPORT_DIR/ClipSync.app" ]; then
    echo "==> Build succeeded: $EXPORT_DIR/ClipSync.app"
else
    echo "==> Build output is in: $EXPORT_DIR/"
    ls -la "$EXPORT_DIR/"
fi
