#!/bin/zsh
# Generates assets/Silimon.icns from assets/icon.svg.
set -euo pipefail
cd "${0:A:h}/.."

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
swift scripts/make-icon.swift assets/icon.svg "$work/master.png"
set_dir="$work/Silimon.iconset"
mkdir "$set_dir"
for size in 16 32 128 256 512; do
  sips -z $size $size "$work/master.png" --out "$set_dir/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z $double $double "$work/master.png" --out "$set_dir/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o assets/Silimon.icns
cp "$work/master.png" assets/icon.png
echo "Done: assets/Silimon.icns"
