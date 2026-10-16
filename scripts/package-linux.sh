#!/usr/bin/env bash
# Builds dist/ztraceny-ostrov-linux-x86_64.AppImage. Needs libfuse2 to run linuxdeploy.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -p ztraceny-ostrov

appdir=target/AppDir
rm -rf "$appdir"
install -Dm755 target/release/ostrov "$appdir/usr/bin/ostrov"
install -Dm644 assets/ztraceny-ostrov.desktop "$appdir/usr/share/applications/ztraceny-ostrov.desktop"
install -Dm644 assets/icon-256.png "$appdir/usr/share/icons/hicolor/256x256/apps/ztraceny-ostrov.png"

tool=target/linuxdeploy-x86_64.AppImage
sha=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
if [ ! -f "$tool" ]; then
  curl -fsSL -o "$tool" \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage
fi
echo "$sha  $tool" | sha256sum -c - || { rm -f "$tool"; exit 1; }
chmod +x "$tool"
mkdir -p dist
OUTPUT="dist/ztraceny-ostrov-linux-x86_64.AppImage" "$tool" --appdir "$appdir" --output appimage
