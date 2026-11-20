# IPC contract: UI <-> Rust

Single source of truth for the first UI slice. The Rust service layer (`app/crates/app`, package `mango-app`, no Tauri dependency) implements it as plain async functions; `app/src-tauri` exposes each as a Tauri command; `app/ui` calls them through `src/lib/api.ts` (`Api` interface with a `tauriApi` and a `mockApi`). JSON is camelCase. Command names are snake_case.

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
