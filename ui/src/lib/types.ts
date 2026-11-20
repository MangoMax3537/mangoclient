export type Loader = 'vanilla' | 'fabric' | 'quilt' | 'forge' | 'neoforge';
export type LaunchState =
  | 'preparing' | 'java' | 'loader' | 'downloading' | 'account' | 'launching'
  | 'running' | 'stopped' | 'crashed';
export type ProgressPhase = 'client' | 'libraries' | 'assets' | 'java' | 'modpack' | 'done';
export type LogLevel = 'info' | 'warn' | 'error';

export interface ProfileView {
  id: string; name: string; mcVersion: string; loader: Loader; loaderVersion: string | null;
  color: string | null; coverUrl: string | null; ram: number | null; javaArgs: string;
  mangoConfig: boolean | null; lastPlayed: number | null; playTimeMs: number; created: number; modCount: number;
}
export interface PublicAccount {
  uuid: string; name: string; type: 'microsoft' | 'offline'; expiresAt: number | null; expired: boolean; hasCape: boolean;
  /** `mangoimg://head/<uuid>`: cached Mojang skin head, Steve/Alex for offline. */
  headUrl: string;
  /** false = memory-only session because no OS keystore is available (login is lost on restart). */
  credentialsPersisted: boolean;
  /** Keystore locked or envelope undecryptable; needs unlock or a new login. */
  credentialsLocked: boolean;
  /** No usable refresh token: the user must sign in again. */
  needsRelogin: boolean;
}
export interface Config {
  language: 'de' | 'en'; ram: number; javaPath: string; javaArgs: string; fullscreen: boolean; width: number; height: number;
  keepLauncherOpen: boolean; hideOnLaunch: boolean; showSnapshots: boolean; mangoConfig: boolean; performanceMods: boolean;
  concurrentDownloads: number; performancePreset: 'potato' | 'balanced' | 'quality'; selectedProfile: string | null;
  selectedAccount: string | null; firstRunDone: boolean; telemetry: boolean; sidebarOpen: boolean;
  /** Write-only (config_set): the backend never returns the key. `null` or `""` clears it. */
  curseforgeApiKey?: string | null;
}
export interface AppState {
  version: string; config: Config; profiles: ProfileView[]; accounts: PublicAccount[];
  totalRamMb: number; running: string[]; rootDir: string;
}
export interface VersionEntry { id: string; type: 'release' | 'snapshot' | 'old_beta' | 'old_alpha'; releaseTime: string }
export interface VersionManifest { latest: { release: string; snapshot: string }; versions: VersionEntry[]; installed: string[] }
export interface LoaderVersion { version: string; stable: boolean }
export interface AppError { code: string; message: string; needsRelogin: boolean }

export interface LaunchStateEvent { profileId: string; state: LaunchState }
export interface LaunchProgressEvent { profileId: string; phase: ProgressPhase; done: number; total: number; label: string }
export interface LaunchLogEvent { profileId: string; line: string; level: LogLevel }


/* ---- Part 2 (docs/ipc.md): content, discover, servers, media, logs, stats, storage, updates ---- */
export type ContentType = 'mod' | 'shader' | 'resourcepack' | 'datapack';
export type ProjectType = ContentType | 'modpack';
/** Part 4: content sources. A project is identified by (provider, projectId). */
export type Provider = 'modrinth' | 'curseforge';
export interface ProviderStatus { provider: Provider; available: boolean; reason?: 'no_api_key' | 'offline' | 'disabled' }
export interface ContentItem {
  /** null = local jar (no provider). */
  provider: Provider | null;
  projectId: string; slug: string | null; title: string; type: ContentType; versionId: string | null; versionNumber: string | null;
  filename: string; enabled: boolean; local: boolean; iconUrl: string | null; requiredBy: string[]; dependency: boolean;
  updateAvailable: string | null; installedAt: number | null;
}
export interface ProjectHit {
  provider: Provider;
  projectId: string; slug: string; title: string; author: string; description: string; type: ProjectType; iconUrl: string | null;
  downloads: number; follows: number; categories: string[]; gameVersions: string[]; loaders: string[]; dateModified: string; installedIn: string[];
}
export type SearchSort = 'relevance' | 'downloads' | 'follows' | 'newest' | 'updated';
export interface SearchParams {
  provider: Provider;
  query: string; projectType: ProjectType; loader?: Loader; mcVersion?: string; categories?: string[];
  sort: SearchSort; offset: number; limit: number;
}
export interface SearchResult { hits: ProjectHit[]; total: number; offset: number }
export interface ProjectDetail extends ProjectHit {
  body: string; gallery: string[]; license: string | null; sourceUrl: string | null; issuesUrl: string | null;
}
export interface ProjectVersion {
  provider: Provider;
  versionId: string; versionNumber: string; name: string; type: 'release' | 'beta' | 'alpha'; gameVersions: string[]; loaders: string[];
  datePublished: string; downloads: number; compatible: boolean;
}
export interface UpdateInfo {
  provider: Provider | null;
  projectId: string; title: string; type: ContentType; from: string | null; to: string; versionId: string;
  compatibility: boolean; conflict: boolean; requiredBy: string[];
}

/* ---- Part 4: install planning and modpacks ---- */
export type TargetState = 'available' | 'installed' | 'incompatible';
export type TargetReason = 'loader' | 'mc_version' | 'content_type' | 'no_compatible_version' | 'distribution_blocked';
export interface InstallTarget {
  profileId: string; state: TargetState; installedVersion: string | null; reason: TargetReason | null;
  /** English diagnostic; the UI localizes by `reason`. */
  detail: string | null;
  /** How many required dependencies would be added. */
  requiredDeps: number;
}
export type PlanRole = 'requested' | 'required' | 'optional' | 'incompatible';
export interface InstallPlanItem {
  provider: Provider; projectId: string; versionId: string; title: string; type: ContentType; iconUrl: string | null;
  role: PlanRole; alreadyInstalled: boolean; installedVersion: string | null; distributionBlocked: boolean;
  sizeBytes: number | null;
  /** Only set when distributionBlocked: the user must fetch the file from the website. */
  downloadUrl: string | null;
}
export interface InstallConflict { projectId: string; title: string; with: string }
export interface InstallPlan { profileId: string; items: InstallPlanItem[]; totalBytes: number; conflicts: InstallConflict[] }
export interface ModpackPlan {
  provider: Provider; projectId: string; versionId: string; name: string; mcVersion: string; loader: Loader; loaderVersion: string | null;
  fileCount: number; sizeBytes: number | null; iconUrl: string | null; blockedFiles: { name: string; url: string }[];
}

export interface ServerEntry { id: string; name: string; address: string; partner: boolean; starred: boolean; iconUrl: string | null }
export interface ServerStatus {
  id: string; online: boolean; ping: number | null; motd: string; players: { online: number; max: number } | null; version: string | null;
}
export interface Screenshot { name: string; size: number; taken: number; url: string }
export interface LogEntry { id: string; name: string; size: number; modified: number; kind: 'game' | 'crash' | 'launcher' }
export interface StatsSummary {
  days: { date: string; ms: number }[];
  perProfile: { profileId: string; name: string; ms: number; sessions: number }[];
  totalMs: number; sessions: number; longestMs: number;
}
export type StorageKey = 'instances' | 'versions' | 'libraries' | 'assets' | 'runtimes' | 'natives' | 'cache' | 'logs';
export interface StorageUsage { key: StorageKey; bytes: number; clearable: boolean }
export type UpdateStateName = 'idle' | 'checking' | 'downloading' | 'ready' | 'current' | 'error' | 'disabled';
export interface UpdateStatus {
  state: UpdateStateName; percent?: number; version?: string; notes?: string;
  errorCode?: 'no-release' | 'offline' | 'other'; disabledReason?: 'dev' | 'unsupported';
}

export interface InstallProgressEvent { profileId: string; projectId: string; done: number; total: number; label: string }
export interface ApiEvents {
  'launch:state': LaunchStateEvent;
  'launch:progress': LaunchProgressEvent;
  'launch:log': LaunchLogEvent;
  'install:progress': InstallProgressEvent;
  'servers:pinged': ServerStatus;
  'update:status': UpdateStatus;
  'content:changed': { profileId: string };
  'accounts:changed': { uuid: string };
}
