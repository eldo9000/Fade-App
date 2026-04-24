#!/usr/bin/env bash
# Build a signed Fade release bundle (DMG + updater tarball with signature).
#
# One-time setup — run this once, paste the passphrase you chose when the
# signing key was generated, then press Enter:
#     security add-generic-password -s tauri-fade -a "$USER" -w
#
# The password is stored in the macOS Keychain (service name: tauri-fade) and
# pulled at build time — it never touches disk in plaintext.
set -euo pipefail

KEY_PATH="${TAURI_SIGNING_KEY_PATH:-$HOME/.tauri/fade.key}"
KEYCHAIN_SERVICE="${TAURI_SIGNING_KEYCHAIN_SERVICE:-tauri-fade}"

if [[ ! -f "$KEY_PATH" ]]; then
  echo "Signing key not found at $KEY_PATH" >&2
  echo "Regenerate with: npx tauri signer generate -w $KEY_PATH" >&2
  exit 1
fi

if ! PASS="$(security find-generic-password -s "$KEYCHAIN_SERVICE" -a "$USER" -w 2>/dev/null)"; then
  echo "Passphrase for signing key not found in Keychain." >&2
  echo "Seed it once with:" >&2
  echo "    security add-generic-password -s $KEYCHAIN_SERVICE -a \"\$USER\" -w" >&2
  exit 1
fi

export TAURI_SIGNING_PRIVATE_KEY="$KEY_PATH"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$PASS"

exec npm run tauri -- build "$@"
