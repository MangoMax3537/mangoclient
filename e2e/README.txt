MangoClient end-to-end harness
==============================

What it does
------------
Starts the REAL built desktop binary (Tauri shell + WebKitGTK webview) under the
system WebKitWebDriver and checks the things a browser-based dev run cannot:

  S1 window opens, title "MangoClient", document loads, no page errors
  S2 IPC works under the reduced capability (core:event:default): app_state via
     window.__TAURI_INTERNALS__.invoke; ungranted plugin commands are refused
  S3 event subscription (plugin:event|listen) works and a core event arrives
  S4 mangoimg:// custom protocol: account head loads; URLs outside the allowed
     roots are refused
  S5 CSP: inline script, fetch() and remote <img> to the internet are blocked
  S6 drives the real UI with WebDriver clicks: onboarding (offline account, RAM),
     new Vanilla profile (reads the Mojang version manifest only), Library,
     Discover (Modrinth search), Settings; screenshots of each
  S7 single instance: a second start with the same data dir exits quickly
  S8 clean quit: window close -> exit code 0, no stray WebKit processes

S1-S8 never click Play, never start the game, never download game files.
Network used: Mojang version manifest, Modrinth search, ping of the seeded
partner servers.

Scenarios that need a prepared data dir (S9 - S13)
--------------------------------------------------
They run on COPIES of a prepared data directory (Minecraft 1.21.4 vanilla +
fabric downloaded, Java runtimes, assets, an offline account, a profile named
"Vanilla Test"). Point to it with --seed <dir> or MANGO_E2E_SEED (default: the
scratch dir used while developing). Without it they are SKIPPED.
- The copy lives in ~/.cache/mango-e2e-<scenario> (never /tmp, it is a small
  tmpfs) and is deleted at the end unless --keep.
- accounts.json of the copy is rebuilt: offline accounts + profiles only, the
  Microsoft record (encrypted credentials) and every *.bak/*.log are NOT copied.
  config.json is patched: language en, firstRunDone, telemetry OFF.
- The game is really started in S9 (a Minecraft window opens on your display
  for a few seconds, it is ended with SIGTERM). The launch resolves Java and the
  version manifest; it downloads only what a normal launch downloads and the
  copy already has everything.

  S9a  hideOnLaunch=true + keepLauncherOpen=false (config_set), game_launch.
       From OUTSIDE (xdotool + /proc): once java runs, the main X11 window and
       all WebKit child processes disappear while the mangoclient process and
       java stay alive (RSS/PSS of the main process is recorded). SIGTERM on
       java: the window and the WebKit processes come back within 10 s. Then a
       WebDriver attach is attempted (see limits below); window close -> exit 0.
  S9b  second start while the window is destroyed: window reappears, second
       process exits; closing the window during the running game does NOT quit
       the app; second start again restores it; what happens when the game then
       ends is recorded (a launcher that quits under an open window is reported
       as a bug candidate).
  S10  Ctrl+5 -> Skins page. The composited screenshot of the viewer canvas is
       decoded (lib/png.mjs) and judged: non-uniform, several significant
       colours. If WebGL is unavailable it checks the flat fallback instead.
       The texture path (fetch/createImageBitmap/crossOrigin) is spied on.
  S11  title bar: brand, back/forward, 3 window controls, 8 resize zones;
       maximize/restore via buttons and via double click (real X11 clicks),
       icon swap, geometry from xdotool, zones hidden when maximized; invokes
       window_start_drag / window_start_resize; minimize -> unmapped -> restore.
  S12  Home hero cycling: samples the layer stack for up to 75 s and needs one
       change. The window is activated first (the cycler pauses unfocused).
       No test hook exists in the production build (mockCycleMs is mock only).
  S13  screenshots of Home, Library, Skins, Settings, every instance tab, the
       icon picker and the shortcut sheet (s13-*.png in the out dir), plus
       broken-image and horizontal-overflow counters per view. LOOK at them.

Options: --seed <dir>, --only S9A,S10 (comma list; S1-S8 only start when one of
them is selected). Limits: WebKitWebDriver always spawns its own browser
process, so it cannot attach to a window the app re-created after a destroy
(S9a records that; app_state/game_running in the re-created window are then
only verifiable by hand). The old session dies with the destroyed webview.

Isolation
---------
The app runs with MANGOCLIENT_HOME set to a fresh directory under the output dir
(deleted at the end unless --keep). The real data dir is never used.
The single-instance lock is global per appId, so the harness refuses to start
when any other process named "mangoclient" is running (close the launcher first).

Prerequisites (nothing is installed by the harness)
---------------------------------------------------
- Linux with webkit2gtk and /usr/bin/WebKitWebDriver (comes with webkit2gtk)
- Node 22+ (global fetch), no npm packages
- A display (X11/XWayland or Wayland session). S8 uses xdotool to send the
  window manager's close request; without xdotool S8 is skipped.
- Fresh UI + binary:
    cd ui && npm run build
    cargo build --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol

Run
---
    node e2e/run.mjs [--binary <path>] [--port 4455] [--out <dir>] [--keep] [--native]

  --binary  default src-tauri/target/debug/mangoclient
  --out     screenshots, app.log, driver.log, exit code; default target/e2e/<timestamp>
  --keep    keep the data dir after the run
  --native  do not force GDK_BACKEND=x11 (then S8 is skipped, no X11 window)

Notes
-----
- TAURI_WEBVIEW_AUTOMATION=true is set so the app allows WebKit automation (what
  tauri-driver does). The harness forces XWayland and WEBKIT_DISABLE_DMABUF_RENDERER=1
  because the dmabuf path fails on XWayland here.
- The app is started through a tiny wrapper script so its real exit status is
  recorded (WebDriver hides it).
- Error collectors are installed right after the page loads, so errors thrown
  during the first moments of boot are only seen indirectly (S2: UI rendered).
- Still manual: native file dialogs (import/export, skin upload, image pick),
  drag-and-drop of files onto the window (files:dropped), real Microsoft login,
  the self-updater, tray behaviour, Wayland-native window controls (the harness
  forces XWayland), macOS/Windows shells, and judging visuals beyond the
  numeric checks (look at the s13-*/s10/s11 screenshots).
- Processes are killed by PID only. Exit status of the harness: 0 all executed
  scenarios passed, 1 otherwise. A failing scenario saves FAIL-<id>.png.
