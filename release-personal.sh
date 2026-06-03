#!/usr/bin/env bash
# Lightweight PERSONAL release for Meetily (arbitraged-life fork).
#
# This is NOT a public release pipeline. It exists so that *your own* installs
# can auto-update from a signed bundle you built locally. No GitHub release is
# required — it assembles the updater artifacts + a latest.json into
#   ./dist/release/<version>/
# which you can either (a) keep local, or (b) drag onto a GitHub release if you
# ever want over-the-air updates between your own machines.
#
# What it does:
#   1. Optional version bump (--bump patch|minor|major  OR  --version X.Y.Z)
#   2. Signed build via ./build-personal.sh (injects ~/.tauri/meetily.key)
#   3. Collects meetily.app.tar.gz + .sig and writes a Tauri-format latest.json
#
# USAGE:
#   ./release-personal.sh                  # build current version, make latest.json
#   ./release-personal.sh --bump patch     # 0.3.0 -> 0.3.1, then build
#   ./release-personal.sh --version 0.4.0  # set explicit version, then build
#   ./release-personal.sh --notes "Mic auto-pick + hotkeys + key registry"

set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
CONF="${REPO_DIR}/frontend/src-tauri/tauri.conf.json"
BUNDLE_DIR="${REPO_DIR}/target/release/bundle/macos"
NOTES="Personal build."
BUMP=""
SET_VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bump)    BUMP="$2"; shift 2 ;;
    --version) SET_VERSION="$2"; shift 2 ;;
    --notes)   NOTES="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

# --- resolve version -------------------------------------------------------
CUR_VERSION="$(python3 -c "import json;print(json.load(open('${CONF}'))['version'])")"
NEW_VERSION="${CUR_VERSION}"

if [[ -n "${SET_VERSION}" ]]; then
  NEW_VERSION="${SET_VERSION}"
elif [[ -n "${BUMP}" ]]; then
  NEW_VERSION="$(python3 - "$CUR_VERSION" "$BUMP" <<'PY'
import sys
ver, part = sys.argv[1], sys.argv[2]
major, minor, patch = (int(x) for x in ver.split("."))
if part == "major": major, minor, patch = major + 1, 0, 0
elif part == "minor": minor, patch = minor + 1, 0
elif part == "patch": patch += 1
else: sys.exit(f"bad --bump: {part}")
print(f"{major}.{minor}.{patch}")
PY
)"
fi

if [[ "${NEW_VERSION}" != "${CUR_VERSION}" ]]; then
  echo "🔖 Bumping version ${CUR_VERSION} -> ${NEW_VERSION}"
  python3 - "$CONF" "$NEW_VERSION" <<'PY'
import json, sys
conf_path, new = sys.argv[1], sys.argv[2]
data = json.load(open(conf_path))
data["version"] = new
json.dump(data, open(conf_path, "w"), indent=4)
open(conf_path, "a").write("\n")
PY
fi

# --- build (signed) --------------------------------------------------------
echo "🏗  Building signed release v${NEW_VERSION}…"
"${REPO_DIR}/build-personal.sh"

# --- collect updater artifacts --------------------------------------------
TARBALL="${BUNDLE_DIR}/meetily.app.tar.gz"
SIGFILE="${TARBALL}.sig"
[[ -f "${TARBALL}" ]] || { echo "❌ Missing updater tarball: ${TARBALL}" >&2; exit 1; }
[[ -f "${SIGFILE}"  ]] || { echo "❌ Missing signature (.sig). Did signing succeed?  ${SIGFILE}" >&2; exit 1; }

OUT_DIR="${REPO_DIR}/dist/release/${NEW_VERSION}"
mkdir -p "${OUT_DIR}"
cp "${TARBALL}" "${OUT_DIR}/"
cp "${SIGFILE}" "${OUT_DIR}/"

SIG_CONTENT="$(cat "${SIGFILE}")"
PUB_URL="https://github.com/arbitraged-life/meetily/releases/download/v${NEW_VERSION}/meetily.app.tar.gz"
PUB_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Tauri v2 updater manifest. The `url` only matters if you publish to a GitHub
# release; for purely-local installs the file presence is what counts.
python3 - "$OUT_DIR/latest.json" "$NEW_VERSION" "$PUB_DATE" "$NOTES" "$SIG_CONTENT" "$PUB_URL" <<'PY'
import json, sys
out, ver, pub_date, notes, sig, url = sys.argv[1:7]
manifest = {
    "version": ver,
    "notes": notes,
    "pub_date": pub_date,
    "platforms": {
        "darwin-aarch64": {"signature": sig, "url": url},
    },
}
json.dump(manifest, open(out, "w"), indent=2)
print(out)
PY

echo
echo "✅ Personal release v${NEW_VERSION} ready:"
echo "   ${OUT_DIR}/"
echo "     • meetily.app.tar.gz       (signed updater bundle)"
echo "     • meetily.app.tar.gz.sig   (minisign signature)"
echo "     • latest.json              (Tauri updater manifest)"
echo
echo "App is already installed to /Applications by build-personal.sh."
echo "To enable over-the-air updates between your machines, upload all three"
echo "files to a GitHub release tagged 'v${NEW_VERSION}' on arbitraged-life/meetily."
