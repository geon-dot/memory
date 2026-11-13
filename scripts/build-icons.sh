#!/usr/bin/env bash
# Rebuilds the app and tray icons from the two pictures in src-tauri/icons/src.
#
#   scripts/build-icons.sh
#
# icon-large.png (the three characters, large) becomes the app icon and
# icon-small.png (the same three, drawn small) the menu bar / tray icon.
# Both are fan-made pictures the project's owner made; nothing else is drawn
# here but the butter tile under the app icon.
#
# Headless Chrome puts the large picture on the tile, `tauri icon` makes the
# .icns / .ico / PNG set from that 1024px image, and `sips` makes the tray
# sizes. macOS only (Chrome and sips); the results are committed, so
# building the app does not need this script.
set -euo pipefail

cd "$(dirname "$0")/.."
SRC=src-tauri/icons/src
OUT=src-tauri/icons
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# The app icon, 1024px: a rounded butter tile with the page's white dots and
# brown outline (824px, the size macOS icons leave inside the canvas), and
# the picture in the middle. The picture is 881px wide and is drawn 700px
# wide, so it is only ever scaled down.
cp "$SRC/icon-large.png" "$work/large.png"
cat > "$work/app.html" <<'HTML'
<!doctype html>
<style>html,body{margin:0;background:transparent}svg{display:block}</style>
<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <defs>
    <pattern id="dots" width="150" height="150" patternUnits="userSpaceOnUse" x="62" y="62">
      <circle cx="52" cy="52" r="15" fill="#fff" fill-opacity="0.55" />
    </pattern>
    <clipPath id="clip"><rect x="100" y="100" width="824" height="824" rx="186" /></clipPath>
  </defs>
  <g clip-path="url(#clip)">
    <rect x="100" y="100" width="824" height="824" fill="#ffe9a6" />
    <rect x="100" y="100" width="824" height="824" fill="url(#dots)" />
  </g>
  <rect x="108" y="108" width="808" height="808" rx="178" fill="none" stroke="#6b4f3a" stroke-width="16" />
  <image href="large.png" x="162" y="248" width="700" height="528" />
</svg>
HTML
"$CHROME" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
  --default-background-color=00000000 --window-size=1024,1024 \
  --screenshot="$work/app.png" "file://$work/app.html" >/dev/null 2>&1

# The app icon set. `tauri icon` also writes mobile icons; only the desktop
# files this project lists are kept.
npx tauri icon "$work/app.png" --output "$work/set" >/dev/null
for f in 32x32.png 64x64.png 128x128.png 128x128@2x.png icon.png icon.icns icon.ico \
  Square30x30Logo.png Square44x44Logo.png Square71x71Logo.png Square89x89Logo.png \
  Square107x107Logo.png Square142x142Logo.png Square150x150Logo.png \
  Square284x284Logo.png Square310x310Logo.png StoreLogo.png; do
  cp "$work/set/$f" "$OUT/$f"
done

# Tray, in the picture's own colours. The menu bar draws the icon 18pt high
# and as wide as its shape asks: 48x36px is that at 2x. Windows wants a
# square: the picture 32px wide in the middle of a clear 32x32 (enough for a
# 200% display).
mkdir -p "$OUT/tray"
sips -z 36 48 "$SRC/icon-small.png" --out "$OUT/tray/menubar.png" >/dev/null
sips -z 24 32 "$SRC/icon-small.png" --out "$work/tray.png" >/dev/null
sips -p 32 32 "$work/tray.png" --out "$OUT/tray/color.png" >/dev/null
echo "icons written to $OUT"
