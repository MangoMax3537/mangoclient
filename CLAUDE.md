# MangoClient

Minecraft launcher (profiles, Modrinth, Microsoft/offline accounts, bundled MangoConfig in-game mod).
Rust rewrite of the former Electron launcher (v1.3.4), with a fully redesigned UI. The old Electron code is gone from this tree: read it with `git show master:src/main/<file>.js` (history), or from the working copy in `/tmp/mangoclient-legacy/` (src/, test/, tools/, CI; lives in RAM and is cleared on reboot). Paths like `src/main/x.js` in these docs refer to that legacy source.
Targets: Windows 10/11 x64, macOS 11+ (arm64 + x64), Linux x86_64 (glibc 2.31+). All three are first-class.

## Layout

| Path | Status |
| --- | --- |
| `crates/` | Rust workspace members (see below). |
| `src-tauri/` | Tauri 2 shell (own `[workspace]`, excluded from the root workspace so `cargo test --workspace` needs no webkit). |
| `ui/` | Svelte 5 + TypeScript frontend (Vite, npm). |
| `bundle/mangoconfig/` | The 14 prebuilt MangoConfig jars shipped with the launcher (`MangoConfig-<ver>+mc<mc>.jar`). |
| `assets/brand/` | The real MangoClient icon (`icon.png`, `icons/`), source of truth for app icons and fallback image. |
| `mod/` | MangoConfig Fabric mod (Java/Gradle). **Stays Java.** Only the launcher interface matters. |
| `server/` | Go presence/stats service. **Unchanged.** Wire contract frozen. |
| `tools/` | `build-mods.js` (builds the jars into `bundle/mangoconfig/`), `mango-badges.ps1`. |
| `docs/` | `port-spec.md`, `ipc.md`, `redesign-brief.md`, `ideas.md`. |

Run: `cargo test --workspace`; UI: `cd ui && npm run dev`; desktop app: `cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol` (needs `webkit2gtk-4.1` on Linux).

## Target architecture (decided unless the user overrides)

Tauri 2 shell + Rust core + a typed TypeScript/Svelte frontend. Chosen over pure-Rust UI (iced/egui/Slint) because the redesign needs rich motion, imagery and the 3D skin viewer, and the existing WebGL viewer ports as-is. Business logic lives only in Rust; the frontend is a thin client over typed commands/events.

```
Cargo.toml                  workspace (root)
crates/
    core/        profiles, settings, store, migrations, paths   (no UI, no Tauri)
    auth/        Microsoft -> Xbox -> XSTS -> Minecraft, offline accounts, credential vault
    net/         reqwest client, download w/ sha512+size verify, retry, progress
    java/        detect local JDKs, download Temurin, per-MC-version requirement
    minecraft/   version manifests, libraries/assets/natives, launch command, loaders
    loaders/     fabric, quilt, forge, neoforge installers
    modrinth/    search, versions, dependency resolution, modpack (.mrpack) install
    content/     mods/resourcepacks/shaders routing, local jar scan, enable/disable
    servers/     servers.dat NBT (modified UTF-8), ping, icons, partner list
    mango/       MangoConfig jar injection, telemetry, presence API base validation
    archive/     safe zip/tar extraction (traversal, symlink, size limits)
    updater/     signed updates
  src-tauri/   commands, events, window, tray, capabilities
  ui/          frontend
xtask/         build-mods, release packaging
```

Rules: `core`, `auth`, `net`, `java`, `minecraft`, `loaders`, `modrinth`, `content`, `servers`, `archive` must not depend on Tauri, so they are testable and reusable from a CLI.

## Compatibility contracts (do not break)

- **Data root and formats**: Windows `%APPDATA%\.mangoclient`, macOS `~/Library/Application Support/mangoclient`, Linux `~/.mangoclient`. Existing profiles, accounts, settings, `install.json`, `servers.dat` must load without user action. Migrations are one-way, versioned, and keep a backup.
- **appId** `dev.mangomax.mangoclient` (Windows upgrade in place).
- **Credentials (hard requirement: tokens must not be stealable by a RAT)**: Microsoft tokens are never stored in plaintext, anywhere, ever (no 0600 plaintext fallback anymore). Design: a random 256-bit per-device key generated on first run (OS CSPRNG) lives only in the OS keystore (Windows Credential Manager/DPAPI, macOS Keychain, Linux Secret Service) and is never written to disk; account secrets are encrypted with XChaCha20-Poly1305 (random nonce, AAD binds account uuid and format version) into an envelope in `accounts.json`, so a copied `accounts.json` is useless on another device. Only the MSA refresh token is persisted; access tokens live in memory only (zeroized, redacted Debug) and are re-derived at launch. If no keystore is available or it is locked, nothing is persisted (memory-only session, UI shows a notice) and any existing envelope is preserved untouched. Legacy plaintext and v1 envelopes are migrated and scrubbed. Honest limit: malware running as the same user in the same login session can often call the same OS APIs, so this protects against file theft, other users, copied profiles and disk forensics, and is complemented by short token lifetime and no tokens in logs/UI. Existing Electron `encryptedCredentials` blobs (Chromium OSCrypt) are not readable: force one re-login (open decision 5).
- **accounts.json holds both accounts and profiles**, and mod records store absolute `file` paths. Keep reading that shape. Full details: `docs/port-spec.md`.
- **Mod injection** (`src/main/mangoconfig.js`): jars `MangoConfig-<ver>+mc<mcver>.jar`; inject only for fabric/quilt with an exactly matching MC version (1.20.5, 1.20.6, 1.21 through 1.21.11); delete stale `^MangoConfig-.*\.jar(\.disabled)?$` first (two jars with one id crash Fabric); skip copy when size is identical; never throw, log and continue; honor `profile.mangoConfig === false` and the global toggle; hide the jar from the mod list. Optional `-Dmangoconfig.apiBase` must pass the HTTPS-origin-only check.
- **Server API** (frozen): `POST /v1/launcher {id, version, os}` every 60s (8s timeout), base from `MANGO_API_BASE`, HTTPS origin only or the exact legacy `http://94.249.184.45:8880`. Telemetry is optional and off-switchable. Payload stays exactly: random install id, version, platform.
- **Security** (port every one): exact navigation/external-URL allow-list; image paths reject traversal, sibling prefixes and escaping symlinks; profile fs targets need canonical UUIDs; archive extraction rejects traversal/symlinks and caps entry count, per-file and total size; downloads verified by SHA-512 and exact size, temp file deleted on failure, Content-Length 0 from a CDN ignored; typed, bounded config validation (reject mutable ids, unsafe versions, bad colours, bad loaders).
- **Persistence**: JSON saves keep a last-known-good generation, recover from truncation, and the first save writes two readable copies. A stale content record must never make a whole instance disappear.
- **servers.dat**: NBT strings are Java modified UTF-8 (NUL = C0 80, astral chars as surrogate pairs). Partner servers deduplicated, starred, first. Never rewrite an existing player list when seeding.

## Feature scope (parity first, then new)

Profiles (vanilla/fabric/quilt/forge/neoforge, per-profile RAM/Java args/cover/playtime), Play/Stop with staged progress (preparing, java, loader, downloading, account, launching, running), console + logs + mclo.gs share, Microsoft + offline accounts with skin change and 3D viewer, Modrinth browse/install for mods, modpacks, resource packs, shaders with dependency resolution and updates, manual jar detection + enable/disable, auto Java (Temurin), performance-mod management (Sodium, Lithium, FerriteCore), servers with live ping and quick join, screenshots, stats, storage manager, onboarding, self-update, EN + DE.

## Phases

1. **Spec**: port-by-port checklist from the legacy modules; frozen JSON fixtures copied from a real 1.3.4 data dir.
2. **Core crates** with tests (net, archive, store, auth, java, minecraft, loaders, modrinth, content, servers, mango).
3. **Headless CLI** (`mango-cli launch <profile>`) proving a real vanilla + fabric launch on all three OSes.
4. **Tauri shell + UI** implementing the approved design (see `docs/redesign-brief.md`).
5. **Release**: CI matrix, installers, signed updater, migration from Electron users.

## Working rules

- Rust stable, edition 2021+, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace` must pass before any commit. `unsafe` needs a comment. No `unwrap()` in non-test code outside provably-infallible spots.
- Errors: `thiserror` in libraries, typed error enums surfaced to the UI as stable codes plus localized messages. Never show raw error strings as the only UI.
- Async: `tokio`. HTTP: `reqwest` (rustls, no OpenSSL; makes Linux/macOS builds simple). Serialization: `serde`. Zip: `zip`; tar: `tar` + `flate2`. NBT: hand-rolled or `fastnbt` only if it handles modified UTF-8. Keychain: `keyring` with an explicit fallback path. Logging: `tracing`.
- Every platform path goes through one `paths` module. No `cfg!` sprinkled through business logic.
- Linux: do not assume a desktop environment; test AppImage on a clean glibc 2.31 container. macOS: test on both arm64 and x64. Windows: long paths and spaces in usernames.
- Nothing in the UI is hard-coded text. All strings go through i18n (Fluent or typed key map with plurals). Legacy keys (~370 in `src/renderer/js/i18n.js`) are the seed.
- Tests: port behavior, not source text. Legacy regex-on-source tests (`mod-source`, `mod-sync-wiring`, `renderer-style`) are dropped except: manual-mods-sync ordering, legacy inline icon migration before profile validation, offline fallback skin = Mojang's official 64x64 Steve.
- Do not touch `server/` or `mod/` unless the task says so. A mod version bump also needs a `CHANGELOG.md` entry.
- Do not commit signing keys, tokens, or `.env` files. Updater signing key lives in CI secrets only.
- Commits: small, imperative subject, one concern each. No `Co-Authored-By` lines unless the user asks.

## Design principles (UI)

**Icons and assets: never draw our own.** Use an existing icon library: Phosphor Icons (MIT, `phosphor-svelte`, tree-shaken, regular/bold/duotone weights; use one weight consistently, bold for the rail and primary actions). Lucide (ISC, `lucide-svelte`) is the accepted alternative if a glyph is missing. Do not mix more than these two, and do not hand-write SVG paths. Same for fonts (Google Fonts or Fontsource, self-hosted in the build, never CDN at runtime), loader and brand logos (official brand kits or Simple Icons), and illustrations (licensed or original files, see below). Check each license and list it in `app/ui/assets/ATTRIBUTIONS.md`. The mockup's inline SVG icons are stand-ins.

**Image-first, never letters, never generated art.** No placeholder letters or initials anywhere, and NO placeholder, generated, hand-drawn or Minecraft-style fake images (no scripts that paint tiles or panoramas, no SVG scenes). Every entity always shows a real image. Fallback chain: the user's own image, then the real source (Modrinth icon, server favicon, skin head from the Mojang skin), then the real MangoClient icon (`src/assets/icon.png`, sized variants in `src/assets/icons/`, bundled in `app/ui/src/assets/brand/`). Heroes use the profile's real cover when it exists, otherwise a clean token-colored surface with the MangoClient icon as a low-contrast watermark. All fallbacks go through one `fallbackImage(kind, id)` function so real art supplied later (commissioned or licensed, never copied from Mojang or Modrinth) can be dropped in without touching anything else. Skeleton loaders are image-shaped, not text.

Professional, distinctive, not a generic dark dashboard, no AI-slop tells: no purple-blue gradients, no glassmorphism cards-in-cards, no emoji as icons, no stock "rocket" illustrations, no identical rounded-rect grids, no lorem-ish marketing copy. It should feel like a game launcher: real Minecraft imagery (panoramas, biome art, block textures at integer scale), strong typographic hierarchy, purposeful motion (<250ms, ease-out, respects reduced-motion), density that rewards power users. Mango orange `#ffad32` is the brand accent. Full brief: `docs/redesign-brief.md`. Accessibility: keyboard-complete, visible focus, 4.5:1 contrast, scalable text.

## Network hosts (privacy)

The app talks only to the hosts listed in `docs/port-spec.md` (Mojang, Microsoft/Xbox, Modrinth (incl. cdn.modrinth.com icons, fetched and cached by Rust, never by the webview), Adoptium, loader metas, mclo.gs, GitHub releases, the MangoClient API). No third-party image/CDN/analytics hosts in the UI. Account heads come from the skin cache in `mango-auth` (Mojang session server), served through the image protocol. Heads are now served via `mangoimg://head/<uuid>` (no `mc-heads.net`).

## Porting the tests (checklist)

archive (traversal, symlinks, limits) · content routing + dependency trees + exact-version reuse · credentials (envelope migration, lock-preservation, 0600 fallback) · net (sha512, size, temp cleanup, CL=0) · persistence/store-recovery · security (URL policy, API base, image paths, UUID targets) · validation · serverlist (NBT mUTF-8, dedupe, recovery, no-rewrite seeding, icon cache) · servericons (first valid favicon is durable).

## Known open decisions (ask the user, do not guess)

1. **Update path for existing Electron users**: keep publishing `latest*.yml` so v1.3.4 can auto-update into the Rust build, or require a manual reinstall for the first Rust release.
2. **Signing**: no Authenticode or Apple Developer ID today. Needed for a trustworthy updater and to avoid SmartScreen/Gatekeeper warnings.
3. **Jar bundling**: jars now live in `bundle/mangoconfig/` (committed). Still open: keep committing vs build in CI, and the Tauri `bundle.resources` wiring so packaged apps find them.
4. **UI stack**: DECIDED, Tauri 2 + Svelte 5 + TypeScript (Vite). Performance budget: cold start under 1s to interactive, idle RAM under 120 MB (launcher window open), under 40 MB when hidden/tray while the game runs, installer under 15 MB. Rules: heavy work only in Rust (hashing, downloads, zip, NBT, JSON parsing), never in the webview; send progress events throttled to 10/s; virtualize long lists (mod lists, version pickers); no UI framework runtime beyond Svelte; images served via the custom scheme, resized in Rust; one WebGL canvas only while the skin viewer is visible, destroyed otherwise; animate only transform/opacity; destroy the webview when the window is hidden for the game session if idle RAM is above budget.
5. **Credential migration**: force re-login once vs implement an OSCrypt-compatible reader for old `encryptedCredentials`.

Detailed port spec (endpoints, layouts, launch order, IPC list): `docs/port-spec.md`.
