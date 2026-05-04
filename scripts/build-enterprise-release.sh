#!/usr/bin/env bash
# build-enterprise-release.sh — Build all enterprise artifacts for a given version.
# Usage: ./scripts/build-enterprise-release.sh [version]
# Artifacts land in releases/enterprise/<version>/

set -euo pipefail

VERSION="${1:-$(grep '^version' rust/apps/enterprise-server/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')}"
RELEASE_DIR="releases/enterprise/${VERSION}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$ROOT_DIR"

echo "=== ClipSync Enterprise Release Builder v${VERSION} ==="
echo "Output directory: ${RELEASE_DIR}"

mkdir -p "${RELEASE_DIR}/server" "${RELEASE_DIR}/desktop" "${RELEASE_DIR}/client"

# ---------------------------------------------------------------------------
# Detect host platform
# ---------------------------------------------------------------------------
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
  Linux)  HOST_OS="linux" ;;
  Darwin) HOST_OS="mac" ;;
  MINGW*|MSYS*|CYGWIN*) HOST_OS="windows" ;;
  *) echo "Unsupported OS: ${OS}"; exit 1 ;;
esac

case "${ARCH}" in
  x86_64|amd64) HOST_ARCH="x64" ;;
  arm64|aarch64) HOST_ARCH="arm64" ;;
  *) echo "Unsupported arch: ${ARCH}"; exit 1 ;;
esac

echo "Host: ${HOST_OS}/${HOST_ARCH}"

# ---------------------------------------------------------------------------
# 1. Enterprise Server
# ---------------------------------------------------------------------------
echo ""
echo "--- Building enterprise-server ---"

cargo build --release -p enterprise-server

SERVER_BIN="target/release/enterprise-server"
if [ "${HOST_OS}" = "windows" ]; then
  SERVER_BIN="${SERVER_BIN}.exe"
fi

if [ "${HOST_OS}" = "linux" ]; then
  # tar.gz
  tar czf "${RELEASE_DIR}/server/clipsync-enterprise-server-${VERSION}-linux-${HOST_ARCH}.tar.gz" \
    -C target/release enterprise-server

  # deb (requires cargo-deb)
  if command -v cargo-deb &>/dev/null; then
    echo "Building .deb package..."
    cargo deb -p enterprise-server --no-build --output "${RELEASE_DIR}/server/"
  else
    echo "SKIP: cargo-deb not installed — skipping .deb"
  fi

  # rpm (requires cargo-generate-rpm)
  if command -v cargo-generate-rpm &>/dev/null; then
    echo "Building .rpm package..."
    cargo generate-rpm -p rust/apps/enterprise-server -o "${RELEASE_DIR}/server/"
  else
    echo "SKIP: cargo-generate-rpm not installed — skipping .rpm"
  fi

elif [ "${HOST_OS}" = "mac" ]; then
  tar czf "${RELEASE_DIR}/server/clipsync-enterprise-server-${VERSION}-mac-${HOST_ARCH}.tar.gz" \
    -C target/release enterprise-server

elif [ "${HOST_OS}" = "windows" ]; then
  cp "${SERVER_BIN}" "${RELEASE_DIR}/server/"
  (cd "${RELEASE_DIR}/server" && zip "clipsync-enterprise-server-${VERSION}-windows-${HOST_ARCH}.zip" enterprise-server.exe)
  rm -f "${RELEASE_DIR}/server/enterprise-server.exe"
fi

echo "Server artifacts: $(ls "${RELEASE_DIR}/server/")"

# ---------------------------------------------------------------------------
# 2. Enterprise Desktop (Tauri)
# ---------------------------------------------------------------------------
echo ""
echo "--- Building enterprise-desktop ---"

if command -v cargo-tauri &>/dev/null || command -v tauri &>/dev/null; then
  TAURI_CMD="cargo tauri"
  if command -v tauri &>/dev/null; then
    TAURI_CMD="tauri"
  fi

  (cd rust/apps/enterprise-desktop/src-tauri && ${TAURI_CMD} build) || true

  # Collect bundles produced by Tauri
  BUNDLE_DIR="rust/apps/enterprise-desktop/src-tauri/target/release/bundle"
  if [ -d "${BUNDLE_DIR}" ]; then
    find "${BUNDLE_DIR}" -type f \( -name "*.dmg" -o -name "*.msi" -o -name "*.AppImage" -o -name "*.deb" \) \
      -exec cp {} "${RELEASE_DIR}/desktop/" \;
  fi
  echo "Desktop artifacts: $(ls "${RELEASE_DIR}/desktop/" 2>/dev/null || echo 'none')"
else
  echo "SKIP: tauri CLI not installed — skipping desktop build"
fi

# ---------------------------------------------------------------------------
# 3. Enterprise Client (Tauri)
# ---------------------------------------------------------------------------
echo ""
echo "--- Building enterprise-client ---"

if command -v cargo-tauri &>/dev/null || command -v tauri &>/dev/null; then
  TAURI_CMD="cargo tauri"
  if command -v tauri &>/dev/null; then
    TAURI_CMD="tauri"
  fi

  (cd rust/apps/enterprise-client/src-tauri && ${TAURI_CMD} build) || true

  BUNDLE_DIR="rust/apps/enterprise-client/src-tauri/target/release/bundle"
  if [ -d "${BUNDLE_DIR}" ]; then
    find "${BUNDLE_DIR}" -type f \( -name "*.dmg" -o -name "*.msi" -o -name "*.AppImage" -o -name "*.deb" \) \
      -exec cp {} "${RELEASE_DIR}/client/" \;
  fi
  echo "Client artifacts: $(ls "${RELEASE_DIR}/client/" 2>/dev/null || echo 'none')"
else
  echo "SKIP: tauri CLI not installed — skipping client build"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "=== Release ${VERSION} complete ==="
echo "Artifacts in: ${RELEASE_DIR}/"
find "${RELEASE_DIR}" -type f | sort
