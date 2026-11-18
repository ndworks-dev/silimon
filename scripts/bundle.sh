#!/bin/zsh
# Builds a release and assembles dist/Silimon.app.
# With SIGN_IDENTITY set, signs with that Developer ID. Without it, signs ad hoc.
set -euo pipefail
cd "${0:A:h}/.."

MACOSX_DEPLOYMENT_TARGET=13.0 cargo build --release --target aarch64-apple-darwin
app=dist/Silimon.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/aarch64-apple-darwin/release/silimon "$app/Contents/MacOS/Silimon"
cp assets/Silimon.icns "$app/Contents/Resources/Silimon.icns"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Silimon</string>
  <key>CFBundleIconFile</key><string>Silimon</string>
  <key>CFBundleIdentifier</key><string>dev.silimon.app</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>Silimon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>LSArchitecturePriority</key><array><string>arm64</string></array>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSRequiresNativeExecution</key><true/>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>Copyright © 2026 Nickolas de Luca Alberton. MIT License.</string>
</dict>
</plist>
PLIST
if [[ -n "${SIGN_IDENTITY:-}" ]]; then
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$app"
else
  codesign --force --sign - "$app"
fi
codesign --verify --strict "$app"
echo "Done: $PWD/$app"
