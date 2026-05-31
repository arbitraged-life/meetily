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

echo "📲 Installing to /Applications..."
rm -rf "$INSTALL_DIR"
cp -R "$APP_BUNDLE" "$INSTALL_DIR"

echo "🔧 Installing MCP server..."
cd "$REPO_DIR"
cargo build --release -p meetily-mcp 2>/dev/null
cp target/release/meetily-mcp ~/.local/bin/

echo "✅ Done! Meetily.app installed and meetily-mcp updated."
