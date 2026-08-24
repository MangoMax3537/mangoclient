# Changelog

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
