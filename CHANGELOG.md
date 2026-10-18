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