<p align="center">
  <img src="assets/brand/icon.png" width="128" height="128" alt="MangoClient logo">
</p>

<h1 align="center">MangoClient</h1>

<p align="center">
  A fast Minecraft launcher for mods, isolated profiles, and multiple accounts.
</p>

<p align="center">
  <a href="https://mangoclient.com/">Website</a> ·
  <a href="https://github.com/MangoMax3537/mangoclient/releases/latest">Download</a> ·
  <a href="CHANGELOG.md">Changelog</a>
</p>

MangoClient brings Modrinth discovery, separate Minecraft setups, and account switching into one desktop app. It is free, has no ads or bundled offers, and is available for Windows, macOS, and Linux.

## Features

- **Profiles that stay separate** — each profile has its own game version, loader, mods, worlds, settings, memory, Java, window size, icon, background, group and tags, and playtime. Right-click an instance for its menu; a click opens it in the Library (on Home a click selects it, a double click opens it).
- **Modrinth built in** — mods, shaders, resource packs and modpacks. Installing asks which instances to use (instances that already have the project or cannot take it are dimmed with the reason) and shows required and optional dependencies before anything is installed. Mods, shaders and resource packs have their own tabs per instance.
- **Import and export** — import Modrinth `.mrpack` files and Prism/MultiMC instances, export an instance as `.mrpack`.
- **Safety nets** — world backups and instance snapshots (automatic before updates and removals), crash analysis in plain language, and a diagnostics export that scrubs paths and tokens.
- **Microsoft and offline accounts** — switch accounts without restarting. Sign-in tokens are encrypted with a key that only exists on your device (kept in the system keystore). A skins page offers a 3D preview, skin upload, capes and a skin library.
- **Automatic Java setup** — a compatible local Java or the matching Eclipse Temurin runtime, with a per-instance override.
- **Sensible memory** — 4 GB on 16 GB machines, 5 GB on 32 GB, 6 GB on 64 GB and more, adjustable per instance; packs can recommend their own value.
- **Performance setup** — off by default; nothing is installed behind your back. Turn it on (`performanceMods`) and the launcher keeps Sodium, Lithium and FerriteCore in compatible Fabric and Quilt instances, respecting your own mods. Anything installed is listed in the instance's mod list and can be removed there.
- **MangoConfig included** — an optional in-game HUD and settings layer with zoom, ping and armor displays, fullbright, and Discord Rich Presence.
- **Looks like a game launcher** — a Minecraft-style angular design, backgrounds per Minecraft version (or your own), instance icons from real blocks read from your own game files, skeleton loading, smooth transitions, keyboard shortcuts (press `?`), a custom title bar and a settings search that understands aliases.
- **Useful tools** — live server status with icons and quick join, screenshots, launcher/game/crash logs, mclo.gs sharing, storage controls, signed automatic updates.
- **English and German UI.**

CurseForge is not supported in 2.0 (Modrinth only).

## Download

Get the newest stable build from the [GitHub Releases page](https://github.com/MangoMax3537/mangoclient/releases/latest) or visit [mangoclient.com](https://mangoclient.com/).

| Platform | Build | Requirements |
| --- | --- | --- |
| Windows | `.exe` installer | Windows 10 or 11, 64-bit |
| macOS | `.dmg` (Apple Silicon and Intel) | macOS 11 Big Sur or newer |
| Linux | AppImage or `.deb` | x86_64, glibc 2.35+ (Ubuntu 22.04 or newer) |

On Linux, make the AppImage executable before starting it:

```bash
chmod +x MangoClient-*.AppImage
./MangoClient-*.AppImage
```

On macOS, download the `.dmg` for your chip (`arm64` for Apple Silicon, `x64` for Intel), open it, and drag MangoClient to Applications. The builds are not signed with an Apple Developer certificate, so the first launch is blocked by Gatekeeper — right-click the app and choose **Open**, then confirm once. Later launches open normally.

MangoClient checks GitHub Releases for updates and can install a new version from inside the app.

## MangoConfig compatibility

MangoConfig `1.10.1` is bundled for Fabric and Quilt profiles running these Minecraft versions:

`1.20.5`, `1.20.6`, and every supported release from `1.21` through `1.21.11`.

MangoConfig is managed by the launcher and can be disabled globally or per profile. It is kept out of the regular mod list because it ships as part of MangoClient rather than as a separately installed Modrinth project.

## Data and privacy

Game data is stored outside the application installation so launcher updates do not replace profiles or worlds:

| Platform | Data directory |
| --- | --- |
| Windows | `%APPDATA%\.mangoclient` |
| macOS | `~/Library/Application Support/mangoclient` |
| Linux | `~/.mangoclient` |

Microsoft sign-in tokens are encrypted with a random key that exists only on your device and lives in the operating system's keystore, so a copied profile folder cannot be used on another machine. If no keystore is available, the login is kept in memory only for that session. Nothing from Mojang's game files is shipped with MangoClient: block icons and panoramas are read at runtime from your own Minecraft installation. Downloads are checked against their published sizes and checksums, and archive extraction is restricted to the selected profile.

Optional telemetry powers the public install and online counters. When enabled, it sends only a random installation ID, the MangoClient version, and the operating-system platform. It does not send a Minecraft account, player name, or hardware information, and it can be disabled in Settings.

While you are in game, MangoConfig sends a presence heartbeat so other MangoClient players see your mango badge. It contains your Minecraft UUID and name, the Minecraft and MangoConfig versions, and the address of the server you are on (or "singleplayer"). Only the badge and the online count are public; the rest is visible to MangoClient staff only.

## Development

MangoClient 2.0 is a Rust rewrite (Tauri 2 shell, Rust crates for all logic, Svelte 5 frontend). It is under active development and has no release yet.

### Requirements

- Rust (stable) and Node.js 22 or newer with npm
- Linux only: `webkit2gtk-4.1` (for example `sudo pacman -S webkit2gtk-4.1`, or `libwebkit2gtk-4.1-dev` on Debian/Ubuntu)
- JDK 21 only if you rebuild the bundled MangoConfig jars

### Run and test

```bash
cargo test --workspace            # all Rust crates
cd ui && npm install && npm run dev   # UI in the browser with mock data
cd ui && npm run build && cd ..
cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol   # desktop app
cargo run -p mango-cli -- --help  # headless launcher
```

Set `MANGOCLIENT_HOME` to use a separate data directory while developing.

## Project structure

| Path | Purpose |
| --- | --- |
| `crates/` | Rust crates: core, auth, net, java, minecraft, loaders, modrinth, content, servers, mango, archive, updater, runtime, app (service layer), cli |
| `src-tauri/` | Tauri 2 desktop shell |
| `ui/` | Svelte 5 + TypeScript frontend |
| `bundle/mangoconfig/` | Prebuilt MangoConfig jars shipped with the launcher |
| `assets/brand/` | MangoClient icon |
| `mod/` | MangoConfig Fabric mod source and compatibility layers |
| `server/` | Presence and launcher statistics service (Go) |
| `tools/` | Mod build tooling |
| `docs/` | Port spec, IPC contract, design brief, ideas |

## License

MangoClient is licensed under the MIT License.

MangoClient is not affiliated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Studios.
