# IPC contract: UI <-> Rust

Single source of truth for the first UI slice. The Rust service layer (`crates/app`, package `mango-app`, no Tauri dependency) implements it as plain async functions; `src-tauri` exposes each as a Tauri command; `ui` calls them through `src/lib/api.ts` (`Api` interface with a `tauriApi` and a `mockApi`). JSON is camelCase. Command names are snake_case.

## Errors
Every command returns `Result<T, AppError>`. `AppError = { code: string, message: string, needsRelogin: boolean }`. `code` is the stable key (from the crates' `code()`), `message` is English diagnostic text; the UI localizes by `code`.

## Types
```ts
type Loader = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge';
type LaunchState = 'preparing'|'java'|'loader'|'downloading'|'account'|'launching'|'running'|'stopped'|'crashed';
interface ProfileView { id: string; name: string; mcVersion: string; loader: Loader; loaderVersion: string|null;
  color: string|null; coverUrl: string|null; ram: number|null; javaArgs: string; mangoConfig: boolean|null;
  lastPlayed: number|null; playTimeMs: number; created: number; modCount: number; }
interface PublicAccount { uuid: string; name: string; type: 'microsoft'|'offline'; expiresAt: number|null; expired: boolean; hasCape: boolean; }
interface Config { language: 'de'|'en'; ram: number; javaPath: string; javaArgs: string; fullscreen: boolean; width: number; height: number;
  keepLauncherOpen: boolean; hideOnLaunch: boolean; showSnapshots: boolean; mangoConfig: boolean; performanceMods: boolean;
  concurrentDownloads: number; performancePreset: 'potato'|'balanced'|'quality'; selectedProfile: string|null; selectedAccount: string|null;
  firstRunDone: boolean; telemetry: boolean; sidebarOpen: boolean; }
interface AppState { version: string; config: Config; profiles: ProfileView[]; accounts: PublicAccount[]; totalRamMb: number; running: string[]; rootDir: string; }
interface VersionEntry { id: string; type: 'release'|'snapshot'|'old_beta'|'old_alpha'; releaseTime: string; }
interface VersionManifest { latest: { release: string; snapshot: string }; versions: VersionEntry[]; installed: string[]; }
interface LoaderVersion { version: string; stable: boolean; }
```

## Commands
| command | args | returns |
|---|---|---|
| `app_state` | none | `AppState` |
| `config_set` | `{ patch: Partial<Config> }` | `Config` |
| `profile_create` | `{ name, mcVersion, loader, loaderVersion? }` | `ProfileView` |
| `profile_update` | `{ id, patch: Partial<ProfileView> }` | `ProfileView` |
| `profile_delete` | `{ id }` | `void` |
| `profile_select` | `{ id }` | `Config` |
| `profile_duplicate` | `{ id }` | `ProfileView` |
| `account_add_offline` | `{ name }` | `PublicAccount` |
| `account_select` | `{ uuid }` | `Config` |
| `account_remove` | `{ uuid }` | `void` |
| `account_sign_in_microsoft` | none | `PublicAccount \| null` (not implemented yet: returns AppError code `not_implemented`) |
| `versions_manifest` | `{ force?: boolean }` | `VersionManifest` |
| `loader_versions` | `{ loader, mcVersion }` | `LoaderVersion[]` |
| `game_launch` | `{ profileId, quickJoin?: string }` | `{ pid: number, versionId: string }` (returns when the process is spawned) |
| `game_stop` | `{ profileId }` | `boolean` |
| `game_running` | none | `string[]` |
| `open_folder` | `{ target: 'root'\|'logs'\|profileId }` | `string` (path) |

## Events (Rust -> UI)
- `launch:state` `{ profileId, state: LaunchState }`
- `launch:progress` `{ profileId, phase: 'client'|'libraries'|'assets'|'java'|'modpack'|'done', done, total, label }` (throttled to <= 10/s)
- `launch:log` `{ profileId, line, level: 'info'|'warn'|'error' }`

## Notes
- Tokens never cross this boundary.
- `launch:state` ends with `stopped` or `crashed`; the UI derives `running` from the set of profiles whose last state is `running`.
- Profile images: `coverUrl` is a `mangoimg://` URL or null; the UI falls back through `fallbackImage('profile', id)` (see CLAUDE.md image-first rule).

---

# Part 2: content, discover, servers, media, logs, stats, storage, updates

Same conventions as above (camelCase JSON, snake_case commands, `Result<T, AppError>`). Images are always `mangoimg://` URLs served by the Rust side (never third-party hosts).

## Changes to existing types
- `PublicAccount` gains `headUrl: string` (`mangoimg://head/<uuid>`, rendered from the cached Mojang skin, Steve/Alex for offline; replaces the `mc-heads.net` stand-in in the UI).

## Types
```ts
type ContentType = 'mod'|'shader'|'resourcepack'|'datapack';
interface ContentItem { projectId: string; slug: string|null; title: string; type: ContentType; versionId: string|null; versionNumber: string|null;
  filename: string; enabled: boolean; local: boolean; iconUrl: string|null; requiredBy: string[]; dependency: boolean; updateAvailable: string|null; installedAt: number|null; }
interface ProjectHit { projectId: string; slug: string; title: string; author: string; description: string; type: ContentType|'modpack'; iconUrl: string|null;
  downloads: number; follows: number; categories: string[]; gameVersions: string[]; loaders: string[]; dateModified: string; installedIn: string[]; }
interface SearchParams { query: string; projectType: ContentType|'modpack'; loader?: Loader; mcVersion?: string; categories?: string[];
  sort: 'relevance'|'downloads'|'follows'|'newest'|'updated'; offset: number; limit: number; }
interface SearchResult { hits: ProjectHit[]; total: number; offset: number; }
interface ProjectDetail extends ProjectHit { body: string; gallery: string[]; license: string|null; sourceUrl: string|null; issuesUrl: string|null; }
interface ProjectVersion { versionId: string; versionNumber: string; name: string; type: 'release'|'beta'|'alpha'; gameVersions: string[]; loaders: string[]; datePublished: string; downloads: number; compatible: boolean; }
interface UpdateInfo { projectId: string; title: string; type: ContentType; from: string|null; to: string; versionId: string; compatibility: boolean; conflict: boolean; requiredBy: string[]; }
interface ServerEntry { id: string; name: string; address: string; partner: boolean; starred: boolean; iconUrl: string|null; }
interface ServerStatus { id: string; online: boolean; ping: number|null; motd: string; players: { online: number; max: number }|null; version: string|null; }
interface Screenshot { name: string; size: number; taken: number; url: string; }
interface LogEntry { id: string; name: string; size: number; modified: number; kind: 'game'|'crash'|'launcher'; }
interface StatsSummary { days: { date: string; ms: number }[]; perProfile: { profileId: string; name: string; ms: number; sessions: number }[]; totalMs: number; sessions: number; longestMs: number; }
interface StorageUsage { key: 'instances'|'versions'|'libraries'|'assets'|'runtimes'|'natives'|'cache'|'logs'; bytes: number; clearable: boolean; }
type UpdateStateName = 'idle'|'checking'|'downloading'|'ready'|'current'|'error'|'disabled';
interface UpdateStatus { state: UpdateStateName; percent?: number; version?: string; notes?: string; errorCode?: 'no-release'|'offline'|'other'; disabledReason?: 'dev'|'unsupported'; }
```

## Commands
| command | args | returns |
|---|---|---|
| `content_list` | `{ profileId }` | `ContentItem[]` (also syncs the `mods/` folder with the profile) |
| `content_toggle` | `{ profileId, projectId, enabled }` | `{ items: ContentItem[], affected: string[] }` |
| `content_uninstall` | `{ profileId, projectId }` | `{ blocked: boolean, dependents: string[], items: ContentItem[] }` |
| `content_check_updates` | `{ profileId }` | `UpdateInfo[]` |
| `content_open_folder` | `{ profileId, type: ContentType }` | `string` |
| `modrinth_search` | `{ params: SearchParams }` | `SearchResult` |
| `modrinth_project` | `{ projectId }` | `ProjectDetail` |
| `modrinth_versions` | `{ projectId, profileId? }` | `ProjectVersion[]` (filtered by profile loader and mcVersion when given; `compatible` marks matches) |
| `modrinth_install` | `{ profileId, projectId, versionId? }` | `ContentItem[]` (the profile's full list after install, incl. required dependencies) |
| `modrinth_install_modpack` | `{ projectId, versionId?, name? }` | `ProfileView` (creates a new profile) |
| `servers_list` | none | `ServerEntry[]` |
| `servers_ping_all` | none | `void` (returns immediately; results arrive as `servers:pinged`) |
| `servers_ping` | `{ address }` | `ServerStatus` |
| `screenshots_list` | `{ profileId }` | `Screenshot[]` |
| `screenshot_action` | `{ profileId, name, action: 'delete'\|'reveal'\|'copy' }` | `void` |
| `logs_list` | `{ profileId }` | `LogEntry[]` |
| `logs_read` | `{ profileId, id }` | `{ text: string, truncated: boolean }` |
| `logs_upload` | `{ profileId, id }` | `{ url: string }` (mclo.gs) |
| `logs_delete` | `{ profileId, id }` | `void` |
| `stats_summary` | `{ days: number }` | `StatsSummary` |
| `storage_usage` | none | `StorageUsage[]` |
| `storage_clear` | `{ key }` | `StorageUsage[]` (refused with code `game_running` while a game runs) |
| `update_status` | none | `UpdateStatus` |
| `update_check` | none | `UpdateStatus` |
| `update_install` | none | `boolean` |

## Events (Rust -> UI), additions
- `install:progress` `{ profileId, projectId, done, total, label }`
- `servers:pinged` `ServerStatus`
- `update:status` `UpdateStatus`
- `content:changed` `{ profileId }` (the `mods/` folder changed on disk)

---

# Part 3: Microsoft login

## Changes to existing types
`PublicAccount` gains `credentialsPersisted: boolean` (false = memory-only session because the OS keystore is unavailable; the UI shows a notice that the login will not survive a restart), `credentialsLocked: boolean` (keystore locked or envelope undecryptable; account needs unlock or re-login) and `needsRelogin: boolean` (no usable refresh token). Tokens still never cross this boundary.

## Commands
| command | args | returns |
|---|---|---|
| `account_sign_in_microsoft` | none | `PublicAccount \| null` (`null` = user cancelled the login window; replaces the `not_implemented` stub) |
| `account_refresh` | `{ uuid }` | `PublicAccount` (refreshes the access token in memory; persists the rotated refresh token through the Vault) |
| `open_external` | `{ url: string }` | `void` (opens the default browser; https only, no credentials, host must be microsoft.com, xbox.com, minecraft.net or live.com or a subdomain; otherwise AppError code `external_url_forbidden`) |

`game_launch` with a Microsoft account refreshes the token first. If that needs a new login it fails with `AppError { needsRelogin: true }`; the UI then offers `account_sign_in_microsoft` again.

## Events
- `accounts:changed` `{ uuid }` after any background refresh or credential-state change.

## Rust-side host trait (not a command)
`mango_app::host::LoginWindow` — `async fn authorize(&self, authorize_url: &str, redirect_prefix: &str) -> LoginOutcome` with `LoginOutcome = Code(String) | Cancelled | Error(String)`. src-tauri implements it with an isolated, non-persistent webview window (incognito/in-memory data store, all permissions denied, navigation allow-list limited to login.live.com / microsoftonline / live.com hosts, intercepting the redirect to `https://login.live.com/oauth20_desktop.srf`). The CLI implements it by printing the URL and reading the pasted redirect URL from stdin.

---

# Part 4: providers (Modrinth + CurseForge), install planning, modpacks

Supersedes the `modrinth_*` commands of Part 2 (the Rust side may keep them as thin deprecated aliases for one release; the UI must use the new names). `projectId` is always a string (CurseForge ids are numeric strings) and is only unique together with `provider`.

## Types
```ts
type Provider = 'modrinth' | 'curseforge';
interface ProviderStatus { provider: Provider; available: boolean; reason?: 'no_api_key'|'offline'|'disabled'; }
// ProjectHit, ProjectDetail, ProjectVersion, SearchParams (Part 2) gain `provider: Provider`.
// ContentItem and UpdateInfo gain `provider: Provider | null` (null = local jar).
type TargetState = 'available' | 'installed' | 'incompatible';
interface InstallTarget { profileId: string; state: TargetState; installedVersion: string | null;
  reason: null | 'loader' | 'mc_version' | 'content_type' | 'no_compatible_version' | 'distribution_blocked';
  detail: string | null;            // e.g. "needs Fabric, instance uses Forge" (English diagnostic; UI localizes by reason)
  requiredDeps: number; }           // how many required dependencies would be added
interface InstallPlanItem { provider: Provider; projectId: string; versionId: string; title: string; type: ContentType; iconUrl: string | null;
  role: 'requested' | 'required' | 'optional' | 'incompatible'; alreadyInstalled: boolean; installedVersion: string | null;
  distributionBlocked: boolean; sizeBytes: number | null; downloadUrl: string | null; }  // downloadUrl only set when distributionBlocked (user must fetch from the website)
interface InstallPlan { profileId: string; items: InstallPlanItem[]; totalBytes: number; conflicts: { projectId: string; title: string; with: string }[]; }
interface ModpackPlan { provider: Provider; projectId: string; versionId: string; name: string; mcVersion: string; loader: Loader; loaderVersion: string | null;
  fileCount: number; sizeBytes: number | null; iconUrl: string | null; blockedFiles: { name: string; url: string }[]; }
```
`SearchParams.provider` selects the source (the UI offers a source switch Modrinth | CurseForge; no merged result list).

## Commands
| command | args | returns |
|---|---|---|
| `providers_status` | none | `ProviderStatus[]` (CurseForge needs an API key from build-time env `CURSEFORGE_API_KEY` or the optional config field `curseforgeApiKey`; when missing `available:false, reason:'no_api_key'` and the UI hides or disables CurseForge) |
| `search_projects` | `{ params: SearchParams }` | `SearchResult` |
| `project_detail` | `{ provider, projectId }` | `ProjectDetail` |
| `project_versions` | `{ provider, projectId, profileId? }` | `ProjectVersion[]` |
| `install_targets` | `{ provider, projectId, versionId? }` | `InstallTarget[]` (one per profile; powers the instance popup: `installed` and `incompatible` entries are shown dark/disabled with the reason) |
| `install_plan` | `{ provider, projectId, versionId?, profileId }` | `InstallPlan` (resolves required AND optional dependencies, marks already-installed ones, nothing is downloaded) |
| `install_apply` | `{ provider, projectId, versionId?, profileId, include: string[] }` | `ContentItem[]` (installs the requested item + required deps + the optional deps whose `projectId` is in `include`; emits `install:progress`) |
| `modpack_plan` | `{ provider, projectId, versionId? }` | `ModpackPlan` |
| `modpack_install` | `{ provider, projectId, versionId?, name? }` | `ProfileView` (creates a new profile; progress via `install:progress` with the new `profileId`; blocked files are listed in the plan and skipped with a warning event) |

## Rules for every provider
- Dependencies are never installed silently: the UI always shows the plan first (`install_plan`) and only calls `install_apply` after confirmation.
- CurseForge files with `allowModDistribution = false` are never downloaded by the app: they appear as `distributionBlocked` with the website URL.
- Updates (`content_check_updates`) and `content_toggle`/`content_uninstall` work for items of both providers; a project is identified by `(provider, projectId)`.

## Part 4: implementation notes (clarifications, no changes to names or shapes)
- `provider` on `SearchParams` defaults to `modrinth` when omitted. A stored mod record without `provider` (written before Part 4) means Modrinth; local jars have `provider: null`. A project is identified by `(provider, projectId)`; records keep the raw provider id in `projectId` (CurseForge ids are numeric strings), so a Modrinth id that is all digits could in theory collide with a CurseForge id inside one profile.
- Config: `curseforgeApiKey` (string, 16..128 chars of `A-Za-z0-9$./_+=-`; `null` or `""` clears) is accepted by `config_set` and persisted in `config.json`, but is never returned by `app_state`/`config_set`. Precedence: config field, then build-time `CURSEFORGE_API_KEY`, then the same variable at startup. A malformed key counts as missing. Without a key every CurseForge command fails with code `curseforge.no_api_key`.
- `install_plan` order: requested, required, optional, then `incompatible` entries (only for incompatible projects that are installed; `versionId` is the installed build or `""`). `totalBytes` counts requested + required items that are neither installed nor blocked. Optional items are leaves: their own dependencies are not crawled. A dependency that cannot be resolved for the profile is skipped. A pinned `include` id that is not an optional item of the plan is ignored.
- `install_apply` refuses a blocked requested item with code `distribution_blocked`; blocked dependencies are skipped and reported as a `launch:log` event with level `warn` for that profile. Records that were downloaded stay recorded when a later download fails.
- Error codes added: `curseforge.*` (from the crate, e.g. `curseforge.unauthorized`, `curseforge.archive`), `distribution_blocked`, `no_compatible_version`, `unsupported_content`, `invalid_filename`, `unsafe_path`.
- `install_targets` reasons: mods on a vanilla instance are `loader`; if builds exist for the Minecraft version but not for the loader (Quilt also accepts Fabric builds, NeoForge also Forge builds) it is `loader`, the other way round `mc_version`, neither `no_compatible_version`. Shader packs, resource packs and data packs ignore the loader (no Iris check). Modpacks are `content_type` for every profile.
- `modpack_plan` downloads the pack file into `cache/` (needed to read the loader and file list) and `modpack_install` reuses it, then deletes it. CurseForge packs install only manifest files with `required: true`; files the author blocks are listed in `blockedFiles` and skipped with a `launch:log` warn event for the new profile. Pack jars become ordinary local content records.
- `open_external` additionally accepts `https://(www.)curseforge.com/minecraft...` so the UI can open the `downloadUrl` of a blocked item.
- CurseForge search ignores `categories` (its category ids are numeric) and filters `loader` exactly (no Fabric-on-Quilt union).
