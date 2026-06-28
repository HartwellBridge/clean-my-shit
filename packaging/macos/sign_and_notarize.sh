#!/usr/bin/env bash
#
# Code-sign (Developer ID), notarize and staple the macOS app + DMG.
#
# Produces a fully notarized, stapled dist/clean-my-shit.dmg that launches on
# any Mac without Gatekeeper warnings — and works offline (stapled ticket).
#
# Requirements (Apple Developer Program account):
#   - A "Developer ID Application" certificate in your login keychain.
#   - Xcode command line tools (codesign, notarytool, stapler).
#
# Configuration via environment variables:
#   DEVELOPER_ID_APP   "Developer ID Application: Your Name (TEAMID)"  (required)
#
#   Notarization credentials — either a stored keychain profile:
#     NOTARY_PROFILE   name created via `xcrun notarytool store-credentials`
#   ...or an Apple ID + app-specific password:
#     APPLE_ID         your-apple-id@example.com
#     APPLE_TEAM_ID    TEAMID
#     APPLE_APP_PASSWORD   app-specific password from appleid.apple.com
#
# If DEVELOPER_ID_APP is unset the script exits 0 without signing, so it is safe
# to call unconditionally in CI for unsigned builds.
#
# Usage:  ./packaging/macos/sign_and_notarize.sh

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP_NAME="Clean My Shit"
BIN_NAME="clean-my-shit"
DIST="$ROOT/dist"
APP="$DIST/$APP_NAME.app"
DMG="$DIST/$BIN_NAME.dmg"
ENTITLEMENTS="$ROOT/packaging/macos/entitlements.plist"

IDENTITY="${DEVELOPER_ID_APP:-}"

if [[ -z "$IDENTITY" ]]; then
  echo "DEVELOPER_ID_APP not set — skipping signing/notarization (unsigned build)."
  exit 0
fi

# Submit a file to the notary service and block until it is Accepted (notarytool
# returns non-zero on Invalid, which `set -e` turns into a failure).
notarize_submit() {
  local file="$1"
  echo "==> Notarizing $(basename "$file") (this can take a few minutes)"
  if [[ -n "${NOTARY_PROFILE:-}" ]]; then
    xcrun notarytool submit "$file" --keychain-profile "$NOTARY_PROFILE" --wait
  else
    : "${APPLE_ID:?set APPLE_ID or NOTARY_PROFILE}"
    : "${APPLE_TEAM_ID:?set APPLE_TEAM_ID or NOTARY_PROFILE}"
    : "${APPLE_APP_PASSWORD:?set APPLE_APP_PASSWORD or NOTARY_PROFILE}"
    xcrun notarytool submit "$file" \
      --apple-id "$APPLE_ID" \
      --team-id "$APPLE_TEAM_ID" \
      --password "$APPLE_APP_PASSWORD" \
      --wait
  fi
}

# 1. Ensure the .app exists (build it, without a DMG).
if [[ ! -d "$APP" ]]; then
  SKIP_DMG=1 "$ROOT/packaging/macos/bundle.sh"
fi

# 2. Sign inside-out (executable first, then the bundle) with Hardened Runtime
#    and a secure timestamp — both mandatory for notarization.
echo "==> Signing app with: $IDENTITY"
SIGN_ARGS=(--force --options runtime --timestamp --sign "$IDENTITY")
if [[ -f "$ENTITLEMENTS" ]]; then
  SIGN_ARGS+=(--entitlements "$ENTITLEMENTS")
fi
codesign "${SIGN_ARGS[@]}" "$APP/Contents/MacOS/$BIN_NAME"
codesign "${SIGN_ARGS[@]}" "$APP"
codesign --verify --strict --verbose=2 "$APP"

# 3. Notarize the app (zip is the upload format), then staple the ticket so the
#    app is trusted even with no network on first launch.
APP_ZIP="$DIST/$BIN_NAME-app.zip"
ditto -c -k --keepParent "$APP" "$APP_ZIP"
notarize_submit "$APP_ZIP"
rm -f "$APP_ZIP"
xcrun stapler staple "$APP"
xcrun stapler validate "$APP"

# 4. Build the DMG around the now-stapled app, then sign it.
echo "==> Building signed DMG"
rm -f "$DMG"
hdiutil create -volname "$APP_NAME" -srcfolder "$APP" -ov -format UDZO "$DMG" >/dev/null
codesign --force --timestamp --sign "$IDENTITY" "$DMG"

# 5. Notarize + staple the DMG itself.
notarize_submit "$DMG"
xcrun stapler staple "$DMG"
xcrun stapler validate "$DMG"

# 6. Final Gatekeeper assessment.
echo "==> Gatekeeper assessment"
spctl --assess --type exec --verbose=2 "$APP" || true

echo "==> Signed, notarized & stapled:"
echo "    $APP"
echo "    $DMG"
