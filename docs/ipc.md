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
