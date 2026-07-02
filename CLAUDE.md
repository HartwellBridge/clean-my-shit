# CLAUDE.md — Clean My Shit

Cross-platform (macOS + Windows) disk-clutter cleaner. Rust + egui GUI. Free.
No daemon: launch, scan, pick, clean, quit.

- Repo: https://github.com/HartwellBridge/clean-my-shit
- Location: `~/workspace/hw/clean-my-shit`
- Website/product page lives in a **separate repo** — see "Website integration".

## Commands

```sh
cargo run --release            # run the app
cargo build --release          # build binary -> target/release/clean-my-shit
cargo test                     # 5 tests (safety + scanner find_dirs)
cargo clippy --release         # lint (keep clean)
```

## Source layout (`src/`)

- `main.rs` — eframe launch. Embeds `assets/icon.png` (`ICON_PNG`) and sets it as
  the window icon via `ViewportBuilder::with_icon` (fixes default-icon issue).
- `app.rs` — egui UI + orchestration. Phases: Idle → Scanning → Results →
  (DryReport) → Cleaning → Done. Scan/clean run on background threads, talk back
  over `mpsc`. Header shows the icon as a texture (egui font has **no glyph for
  🧹/most emoji** → don't put emoji in the UI; `♥ ★` happen to render). Has
  Dry-run toggle, per-category "Show N items" preview, `♥ Support` +
  `★ GitHub` buttons (`SUPPORT_URL`, `GITHUB_URL`, shared `link_button`).
- `categories.rs` — `Category { source, risk, default_on }`. `Source` is either
  `Contents(roots)` (purge contents of fixed cache dirs) or
  `FindDirs { base, name, sibling, max_depth }` (discover dirs anywhere under
  home). `dev_find_categories()` = node_modules, Rust `target` (requires sibling
  `Cargo.toml`), `__pycache__` — all **off by default**, Caution. Platform lists
  are `#[cfg]`-gated (mac/windows/other).
- `scanner.rs` — `targets(cat)` resolves either source to concrete paths (safety-
  filtered). `find_dirs` = bounded recursive walk: skips symlinks, hidden dirs,
  and noise (`Library`, media, `.app`…); never descends into a match. `dir_size`,
  parallel `scan_category`. Has unit tests.
- `cleaner.rs` — `clean_category` re-resolves `targets` then `purge_path` each
  (recursive delete, symlinks removed as links, counts freed bytes).
- `safety.rs` — `is_safe_target`: absolute, no `..`, and either == OS temp /
  under it, or strictly inside home (depth > home), or contains `$Recycle.Bin`.
  Second independent guard before any deletion. Has tests.
- `util.rs` — `format_size`, `short_path` (collapses home to `~`).
- `build.rs` — Windows-only: embeds `assets/icon.ico` into the `.exe` via
  `winresource` (a `target.'cfg(windows)'.build-dependencies`). No-op elsewhere.

## Icon pipeline (`tools/iconforge/`)

Standalone crate (own empty `[workspace]`, not linked into the app). Uses
`resvg` + `ico` + `image`.

```sh
cargo run --manifest-path tools/iconforge/Cargo.toml -- assets [fluent|noto|twemoji]
# -> assets/icon-1024.png, assets/icon.png (256), assets/icon.ico
```

- `icon.svg` = blue gradient rounded tile + 3 white sparkles.
- An emoji is composited on top, **centered on its opaque content bbox**, ~0.72 of
  the tile. Default source `fluent` = MS Fluent 3D poo (`poo.png`, MIT, 256px,
  Lanczos-upscaled — looks 3D but is slightly soft at 1024; this is accepted).
  `noto`/`twemoji` are vector (crisp, flat) fallbacks.
- Attribution: `tools/iconforge/THIRD-PARTY.md`. Current icon = Fluent (MIT).
- After changing the icon: rerun iconforge → `bundle.sh` (regenerates `.icns` +
  rebuilds the binary that embeds `icon.png`).

## Packaging

- macOS: `./packaging/macos/bundle.sh` → `dist/Clean My Shit.app` + `clean-my-shit.dmg`
  (built-in `sips`/`iconutil`/`hdiutil`). `SKIP_DMG=1` = app only. `Info.plist` in
  `packaging/macos/`.
- Windows: `packaging/windows/installer.nsi` + `build.ps1` (NSIS; Authenticode if
  `WINDOWS_CERT_PFX` set). **Installer not built yet** → website shows Windows as
  "coming soon".
- CI: `.github/workflows/release.yml` on `v*` tag builds both, signs/notarizes if
  the secrets are present.

## Code signing & notarization (macOS) — IMPORTANT

- Cert: **Developer ID Application: Saverino La Placa (DY9R6X78A3)** (team `DY9R6X78A3`).
- Notary creds stored in keychain profile **`cms-notary`** (`xcrun notarytool store-credentials`).
- Ship/release pipeline: `packaging/macos/sign_and_notarize.sh` — signs app
  (Hardened Runtime + timestamp) → notarizes zip → staples → builds dmg → signs →
  notarizes → staples → `spctl` check.
- **GOTCHA:** run it in a **foreground Terminal**. The first notary call triggers a
  keychain access prompt; a background/headless run fails with exit 69
  `No Keychain password item found for profile: cms-notary`. (`notarytool history
  --keychain-profile cms-notary` confirms the profile is fine.)

## Release a new version

```sh
cd ~/workspace/hw/clean-my-shit
export DEVELOPER_ID_APP="$(security find-identity -v -p codesigning | grep 'Developer ID Application' | head -1 | sed -E 's/.*"(.*)"/\1/')"
export NOTARY_PROFILE=cms-notary
./packaging/macos/sign_and_notarize.sh
# copy artifacts into the website repo:
cp dist/clean-my-shit.dmg  ~/workspace/hw/hw-future/packages/website-and-cms/public/downloads/clean-my-shit.dmg
cp assets/icon-1024.png    ~/workspace/hw/hw-future/packages/website-and-cms/public/clean-my-shit/icon.png
# then commit + deploy the website
```

## Website integration (separate repo)

`~/workspace/hw/hw-future/packages/website-and-cms` — **Next.js 15 (App Router) +
Payload CMS 3.33 + Tailwind v4 + shadcn**, site = hartwellbridge.com. Localized
under `[locale]/` (`it` default, `en`). `it.ts` is the canonical dictionary;
`en.ts` must match its shape. Typecheck: `node_modules/.bin/tsc --noEmit -p tsconfig.json`.

What was added for this app:
- Product page: `src/app/(frontend)/[locale]/clean-my-shit/page.tsx` (+ `thanks/page.tsx`).
- Sections: `src/components/site/sections/cleanmyshit/{Hero,Features,Support,config}.tsx`.
  `config.ts` has `DMG_URL`, `EXE_URL`, `GITHUB_URL`.
- Home product card: `Products.tsx` (`CleanMyShitCard`, needs `locale` prop).
- Dictionary: `src/i18n/messages/{it,en}.ts` → `cleanMyShit.*` + `products.cleanmyshit`.
- DMG served from `public/downloads/clean-my-shit.dmg`; icon `public/clean-my-shit/icon.png`
  (both copied on release; gitignore doesn't exclude them; Dockerfile copies `public/`).

External links/config:
- In-app `SUPPORT_URL` → `https://hartwellbridge.com/en/clean-my-shit#support`.
- Stripe donate link `https://donate.stripe.com/3cIbJ1cQlci7eou9cxg3600`, wired via
  `NEXT_PUBLIC_STRIPE_DONATE_URL` in `.env` + `.env.production`. Set the Stripe
  Payment Link's "after payment" redirect to `…/en/clean-my-shit/thanks`.
- `GITHUB_URL` = https://github.com/HartwellBridge/clean-my-shit.

## Open items / TODO

- Build the Windows installer (CI on a `v*` tag, or `build.ps1` on a Windows box),
  drop `CleanMyShit-Setup.exe` into the site's `public/downloads/`, flip the
  website Windows button from "coming soon" to a real download.
- Commit + deploy the website to push the latest DMG/icon + donate/GitHub.
- Icon 1024 is slightly soft (Fluent 3D source is 256px max; accepted).
