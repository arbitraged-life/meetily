#!/usr/bin/env bash
# Personal build wrapper: injects the (passwordless) Tauri updater signing key
# so the updater bundle is signed, then runs the normal build-and-install.
set -euo pipefail

KEY_FILE="${HOME}/.tauri/meetily.key"
if [[ ! -f "${KEY_FILE}" ]]; then
  echo "❌ Updater signing key not found at ${KEY_FILE}" >&2
  exit 1
fi

export TAURI_SIGNING_PRIVATE_KEY="$(cat "${KEY_FILE}")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

exec "${SCRIPT_DIR}/build-and-install.sh"
