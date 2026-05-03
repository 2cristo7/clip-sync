#!/usr/bin/env bash
set -euo pipefail

REPO="2cristo7/clip-sync"
API="https://api.github.com/repos/$REPO/releases/latest"

# ── resolve latest version ───────────────────────────────────────────────────
VERSION=$(curl -fsSL "$API" | grep '"tag_name"' | head -1 | cut -d'"' -f4)
if [[ -z "$VERSION" ]]; then
  echo "error: could not fetch latest release from GitHub" >&2
  exit 1
fi
BASE="https://github.com/$REPO/releases/download/$VERSION"

echo "ClipSync $VERSION"
echo

# ── macOS installer ───────────────────────────────────────────────────────────
install_mac() {
  local dmg="ClipSync-${VERSION}.dmg"
  local tmp="/tmp/$dmg"

  echo "Downloading $dmg …"
  curl -fsSL --progress-bar "$BASE/$dmg" -o "$tmp"

  echo "Installing ClipSync.app …"
  hdiutil attach "$tmp" -quiet -nobrowse

  rm -rf /Applications/ClipSync.app
  cp -R /Volumes/ClipSync/ClipSync.app /Applications/ClipSync.app

  hdiutil detach /Volumes/ClipSync -quiet
  rm -f "$tmp"

  echo
  echo "Done. ClipSync installed at /Applications/ClipSync.app"
  echo "Open it from Launchpad or Spotlight."
}

# ── Android installer ─────────────────────────────────────────────────────────
install_android() {
  local apk="ClipSync-${VERSION}.apk"
  local tmp="/tmp/$apk"

  if command -v adb &>/dev/null && adb devices 2>/dev/null | grep -q "device$"; then
    echo "Android device detected via ADB."
    echo "Downloading $apk …"
    curl -fsSL --progress-bar "$BASE/$apk" -o "$tmp"
    adb install -r "$tmp"
    rm -f "$tmp"
    echo "Done. ClipSync installed on connected device."
  else
    echo "No ADB device found. Manual install:"
    echo
    echo "  1. On your Android: Settings → Security → enable 'Install unknown apps'"
    echo "     (or per-browser: allow your browser to install APKs)"
    echo "  2. Download the APK on your phone:"
    echo "     $BASE/$apk"
    echo "  3. Open the downloaded file and tap Install."
  fi
}

# ── dispatch ──────────────────────────────────────────────────────────────────
case "$(uname -s)" in
  Darwin)
    install_mac
    ;;
  Linux)
    echo "Linux detected — ClipSync Mac app requires macOS."
    echo
    install_android
    ;;
  *)
    echo "Unsupported platform: $(uname -s)"
    echo
    install_android
    ;;
esac
