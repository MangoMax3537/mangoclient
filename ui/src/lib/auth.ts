import { toAppError } from './api';

/** What went wrong while signing in, as far as the user can act on it. */
export type LoginErrorKind =
  | 'no_xbox_profile' | 'country' | 'adult_verification' | 'child_account' | 'not_owned' | 'no_profile' | 'offline' | 'generic';
export type HelpSite = 'xbox' | 'microsoft' | 'minecraft';
export interface LoginErrorInfo { kind: LoginErrorKind; help: { url: string; site: HelpSite } | null }

/** Stable codes from crates/auth/src/error.rs (AuthError::code) and net (NetError::code). */
const KIND_BY_CODE: Record<string, LoginErrorKind> = {
  'auth.xbox.no_profile': 'no_xbox_profile',
  'auth.xbox.country': 'country',
  'auth.xbox.adult_verification': 'adult_verification',
  'auth.xbox.child_account': 'child_account',
  'auth.not_owned': 'not_owned',
  'auth.no_profile': 'no_profile',
  'auth.request': 'offline', 'auth.timeout': 'offline', 'net.request': 'offline', 'net.timeout': 'offline', offline: 'offline',
};

/** Only official pages. Every URL here must also pass `isAllowedExternalUrl`. */
const HELP: Partial<Record<LoginErrorKind, { url: string; site: HelpSite }>> = {
  no_xbox_profile: { url: 'https://account.xbox.com/', site: 'xbox' },
  country: { url: 'https://www.xbox.com/regions', site: 'xbox' },
  adult_verification: { url: 'https://support.xbox.com/', site: 'xbox' },
  child_account: { url: 'https://account.microsoft.com/family/about', site: 'microsoft' },
  not_owned: { url: 'https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc', site: 'minecraft' },
  no_profile: { url: 'https://www.minecraft.net/msaprofile/mygames/editprofile', site: 'minecraft' },
};

export function classifyLoginError(e: unknown): LoginErrorInfo {
  const kind = KIND_BY_CODE[toAppError(e).code] ?? 'generic';
  return { kind, help: HELP[kind] ?? null };
}

const EXTERNAL_HOSTS = ['microsoft.com', 'xbox.com', 'minecraft.net'];
/** https only, no credentials in the URL, host is (a subdomain of) an official Microsoft/Xbox/Minecraft domain, or a CurseForge Minecraft project page. */
export function isAllowedExternalUrl(raw: string): boolean {
  let u: URL;
  try { u = new URL(raw); } catch { return false; }
  if (u.protocol !== 'https:' || u.username || u.password || u.port) return false;
  const h = u.hostname.toLowerCase();
  // blocked CurseForge files: the website page of the project (docs/ipc.md Part 4), nothing else on that host
  if (h === 'curseforge.com' || h === 'www.curseforge.com') return u.pathname === '/minecraft' || u.pathname.startsWith('/minecraft/');
  return EXTERNAL_HOSTS.some((d) => h === d || h.endsWith(`.${d}`));
}

export type OsKind = 'linux' | 'mac' | 'win';
export function detectOs(ua: string = typeof navigator !== 'undefined' ? navigator.userAgent : ''): OsKind {
  if (/Windows/i.test(ua)) return 'win';
  if (/Mac/i.test(ua)) return 'mac';
  return 'linux';
}
