<p align="center">
  <img src="src/assets/icon.png" width="128" height="128" alt="MangoClient logo">
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

MangoClient brings Modrinth discovery, separate Minecraft setups, and account switching into one desktop app. It is free, has no ads or bundled offers, and is available for Windows and Linux.

## Features

- **Profiles that stay separate** — each profile has its own game version, loader, mods, worlds, settings, memory limit, cover image, and playtime.
- **Built-in Modrinth browser** — find and install mods, modpacks, resource packs, and shaders without leaving the launcher. Required dependencies are resolved automatically.
- **Manual mod support** — drop `.jar` files into a profile's `mods` folder and MangoClient detects them. Individual mods can be enabled or disabled without deleting them.
- **Multiple loaders** — create Vanilla, Fabric, Quilt, Forge, and NeoForge profiles and choose a loader version per profile.
- **Microsoft and offline accounts** — keep multiple Microsoft accounts signed in and switch between them without restarting the launcher. Offline accounts are available for single-player and offline-mode servers.
- **Automatic Java setup** — MangoClient finds a compatible local Java installation or downloads the required Eclipse Temurin runtime for the selected Minecraft version.
- **One-click performance setup** — optionally manages Sodium, Lithium, and FerriteCore for compatible Fabric and Quilt profiles while respecting manually installed or conflicting mods.
- **MangoConfig included** — an optional in-game HUD and settings layer with features such as zoom, ping and armor displays, fullbright, and Discord Rich Presence.
- **Useful tools** — live server status and quick join, screenshot management, launcher/game/crash logs, mclo.gs sharing, storage controls, and automatic launcher updates.
- **English and German UI** — switch languages from the launcher settings.

## Download

Get the newest stable build from the [GitHub Releases page](https://github.com/MangoMax3537/mangoclient/releases/latest) or visit [mangoclient.com](https://mangoclient.com/).

| Platform | Build | Requirements |
| --- | --- | --- |
| Windows | `.exe` installer | Windows 10 or 11, 64-bit |
| Linux | AppImage or `.tar.gz` | x86_64, glibc 2.31+ |

On Linux, make the AppImage executable before starting it:

```bash
chmod +x MangoClient-*.AppImage
./MangoClient-*.AppImage
```

MangoClient checks GitHub Releases for updates and can install a new version from inside the app.

## MangoConfig compatibility

MangoConfig `1.9.2` is bundled for Fabric and Quilt profiles running these Minecraft versions:

`1.20.5`, `1.20.6`, and every supported release from `1.21` through `1.21.11`.

MangoConfig is managed by the launcher and can be disabled globally or per profile. It is kept out of the regular mod list because it ships as part of MangoClient rather than as a separately installed Modrinth project.

## Data and privacy

Game data is stored outside the application installation so launcher updates do not replace profiles or worlds:

| Platform | Data directory |
| --- | --- |
| Windows | `%APPDATA%\.mangoclient` |
| Linux | `~/.mangoclient` |

Microsoft credentials use the operating system's secure storage when available. Downloads are checked against their published sizes and checksums, and archive extraction is restricted to the selected profile.

Optional telemetry powers the public install and online counters. When enabled, it sends only a random installation ID, the MangoClient version, and the operating-system platform. It does not send a Minecraft account, player name, or hardware information, and it can be disabled in Settings.

## Development

### Requirements

- Node.js 22.12 or newer
- npm
- JDK 21 for building the bundled MangoConfig jars

### Run from source

```bash
git clone https://github.com/MangoMax3537/mangoclient.git
cd mangoclient
npm install
npm start
```

Use `npm run dev` to start Electron with developer tools enabled.

### Test and build

```bash
npm test
npm run build
```

`npm run build` first compiles MangoConfig for every supported Minecraft version and then creates platform packages in `dist/`. To create only an unpacked Electron application, run `npm run pack`.

## Project structure

| Path | Purpose |
| --- | --- |
| `src/main/` | Electron main process, launcher, authentication, downloads, profiles, and updater |
| `src/renderer/` | Launcher interface, styles, icons, and translations |
| `src/main/assets/` | MangoConfig jars bundled with the launcher |
| `mod/` | MangoConfig Fabric mod source and compatibility layers |
| `server/` | Presence and launcher statistics service |
| `test/` | Node.js test suite |
| `tools/` | Release and multi-version build tooling |

## License

MangoClient is licensed under the MIT License.

MangoClient is not affiliated with Mojang Studios or Microsoft. Minecraft is a trademark of Mojang Studios.
