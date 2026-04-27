#!/usr/bin/env bash
# setup-signing.sh — Configure code-signing for ClipSync (macOS)
#
# Three possible outcomes:
#   A) Apple Developer certificate already in Keychain → nothing to do
#   B) Apple Developer account available (xcrun notarytool) → guides login
#   C) No Developer account → creates a local self-signed certificate
#
# The self-signed cert (option C) lets codesign work and the app run on
# YOUR Mac. Gatekeeper will still show a warning the first time; dismiss
# it via System Settings → Privacy & Security → "Open Anyway".
#
# Usage:
#   bash mac/scripts/setup-signing.sh          # interactive
#   bash mac/scripts/setup-signing.sh --ci     # non-interactive (option C only)

set -euo pipefail

# ── Colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}==>${RESET} $*"; }
ok()      { echo -e "${GREEN}✔${RESET}  $*"; }
warn()    { echo -e "${YELLOW}⚠${RESET}  $*"; }
die()     { echo -e "${RED}✘  $*${RESET}" >&2; exit 1; }
header()  { echo -e "\n${BOLD}$*${RESET}"; }

CI=false
[[ "${1:-}" == "--ci" ]] && CI=true

KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"
CERT_LABEL="ClipSync Local Developer"
CERT_DIR="$HOME/.clipsync-signing"
TMPDIR_SIGN="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_SIGN"' EXIT

# ── Step 1: Check for existing Apple Developer identities ────────────────────
header "Step 1 — Searching for existing code-signing identities"

APPLE_IDS=$(security find-identity -v -p codesigning 2>/dev/null \
    | grep -E "(Apple Development|Apple Distribution|Developer ID Application|Mac Developer|3rd Party)" \
    | grep -oE '"[^"]+"' | tr -d '"' || true)

if [[ -n "$APPLE_IDS" ]]; then
    ok "Apple Developer certificate(s) found:"
    echo "$APPLE_IDS" | sed 's/^/     /'
    echo ""
    ok "Nothing to do — build-release.sh will pick these up automatically."
    echo ""
    info "To build and sign:"
    echo "     bash mac/scripts/build-release.sh"
    exit 0
fi

warn "No Apple Developer certificate found in Keychain."

# ── Step 2: Offer guided Apple ID login (interactive only) ───────────────────
if [[ "$CI" == false ]]; then
    header "Step 2 — Do you have an Apple Developer account? (optional)"
    echo ""
    echo "  An Apple Developer account (\$99/year) is needed to:"
    echo "    • Distribute the app to other Macs"
    echo "    • Notarize (remove Gatekeeper warnings for everyone)"
    echo ""
    echo "  For personal use on your own Mac, a local self-signed"
    echo "  certificate is sufficient (created in Step 3)."
    echo ""
    read -rp "  Do you want to log in with your Apple ID now? [y/N] " REPLY
    echo ""

    if [[ "$REPLY" =~ ^[Yy]$ ]]; then
        header "Apple ID Login"
        echo "  Xcode will open to sign in. After signing in:"
        echo "    1. Xcode → Settings → Accounts → add your Apple ID"
        echo "    2. Select your team and click 'Manage Certificates'"
        echo "    3. Click '+' → Apple Development"
        echo "    4. Close Xcode and re-run this script."
        echo ""
        read -rp "  Press Enter to open Xcode, or Ctrl-C to skip... "
        open -a Xcode
        echo ""
        warn "After adding your certificate in Xcode, run this script again."
        exit 0
    fi
fi

# ── Step 3: Create a local self-signed code-signing certificate ───────────────
header "Step 3 — Creating local self-signed certificate: \"$CERT_LABEL\""

# Check if our self-signed cert already exists
EXISTING=$(security find-identity -v -p codesigning 2>/dev/null \
    | grep "$CERT_LABEL" | grep -oE '"[^"]+"' | tr -d '"' || true)

if [[ -n "$EXISTING" ]]; then
    ok "Self-signed certificate already exists: $EXISTING"
    echo ""
    info "To rebuild with it:"
    echo "     bash mac/scripts/build-release.sh"
    exit 0
fi

# Require openssl
if ! command -v openssl &>/dev/null; then
    die "openssl not found. Install it with: brew install openssl"
fi

mkdir -p "$CERT_DIR"
KEY="$CERT_DIR/clipsync.key"
CRT="$CERT_DIR/clipsync.crt"
P12="$CERT_DIR/clipsync.p12"
CNF="$TMPDIR_SIGN/codesign.cnf"

info "Generating RSA-2048 key and self-signed certificate (3650 days)..."

cat > "$CNF" <<EOF
[req]
distinguished_name = dn
x509_extensions    = ext
prompt             = no
[dn]
CN = $CERT_LABEL
O  = ClipSync Personal
[ext]
keyUsage         = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
basicConstraints = critical, CA:FALSE
subjectKeyIdentifier = hash
EOF

openssl req -x509 \
    -newkey rsa:2048 \
    -keyout "$KEY" \
    -out "$CRT" \
    -days 3650 \
    -nodes \
    -config "$CNF" \
    2>/dev/null

openssl pkcs12 -export \
    -out "$P12" \
    -inkey "$KEY" \
    -in "$CRT" \
    -passout pass: \
    2>/dev/null

ok "Certificate files written to $CERT_DIR/"

info "Importing certificate into login Keychain..."

# Import with codesign access — macOS may show a Keychain prompt
security import "$P12" \
    -k "$KEYCHAIN" \
    -P "" \
    -T /usr/bin/codesign \
    -A \
    2>/dev/null || {
        warn "Keychain import failed. Trying without -A flag..."
        security import "$P12" \
            -k "$KEYCHAIN" \
            -P "" \
            -T /usr/bin/codesign \
            2>/dev/null
    }

info "Trusting certificate for code signing..."
# This requires the user password — macOS will prompt if needed
security add-trusted-cert \
    -r trustRoot \
    -k "$KEYCHAIN" \
    "$CRT" 2>/dev/null || {
        warn "Could not add trust automatically."
        warn "Open Keychain Access → find '$CERT_LABEL' → Get Info → Trust → 'Always Trust' for Code Signing."
    }

# ── Verify ────────────────────────────────────────────────────────────────────
header "Verifying..."

FOUND=$(security find-identity -v -p codesigning 2>/dev/null \
    | grep "$CERT_LABEL" | grep -oE '"[^"]+"' | tr -d '"' || true)

if [[ -n "$FOUND" ]]; then
    echo ""
    ok "Certificate ready: $FOUND"
    echo ""
    echo -e "  ${BOLD}Next steps:${RESET}"
    echo "  1. Build and sign the app:"
    echo "       bash mac/scripts/build-release.sh"
    echo ""
    echo "  2. First time you open ClipSync.app, macOS may show a warning."
    echo "     Go to System Settings → Privacy & Security → 'Open Anyway'."
    echo ""
    echo "  Certificate files are saved in: $CERT_DIR/"
    echo "  Keep these private — they are not needed after import."
else
    echo ""
    warn "Certificate was imported but is not showing as trusted yet."
    echo ""
    echo "  Manual fix:"
    echo "  1. Open Keychain Access (Applications → Utilities)"
    echo "  2. Select 'login' keychain → 'My Certificates'"
    echo "  3. Find '$CERT_LABEL' → double-click → Trust"
    echo "  4. Set 'Code Signing' to 'Always Trust'"
    echo "  5. Re-run: bash mac/scripts/setup-signing.sh"
fi
