#!/usr/bin/env bash
#
# Build the macOS app bundle (.app) and a distributable disk image (.dmg).
# Uses only the macOS built-in tools: cargo, sips, iconutil, hdiutil.
#
# Usage:  ./packaging/macos/bundle.sh
# Output: dist/Clean My Shit.app  and  dist/clean-my-shit.dmg

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP_NAME="Clean My Shit"
BIN_NAME="clean-my-shit"
DIST="$ROOT/dist"
APP="$DIST/$APP_NAME.app"
SRC_ICON="$ROOT/assets/icon-1024.png"

echo "==> Building release binary"
cargo build --release --manifest-path "$ROOT/Cargo.toml"

if [[ ! -f "$SRC_ICON" ]]; then
  echo "==> Generating icon assets"
  cargo run --release --manifest-path "$ROOT/tools/iconforge/Cargo.toml" -- "$ROOT/assets"
fi

echo "==> Building AppIcon.icns"
ICONSET="$DIST/AppIcon.iconset"
rm -rf "$APP" "$ICONSET"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s"           "$SRC_ICON" --out "$ICONSET/icon_${s}x${s}.png"     >/dev/null
  d=$((s * 2))
  sips -z "$d" "$d"           "$SRC_ICON" --out "$ICONSET/icon_${s}x${s}@2x.png"  >/dev/null
done
iconutil -c icns "$ICONSET" -o "$DIST/AppIcon.icns"

echo "==> Assembling $APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/target/release/$BIN_NAME" "$APP/Contents/MacOS/$BIN_NAME"
cp "$DIST/AppIcon.icns"             "$APP/Contents/Resources/AppIcon.icns"
cp "$ROOT/packaging/macos/Info.plist" "$APP/Contents/Info.plist"
chmod +x "$APP/Contents/MacOS/$BIN_NAME"

# Clean up scratch artifacts.
rm -rf "$ICONSET" "$DIST/AppIcon.icns"

# SKIP_DMG=1 stops here with just the .app (used by the signing pipeline, which
# must sign + notarize + staple the app *before* the DMG is built around it).
if [[ -n "${SKIP_DMG:-}" ]]; then
  echo "==> Done (app only): $APP"
  exit 0
fi

echo "==> Creating DMG"
hdiutil create -volname "$APP_NAME" -srcfolder "$APP" -ov -format UDZO \
  "$DIST/$BIN_NAME.dmg" >/dev/null

echo "==> Done:"
echo "    $APP"
echo "    $DIST/$BIN_NAME.dmg"
