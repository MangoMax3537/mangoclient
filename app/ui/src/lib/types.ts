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
}
export interface Config {
  language: 'de' | 'en'; ram: number; javaPath: string; javaArgs: string; fullscreen: boolean; width: number; height: number;
  keepLauncherOpen: boolean; hideOnLaunch: boolean; showSnapshots: boolean; mangoConfig: boolean; performanceMods: boolean;
  concurrentDownloads: number; performancePreset: 'potato' | 'balanced' | 'quality'; selectedProfile: string | null;
  selectedAccount: string | null; firstRunDone: boolean; telemetry: boolean; sidebarOpen: boolean;
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
export interface ApiEvents {
  'launch:state': LaunchStateEvent;
  'launch:progress': LaunchProgressEvent;
  'launch:log': LaunchLogEvent;
}
