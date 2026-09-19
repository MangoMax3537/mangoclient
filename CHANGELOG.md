### MangoClient 1.3.2

- Added macOS builds. MangoClient now ships as a `.dmg` (and an update `.zip`) for both Apple Silicon (arm64) and Intel (x64) Macs, built on a macOS CI runner and published alongside the Windows and Linux artifacts. Game data on macOS lives in `~/Library/Application Support/mangoclient`. The builds are unsigned, so the first launch needs a right-click → **Open**.

### MangoClient 1.3.1 HOTFIX

Fixed the issue with Servers not appearing after Instance Restart / Client Update (yes thats it)