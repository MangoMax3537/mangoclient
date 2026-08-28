# Changelog

## Unreleased

### MangoConfig

- The in-game HUD, config screen and HUD editor now use the launcher's current palette: warm greys and the orange sampled from the logo, instead of the cool greys they were left on. Colours already saved in `config/mangoconfig.json` are carried over, so an existing HUD changes with it.
- The Ping HUD now measures the round trip to the server itself, with the same play-protocol ping request Minecraft's own debug chart uses, instead of reading whatever latency the server publishes in the tab list. Servers that never answer fall back to the tab list as before.
- Status colours in the Memory, Combo, Armor, Potions and Ping modules come from the shared palette rather than being written out in each module.

## MangoClient 1.2.12

### Launcher

- Manually added mods now appear immediately after opening the launcher or switching profiles, without requiring Minecraft to start first.
- Legacy inline mod icons are migrated before profile validation, preserving existing Modrinth metadata during updates.

## MangoClient 1.2.11

This update is mostly about the small things that get annoying fast: a ping display stuck at zero, zoom steps that feel too sharp, and downloads failing even though Mojang is sending the right file.

### MangoConfig

- Fixed the Ping HUD showing `0 ms` after joining a server. It now uses the server-list measurement until Minecraft receives the live tab-list ping.
- Hold the zoom key and scroll to change the zoom level without moving through the hotbar.
- Zooming now eases in and out, including when the zoom level changes with the wheel.
- The Armor HUD no longer leaves an empty grey background when `Hide empty` is enabled. Its editor preview still stays visible and draggable.
- Fullbright is now a separate saved setting, displays as `150000%`, and leaves Minecraft's normal brightness value alone.

### Launcher

- Fixed Mojang downloads failing when the CDN reports a placeholder file size of zero.
- Fixed the white squares where horizontal and vertical scrollbars meet.
- Failed downloads are cleaned up properly, and downloaded files and modpack archives get stricter size, path and checksum checks.
- MangoConfig 1.9.2 is bundled for all supported Minecraft versions from 1.20.5 through 1.21.11.
- Updated Electron and the bundled JSON library.

### Security

- Login tokens now use the operating system's secure storage where available. Existing logins migrate automatically.
- Tightened external links, window navigation, permissions, IPC calls, local image access and profile settings.
- Hardened the presence and admin services with HTTPS-only admin routes, safer cookies, request limits and bounded in-memory data.

Linux and Windows builds are available on the GitHub release page.
