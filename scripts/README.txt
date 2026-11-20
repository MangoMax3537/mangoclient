MangoClient release tooling
===========================

Files
-----
.github/workflows/ci.yml       PR/push checks (Rust fmt+clippy+test on Linux/Windows/macOS,
                               UI check/test/build, Go vet/test, jar check, actionlint,
                               Tauri shell build on ubuntu-22.04). No secrets needed.
.github/workflows/release.yml  Tag v* (or manual run) -> installers -> signed update
                               manifest -> DRAFT GitHub release. Publishing is manual.
scripts/check-jars.mjs         Asserts the 14 MangoConfig jars in bundle/mangoconfig match
                               mod_version in mod/gradle.properties (run by CI and release).
crates/updater/examples/       keygen.rs (key pair / sign one file), make_manifest.rs
                               (sign all artifacts + write latest.json, used by release.yml).

Releasing
---------
1. Bump the version in src-tauri/tauri.conf.json (and src-tauri/Cargo.toml). The tag
   must be "v" + that version, e.g. 2.0.1 -> v2.0.1; the workflow refuses a mismatch.
2. git tag v2.0.1 && git push origin v2.0.1   (or Actions -> Release -> Run workflow).
3. Wait for the "Sign updates and draft release" job, review the draft release, then
   publish it by hand. latest.json becomes reachable under releases/latest/download/
   only after publishing (the launcher's default feed URL).

Artifacts per release
---------------------
MangoClient_<ver>_x64-setup.exe        Windows NSIS, per-user, silent capable (/S).
MangoClient_<ver>_aarch64.dmg / _x64.dmg
MangoClient_<ver>_aarch64.app.zip / _x64.app.zip   (update payload for macOS)
MangoClient_<ver>_amd64.AppImage       (update payload for Linux)
MangoClient_<ver>_amd64.deb            (no self-update; apt/dpkg users reinstall)
latest.json, SHA256SUMS.txt
latest.json keys: windows-x86_64, darwin-aarch64, darwin-x86_64, linux-x86_64-appimage.

Secrets (GitHub repository secrets; every one is optional)
----------------------------------------------------------
UPDATE_PRIVATE_KEY        Base64 Ed25519 seed (32 bytes). Without it no latest.json is
                          produced and in-app updates stay off. Never printed, masked.
CURSEFORGE_API_KEY        Compiled into the build (option_env!). Without it CurseForge
                          browsing is disabled in that build.
WINDOWS_CERTIFICATE       Base64 of the Authenticode .pfx.        } both needed, else
WINDOWS_CERTIFICATE_PASSWORD                                      } Windows is unsigned
APPLE_CERTIFICATE         Base64 of the Developer ID Application .p12.  } all three needed
APPLE_CERTIFICATE_PASSWORD                                              } to sign the
APPLE_SIGNING_IDENTITY    e.g. "Developer ID Application: Name (TEAMID)" } macOS builds
APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID
                          Notarization (APPLE_PASSWORD is an app-specific password).
                          Only used when the three signing secrets above also exist.
GITHUB_TOKEN is provided automatically (contents: write only in the release job).

Update signing key
------------------
1. cargo run -p mango-updater --example keygen
   Prints "public (compile in)" and "private (CI secret)". Run it on Linux/macOS (it
   reads /dev/urandom).
2. Store the private line as secret UPDATE_PRIVATE_KEY. Do not commit or paste it
   anywhere else. Losing it means shipping a new public key via a manual reinstall.
3. Paste the public line into UPDATE_PUBLIC_KEY_B64 in crates/updater/src/verify.rs
   (currently the placeholder REPLACE_WITH_BASE64_ED25519_PUBLIC_KEY, which makes every
   update fail closed). Commit that, then tag the release.
Local dry run of the manifest step:
   MANGO_UPDATE_PRIVATE_KEY=<seed> cargo run -p mango-updater --example make_manifest -- \
       v2.0.1 https://github.com/<owner>/<repo>/releases/download/v2.0.1 <dir-with-artifacts>

How the installers apply an update (crates/updater/src/platform_install.rs)
--------------------------------------------------------------------------
Linux AppImage  Only when $APPIMAGE is set. Copies the verified file to
                .<name>.new beside the image (0755), hard-links the current image to
                <name>.old, renames the new one over it, runs it. Refuses symlinks,
                files outside the updater download dir and any file whose SHA-256
                changed after verification. Non-AppImage installs never self-update.
Windows         Runs the verified setup with "/S /UPDATE" (silent, per-user; Tauri's
                NSIS template relaunches the app), then quits.
macOS           Unpacks the .app zip with the safe extractor beside the bundle, swaps
                it in with a hidden .<name>.old backup (rolled back on failure), opens it.
Wiring status: src-tauri/src/platform_updater.rs builds the installer, but
mango_app::build_updater (crates/app/src/service.rs) still passes NoopInstaller. It
needs to accept an Installer (e.g. a ServiceOptions field) and the shell must pass
platform_updater::build(...) before updates actually install.

Linux baseline
--------------
Tauri 2 needs webkit2gtk-4.1, which first ships in Ubuntu 22.04 / Debian 12 (glibc
2.35 / 2.36). The Linux job therefore builds on ubuntu-22.04 and the AppImage will not
run on a glibc 2.31 system (Ubuntu 20.04). CLAUDE.md asks for glibc 2.31+; that target
needs a custom build container with a backported webkit2gtk-4.1 and is NOT met today.

Existing Electron users (v1.3.4 -> Rust build). OPEN DECISION, not decided here
-------------------------------------------------------------------------------
The Electron updater reads latest.yml / latest-mac.yml / latest-linux.yml from the
GitHub release, downloads the file named there and verifies its sha512. The new
updater reads latest.json with Ed25519 signatures; the old one cannot read it.

Option A: publish a final Electron-compatible release (latest*.yml) whose entries
point at the new installer(s).
  + Existing users upgrade in place with no action (appId dev.mangomax.mangoclient).
  - Electron verifies only a hash from the same release feed, so the first Rust build
    is trusted on TLS + GitHub alone (no Ed25519 step) and is unsigned on Windows/macOS
    unless certificates exist by then.
  - The Windows NSIS format differs (electron-builder vs Tauri): install location and
    uninstall entry may differ, risking a second install side by side; needs testing.
  - macOS Electron updates need a zip in the expected shape; Linux needs an AppImage.
  - Two feed formats to keep alive for that one release; mistakes strand users.
Option B: manual reinstall for the first Rust release (announce on the site).
  + Clean trust root: users get the new public key from the download page.
  + No compatibility shims, simple release pipeline.
  - Every user must act; some will stay on 1.3.4 indefinitely.
  - Without signing certificates the manual download triggers the warnings below.
Data (accounts, profiles, settings) migrates either way, see CLAUDE.md "Credentials":
old safeStorage blobs force one re-login unless an OSCrypt reader is written.

Unsigned binary warnings (until certificates exist)
---------------------------------------------------
Windows  SmartScreen: "Windows protected your PC / unknown publisher". Users click
         "More info" -> "Run anyway". Reputation builds slowly with an OV cert and
         immediately only with EV. Silent /S updates started by the launcher itself do not
         show the prompt for files the app downloaded, but a fresh download does.
macOS    Gatekeeper: "cannot be opened because the developer cannot be verified" (or
         "damaged"). Users right-click -> Open, or System Settings -> Privacy &
         Security -> Open Anyway. The in-app update path does not set the quarantine
         flag, so updates themselves run unprompted. Notarization removes the warning.
Linux    No warning. chmod +x the AppImage if the file manager did not do it.
The Ed25519 update signature protects updates regardless of OS code signing; it does
not replace Authenticode / Developer ID for the first download.
