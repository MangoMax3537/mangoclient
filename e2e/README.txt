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

It never clicks Play, never starts the game, never downloads game files.
Network used: Mojang version manifest, Modrinth search, ping of the seeded
partner servers.

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
- Processes are killed by PID only. Exit status of the harness: 0 all executed
  scenarios passed, 1 otherwise. A failing scenario saves FAIL-<id>.png.
