#!/bin/bash
# Meetily build & install script
# Builds the app, embeds onnxruntime, signs everything, installs to /Applications
set -e

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
APP_BUNDLE="$REPO_DIR/target/release/bundle/macos/meetily.app"
ORT_LIB="/opt/homebrew/lib/libonnxruntime.1.26.0.dylib"
INSTALL_DIR="/Applications/Meetily.app"

echo "🔨 Building Meetily..."
cd "$REPO_DIR/frontend"
# Updater signing: use the fork's private key if present so the .app.tar.gz
# updater bundle gets signed (no-op if the key isn't on this machine).
if [ -z "$TAURI_SIGNING_PRIVATE_KEY" ] && [ -f "$HOME/.tauri/meetily.key" ]; then
    export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.tauri/meetily.key")"
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
fi
ORT_LIB_LOCATION=/opt/homebrew/lib pnpm tauri build -- --features coreml 2>&1 | grep -v "^$" | tail -5 || true

if [ ! -d "$APP_BUNDLE" ]; then
    echo "❌ Build failed — no .app bundle found"
    exit 1
fi

echo "📦 Embedding ONNX Runtime..."
mkdir -p "$APP_BUNDLE/Contents/Frameworks"
cp "$ORT_LIB" "$APP_BUNDLE/Contents/Frameworks/libonnxruntime.1.dylib"
install_name_tool -id @executable_path/../Frameworks/libonnxruntime.1.dylib \
    "$APP_BUNDLE/Contents/Frameworks/libonnxruntime.1.dylib" 2>/dev/null
install_name_tool -change /opt/homebrew/opt/onnxruntime/lib/libonnxruntime.1.dylib \
    @executable_path/../Frameworks/libonnxruntime.1.dylib \
    "$APP_BUNDLE/Contents/MacOS/meetily" 2>/dev/null

echo "🔏 Signing..."
codesign --force --sign - "$APP_BUNDLE/Contents/Frameworks/libonnxruntime.1.dylib"
codesign --force --sign - "$APP_BUNDLE/Contents/MacOS/llama-helper"
codesign --force --sign - "$APP_BUNDLE/Contents/MacOS/ffmpeg"
codesign --force --sign - "$APP_BUNDLE/Contents/MacOS/meetily"
codesign --force --sign - "$APP_BUNDLE"

echo "📲 Installing..."
# Prefer /Applications, but fall back to ~/Applications if it's not writable
# (e.g. a prior install left a root-owned bundle there and we have no sudo).
install_app() {
  local dest="$1"
  rm -rf "$dest" 2>/dev/null || return 1
  cp -R "$APP_BUNDLE" "$dest" 2>/dev/null || return 1
  codesign --force --deep --sign - "$dest" 2>/dev/null
  return 0
}
if install_app "$INSTALL_DIR"; then
  echo "   → installed to $INSTALL_DIR"
else
  # Per user policy: Meetily ALWAYS lives in /Applications — never ~/Applications.
  # The plain install failed (likely a root-owned bundle left by a prior install).
  # Try to reclaim /Applications non-interactively first, then with a sudo prompt.
  echo "   ⚠️  /Applications/Meetily.app blocked (root-owned?) — reclaiming..."
  reclaim() {
    local sudo_cmd="$1"
    $sudo_cmd rm -rf "$INSTALL_DIR" 2>/dev/null \
      && $sudo_cmd cp -R "$APP_BUNDLE" "$INSTALL_DIR" 2>/dev/null \
      && $sudo_cmd chown -R "$(whoami):staff" "$INSTALL_DIR" 2>/dev/null \
      && codesign --force --deep --sign - "$INSTALL_DIR" 2>/dev/null
  }
  if reclaim "sudo -n"; then
    echo "   → reclaimed and installed to $INSTALL_DIR (passwordless sudo)"
  elif [ -t 0 ] && reclaim "sudo"; then
    echo "   → reclaimed and installed to $INSTALL_DIR (sudo)"
  else
    echo "   ❌ Could not write to /Applications. Run this once, then re-run the build:" >&2
    echo "      sudo rm -rf \"$INSTALL_DIR\" && sudo cp -R \"$APP_BUNDLE\" \"$INSTALL_DIR\" && sudo chown -R \"$(whoami):staff\" \"$INSTALL_DIR\"" >&2
    exit 1
  fi
fi

echo "🔧 Installing MCP server..."
cd "$REPO_DIR"
cargo build --release -p meetily-mcp 2>/dev/null
cp target/release/meetily-mcp ~/.local/bin/

echo "✅ Done! Meetily.app installed and meetily-mcp updated."

echo "🎤 Ensuring speaker diarization model is present..."
"$REPO_DIR/scripts/download-diarization-model.sh" || echo "   ⚠️  Diarization model download failed — speaker labels will be disabled until it's fetched."
