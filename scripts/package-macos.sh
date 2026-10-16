#!/usr/bin/env bash
# Builds a universal (Apple Silicon + Intel) "Ztracený Ostrov.app" into dist/.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="ztraceny-ostrov"))')
targets=(aarch64-apple-darwin x86_64-apple-darwin)
for t in "${targets[@]}"; do
  rustup target add "$t" >/dev/null
  MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release -p ztraceny-ostrov --target "$t"
done

app="dist/Ztracený Ostrov.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
binaries=()
for t in "${targets[@]}"; do binaries+=("target/$t/release/ostrov"); done
lipo -create -output "$app/Contents/MacOS/ostrov" "${binaries[@]}"

iconset=target/AppIcon.iconset
rm -rf "$iconset" && mkdir -p "$iconset"
for px in 16 32 128 256 512; do
  sips -z $px $px assets/icon-1024.png --out "$iconset/icon_${px}x${px}.png" >/dev/null
  sips -z $((px * 2)) $((px * 2)) assets/icon-1024.png --out "$iconset/icon_${px}x${px}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Ztracený Ostrov</string>
  <key>CFBundleDisplayName</key><string>Ztracený Ostrov</string>
  <key>CFBundleIdentifier</key><string>org.boxb.ztraceny-ostrov</string>
  <key>CFBundleExecutable</key><string>ostrov</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.adventure-games</string>
</dict>
</plist>
PLIST

# Ad-hoc signature so Gatekeeper lets a locally built app run.
codesign --force --deep --sign - "$app"
rm -f dist/*.zip
(cd dist && ditto -c -k --keepParent "Ztracený Ostrov.app" "ztraceny-ostrov-macos-universal.zip")
echo "Built $app"
