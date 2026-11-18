#!/bin/zsh
# Compila em release e monta dist/Silimon.app com assinatura ad-hoc.
set -euo pipefail
cd "${0:A:h}/.."

cargo build --release
app=dist/Silimon.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/silimon "$app/Contents/MacOS/Silimon"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Silimon</string>
  <key>CFBundleIdentifier</key><string>dev.silimon.app</string>
  <key>CFBundleName</key><string>Silimon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSArchitecturePriority</key><array><string>arm64</string></array>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$app"
echo "Pronto: $PWD/$app"
