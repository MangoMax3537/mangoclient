# Changelog

## MangoClient 1.3.0

This is MangoClient's largest launcher update so far. The frontend has been rebuilt around a faster, denser Minecraft-client experience while keeping existing profiles, accounts, worlds and launcher behaviour intact.

### Launcher UI

- Completely redesigned the Electron renderer with a near-black gaming-client canvas, Mango orange interaction states, a slim navigation rail, an integrated top bar and view-specific layouts instead of a generic dashboard/card treatment.
- Reworked Home around the selected instance: its cover, Minecraft version, loader, memory, mod count and playtime now form the main launch composition, with a split Play/profile selector and contextual accounts and partnered-server area.
- Added dedicated instance workspaces for Overview, Content, Screenshots, Logs and per-instance Settings. Launch progress is visible directly in the opened instance instead of only on Home.
- Renamed the launcher-wide content browser to **Discover** and improved its search layout, equal-height results, filters and installed-content handling.
- Reworked Profiles into playable instance slots, Statistics into a readable time chart with values and axes, and Settings into full-width native launcher sections.
- Fixed navigation selection conflicts, clipped instance names, overflowing statistics controls, search-icon alignment and several narrow-window layout issues.
- Added the official Steve texture as the offline/no-network fallback instead of a generated placeholder model.

### Content and instances

- Discover can now search and install Modrinth mods, modpacks, resource packs and shaders directly into the selected instance's Minecraft-compatible folders.
- Added separate Mods, Resource Packs and Shaders shelves to each instance, including install, update, enable/disable, remove and open-folder actions.
- Required Modrinth dependencies are installed with their exact compatible versions. Disabling a dependency also disables its consumers, enabling a mod enables its requirements, and required content cannot be removed accidentally.
- Dependency badges now show which installed mods require that library.
- Installed-content updates are checked once whenever MangoClient opens; the manual **Check for updates** action remains available.
- Modrinth requests are debounced, deduplicated, cached and preloaded for much faster switching between mods, resource packs and shaders.

### Launching and servers

- Added a persistent integrity cache for verified Minecraft libraries and assets, avoiding thousands of repeated hashes on later launches while still invalidating changed files.
- Reused installed Fabric and Quilt profiles, cached Java probes, and parallelised independent account, MangoConfig and performance-mod preparation to shorten the critical launch path.
- VincentVanilla is written to the real Minecraft multiplayer list as `★ VincentVanilla`, deduplicated and kept first without replacing the player's other servers, icons or settings.
- Partnered-server rows now expose a clear hover-to-play action and Home labels the section consistently as **Partnered Servers**.

### MangoConfig

- The in-game HUD, config screen and HUD editor now use the launcher's current palette: warm greys and the orange sampled from the logo, instead of the cool greys they were left on. Colours already saved in `config/mangoconfig.json` are carried over, so an existing HUD changes with it.
- The Ping HUD now measures the round trip to the server itself, with the same play-protocol ping request Minecraft's own debug chart uses, instead of reading whatever latency the server publishes in the tab list. Servers that never answer fall back to the tab list as before.
- Status colours in the Memory, Combo, Armor, Potions and Ping modules come from the shared palette rather than being written out in each module.

### Highlights

- Complete MangoClient UI rework
- New **Discover** browser
- Direct resource-pack and shader installation
- Dependency-aware mod management
- Per-instance content, screenshots, logs and settings
- Faster searches and launches
- Improved statistics and partnered-server integration
