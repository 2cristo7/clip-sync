#!/usr/bin/env bash
# Builds ClipSync.dmg (macOS) and ClipSync.apk (Android) and stages them in releases/.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(cat "$REPO_ROOT/VERSION")"
OUT="$REPO_ROOT/releases"
mkdir -p "$OUT"

echo "Building ClipSync $VERSION"
echo

# ── macOS DMG ─────────────────────────────────────────────────────────────────
echo "==> macOS"
cd "$REPO_ROOT/mac"
rm -rf build
xcodebuild build \
  -project ClipSync.xcodeproj \
  -scheme ClipSync \
  -configuration Release \
  -derivedDataPath build/DerivedData \
  CODE_SIGN_IDENTITY="-" \
  CODE_SIGNING_REQUIRED=YES \
  CODE_SIGNING_ALLOWED=YES \
  -quiet

APP="build/DerivedData/Build/Products/Release/ClipSync.app"
DMG_NAME="ClipSync-${VERSION}.dmg"
STAGING="$REPO_ROOT/mac/build/dmg-staging"

rm -rf "$STAGING"
mkdir -p "$STAGING"
cp -R "$APP" "$STAGING/ClipSync.app"
ln -s /Applications "$STAGING/Applications"

hdiutil create \
  -volname "ClipSync" \
  -srcfolder "$STAGING" \
  -ov -format UDZO \
  "$OUT/$DMG_NAME" \
  -quiet

rm -rf "$STAGING"
echo "  → $OUT/$DMG_NAME"

# ── Android APK ───────────────────────────────────────────────────────────────
echo "==> Android"
cd "$REPO_ROOT/android"
./gradlew assembleDebug --quiet

APK_SRC="app/build/outputs/apk/debug/app-debug.apk"
APK_NAME="ClipSync-${VERSION}.apk"
cp "$APK_SRC" "$OUT/$APK_NAME"
echo "  → $OUT/$APK_NAME"

echo
echo "Release files ready in releases/:"
ls -lh "$OUT/"
echo
echo "Next: create a GitHub Release tagged v${VERSION} and upload both files."
echo "  gh release create v${VERSION} $OUT/$DMG_NAME $OUT/$APK_NAME --title \"v${VERSION}\" --notes \"\""
