#!/usr/bin/env bash
# Rebuilds the app and tray icons from the SVG drawings in src-tauri/icons/src.
#
#   scripts/build-icons.sh
#
# The drawings are this project's own (칩이, the widget's mascot). Headless
# Chrome turns each SVG into a PNG with a transparent background, `sips`
# makes the small sizes, and `tauri icon` makes the .icns / .ico / PNG set
# from the 1024px app icon. macOS only (Chrome and sips); the results are
# committed, so building the app does not need this script.
set -euo pipefail

cd "$(dirname "$0")/.."
SRC=src-tauri/icons/src
OUT=src-tauri/icons
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# render <svg> <png> <pixels>: the SVG scaled to a square of that many pixels.
render() {
  local page="$work/page.html"
  printf '<!doctype html><style>html,body{margin:0;background:transparent}img{display:block;width:%spx;height:%spx}</style><img src="file://%s">' \
    "$3" "$3" "$PWD/$1" > "$page"
  "$CHROME" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --default-background-color=00000000 --window-size="$3,$3" \
    --screenshot="$2" "file://$page" >/dev/null 2>&1
}

render "$SRC/app-icon.svg" "$work/app.png" 1024
render "$SRC/tray-color.svg" "$work/tray-color.png" 512
render "$SRC/tray-template.svg" "$work/tray-template.png" 504

# The app icon set. `tauri icon` also writes mobile icons; only the desktop
# files this project lists are kept.
npx tauri icon "$work/app.png" --output "$work/set" >/dev/null
for f in 32x32.png 64x64.png 128x128.png 128x128@2x.png icon.png icon.icns icon.ico \
  Square30x30Logo.png Square44x44Logo.png Square71x71Logo.png Square89x89Logo.png \
  Square107x107Logo.png Square142x142Logo.png Square150x150Logo.png \
  Square284x284Logo.png Square310x310Logo.png StoreLogo.png; do
  cp "$work/set/$f" "$OUT/$f"
done

# Tray: 36px is the 18pt menu bar icon at 2x; 32px covers Windows up to 200%.
mkdir -p "$OUT/tray"
sips -z 36 36 "$work/tray-template.png" --out "$OUT/tray/template.png" >/dev/null
sips -z 32 32 "$work/tray-color.png" --out "$OUT/tray/color.png" >/dev/null
echo "icons written to $OUT"
