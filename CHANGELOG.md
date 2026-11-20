### MangoClient 2.0.0

A complete rewrite: the launcher core is now Rust, the app is a Tauri shell with a new interface. It ships MangoConfig 1.10.1.

- **New design.** A new, calmer look with a gold-yellow brand, skeleton loading, a custom title bar with back and forward, smooth transitions, keyboard shortcuts (press ? for the list) and a settings search that understands aliases ("darkmode" finds Design).
- **Instances.** Per-instance RAM (sensible defaults of 4 GB on 16 GB machines, 5 GB on 32 GB, 6 GB on 64 GB and more, packs can recommend their own), Java and window size per instance, groups and tags, instance icons from your modpack, your own image or real Minecraft blocks read from your own game files, and a background per instance: a wallpaper for the Minecraft version, the version's panorama, or your own picture. Home cycles through the backgrounds.
- **Content.** Mods, shaders and resource packs have their own tabs. Installing asks which instances (instances that already have it or cannot take it are dimmed with the reason) and shows required and optional dependencies before anything is installed. Modrinth modpacks install as new instances; Modrinth `.mrpack` and Prism/MultiMC instances can be imported, instances can be exported as `.mrpack`.
- **Safety nets.** World backups and snapshots (automatic before updates and removals), restore never overwrites, crash analysis with plain-language causes and a diagnostics export that scrubs paths and tokens.
- **Accounts.** Microsoft sign-in tokens are encrypted with a key that exists only on your device (stored in the system keystore); a copied accounts file is useless elsewhere. A skins page with a 3D preview, skin upload, capes and a skin library. Offline accounts still work. Accounts from 1.3.x need one new sign-in.
- **Servers.** Real SRV lookups, server icons from the ping, quick join.
- **Updates.** Updates are verified with an Ed25519 signature bound to version, platform and file hash.
- **Not included.** CurseForge is not supported in 2.0 (Modrinth only).
- **Upgrade note.** 2.0 is a new installation: download it from the release page. Your game data, profiles and settings are picked up from the same folder; Microsoft accounts must be signed in again.

### MangoClient 1.3.4

- Ships MangoConfig 1.10.1 (below).

### MangoConfig 1.10.1

- The presence heartbeat now also sends your Minecraft name, the Minecraft and MangoConfig versions, and the address of the server you are on (or "singleplayer"). It beats immediately when you switch servers. This powers the staff dashboard; nothing new is shown publicly.

### MangoClient 1.3.3

- Ships MangoConfig 1.10.0 (below): the new MangoClient home screen in game and the Visuals page. The launcher swaps the old MangoConfig jar for the new one on the next game start.

### MangoConfig 1.10.0

- New MangoClient home screen in place of Minecraft's title screen: the pixel mango and wordmark over the blurred panorama, Singleplayer / Multiplayer / Options / MangoConfig, a bar top right with your face and name plus MangoConfig, Options, Language, Accessibility and Realms, and a "Join our Discord" card.
- New **Visuals** page in MangoConfig with nine mods:
  - **Block Overlay**: your own colour and opacity on the outline of the block you look at.
  - **Brightness**: light the world anywhere from 0% to 1000% without touching Minecraft's own brightness slider.
  - **Colour Saturation**: from black and white (0%) to vivid (200%). It applies to the world only, so the HUD keeps its colours.
  - **Custom Chat**: smooth chat (new messages slide in, with an adjustable slide time), plus chat background, line spacing and width in one place.
  - **Custom F3**: background opacity, text colour and text shadow of the debug screen.
  - **Custom Fog**: remove the world fog or set its distance, and optionally clear water and lava fog. Blindness and darkness are never touched.
  - **Hitboxes**: box colour for F3+B, and switches for the eye-height line and the look direction.
  - **Tab & Scoreboard**: see-through tab list and scoreboard, with a separate opacity for the tab list rows.
  - **UI Scale**: inventories and containers get their own GUI scale, separate from the rest of the GUI.
- MangoConfig now loads its own bundled assets without Fabric API.

### MangoClient 1.3.2

- Added macOS builds. MangoClient now ships as a `.dmg` (and an update `.zip`) for both Apple Silicon (arm64) and Intel (x64) Macs, built on a macOS CI runner and published alongside the Windows and Linux artifacts. Game data on macOS lives in `~/Library/Application Support/mangoclient`. The builds are unsigned, so the first launch needs a right-click → **Open**.

### MangoClient 1.3.1 HOTFIX

Fixed the issue with Servers not appearing after Instance Restart / Client Update (yes thats it)