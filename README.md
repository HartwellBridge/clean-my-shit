# Clean My Shit

A tiny cross-platform disk clutter cleaner — like CleanMyMac / CCleaner, but
focused on one job: **freeing space**. Built in Rust with a simple GUI.

- **Cross-platform** — macOS & Windows (also builds on Linux for development).
- **No daemon** — launch it, scan, clean, quit. Nothing runs in the background.
- **Safe by design** — it only ever targets known cache / temp / log / trash
  locations, never your documents. Nothing is deleted until you review the list
  and confirm. Irreversible items (Trash, Recycle Bin) are **off by default**.

## What it cleans

**macOS**
- Application caches (`~/Library/Caches`)
- Application logs (`~/Library/Logs`)
- Temporary files (`$TMPDIR`)
- Xcode Derived Data, Device Support, Simulator caches
- npm & Cargo download caches
- Trash *(opt-in)*

**Windows**
- User & Windows temp files
- Crash dumps
- Chrome & Edge on-disk caches (bookmarks/history untouched)
- npm, pip & Cargo download caches
- Recycle Bin *(opt-in)*

## How it works

1. **Scan** — walks each location in parallel and measures reclaimable bytes,
   plus a per-item breakdown. Symlinks are never followed.
2. **Review** — each category shows its size; expand **Show N items** to see
   exactly what's inside. Tick what you want gone.
3. **Dry run** *(optional)* — tick **Dry run** and press **Preview** to see the
   full list of everything that *would* be deleted, and how much it'd free,
   without removing anything.
4. **Clean** — after a confirmation dialog, the selected contents are deleted
   and the total freed space is reported.

Two independent safety layers stand between a click and a deletion: the
category list only contains cache/temp paths, and `safety::is_safe_target`
refuses any path that isn't clearly a per-user cache/temp/trash directory.

## Build & run

```sh
cargo run --release
```

The release binary lands at `target/release/clean-my-shit`
(`clean-my-shit.exe` on Windows) — a single self-contained executable.

## Packaging (installers)

The app icon is generated procedurally — no image assets in the repo:

```sh
cargo run --manifest-path tools/iconforge/Cargo.toml -- assets
```

**macOS** — builds `Clean My Shit.app` and a `.dmg` (uses only built-in
`sips` / `iconutil` / `hdiutil`):

```sh
./packaging/macos/bundle.sh        # -> dist/Clean My Shit.app, dist/clean-my-shit.dmg
```

**Windows** — builds the `.exe` (icon embedded) and an NSIS installer.
Needs [NSIS](https://nsis.sourceforge.io) on `PATH` (`winget install NSIS.NSIS`):

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\build.ps1   # -> dist\CleanMyShit-Setup.exe
```

**Both at once** — push a `v*` tag (or run the workflow manually) and
`.github/workflows/release.yml` builds the macOS `.dmg` and the Windows
installer on real runners and uploads them as artifacts.

## Code signing & notarization

Signing is optional and credential-driven — every script self-skips when its
credentials are absent, so unsigned dev builds always work.

**macOS** (needs an Apple Developer Program account + a *Developer ID
Application* certificate). The script signs the app with Hardened Runtime,
notarizes both the `.app` and the `.dmg`, and staples the tickets so they launch
with no Gatekeeper prompt — even offline:

```sh
export DEVELOPER_ID_APP="Developer ID Application: Your Name (TEAMID)"
# notary credentials — either a stored profile…
export NOTARY_PROFILE="my-profile"     # via: xcrun notarytool store-credentials
# …or an Apple ID + app-specific password:
export APPLE_ID="you@example.com"
export APPLE_TEAM_ID="TEAMID"
export APPLE_APP_PASSWORD="abcd-efgh-ijkl-mnop"

./packaging/macos/sign_and_notarize.sh   # -> signed, notarized, stapled dist/clean-my-shit.dmg
```

**Windows** (Authenticode). Set the cert env vars and `build.ps1` signs the
`.exe` and the installer with a timestamp:

```powershell
$env:WINDOWS_CERT_PFX = "C:\path\to\cert.pfx"
$env:WINDOWS_CERT_PASSWORD = "…"
powershell -ExecutionPolicy Bypass -File packaging\windows\build.ps1
```

**In CI**, add these as repository secrets and the release workflow signs
automatically: `DEVELOPER_ID_APP`, `MACOS_CERT_P12` (base64 of the `.p12`),
`MACOS_CERT_PASSWORD`, `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_PASSWORD`,
`WINDOWS_CERT_P12` (base64 of the `.pfx`), `WINDOWS_CERT_PASSWORD`.

## Tech

- [`eframe`/`egui`](https://github.com/emilk/egui) — immediate-mode GUI, single binary, no system webview.
- [`walkdir`](https://crates.io/crates/walkdir) + [`rayon`](https://crates.io/crates/rayon) — parallel directory walking.
- [`dirs`](https://crates.io/crates/dirs) — cross-platform standard directories.

## Release a new version

Run from your own Terminal (so the notary keychain credential is reachable):

```sh
# 1. rebuild + re-sign + notarize
cd ~/workspace/hw/clean-my-shit
export DEVELOPER_ID_APP="$(security find-identity -v -p codesigning | grep 'Developer ID Application' | head -1 | sed -E 's/.*"(.*)"/\1/')"
export NOTARY_PROFILE=cms-notary
./packaging/macos/sign_and_notarize.sh

# 2. copy the signed .dmg into the website
cp dist/clean-my-shit.dmg ~/workspace/hw/hw-future/packages/website-and-cms/public/downloads/clean-my-shit.dmg

# 3. commit + redeploy the site
```

## License

MIT
