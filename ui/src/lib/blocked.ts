// Pure helpers for the blocked-files experience (docs/ipc.md Part 5): summaries, state grouping, settings validation.
import type { PluralKey } from './i18n';
import type { BlockedFile, BlockedState } from './types';

/** More pages than this need an explicit confirmation before "Open all missing pages". */
export const OPEN_ALL_CONFIRM_THRESHOLD = 5;
/** Delay between two opened pages, so the browser is not flooded and the order stays visible. */
export const OPEN_ALL_DELAY_MS = 900;
export const MAX_WATCH_DIRS = 8;

export interface BlockedSummary {
  total: number;
  /** Matched on Modrinth, handled automatically. */
  matched: number;
  /** The user still has to download these. */
  pending: number;
  found: number;
  installed: number;
  skipped: number;
  /** Files the user has to provide (everything except Modrinth matches and skipped files). */
  manual: number;
  /** Of `manual`, how many are picked up already (found or installed). */
  resolved: number;
}

export function summarize(files: readonly BlockedFile[]): BlockedSummary {
  const c: Record<BlockedState, number> = { pending: 0, matched_modrinth: 0, found: 0, installed: 0, skipped: 0 };
  for (const f of files) c[f.state]++;
  const manual = c.pending + c.found + c.installed;
  return { total: files.length, matched: c.matched_modrinth, pending: c.pending, found: c.found, installed: c.installed, skipped: c.skipped, manual, resolved: c.found + c.installed };
}

/** Plan-time split: how many come from Modrinth automatically and how many the user must download. */
export function planSplit(files: readonly BlockedFile[]): { total: number; matched: number; manual: number } {
  const matched = files.filter((f) => f.state === 'matched_modrinth').length;
  return { total: files.length, matched, manual: files.length - matched };
}

/** Files the user still has to fetch (their website page is the next step). */
export const missingFiles = (files: readonly BlockedFile[]): BlockedFile[] => files.filter((f) => f.state === 'pending');

export const openAllNeedsConfirm = (n: number): boolean => n > OPEN_ALL_CONFIRM_THRESHOLD;

/** Done may stop the watcher right away only when nothing is pending; otherwise the user confirms leaving files out. */
export const canFinishSilently = (files: readonly BlockedFile[]): boolean => !files.some((f) => f.state === 'pending');

/** Sorts pending first, then found, installed, matched, skipped; stable within a group. */
const ORDER: Record<BlockedState, number> = { pending: 0, found: 1, installed: 2, matched_modrinth: 3, skipped: 4 };
export const sortBlocked = (files: readonly BlockedFile[]): BlockedFile[] =>
  files.map((f, i) => ({ f, i })).sort((a, b) => ORDER[a.f.state] - ORDER[b.f.state] || a.i - b.i).map((x) => x.f);

/** Absolute POSIX path, Windows drive path or UNC path. No NUL, no `..` segments. */
export function isAbsolutePath(raw: string): boolean {
  const s = raw.trim();
  if (!s || s.length > 1024 || s.includes('\0')) return false;
  if (s.split(/[\\/]/).includes('..')) return false;
  return /^\//.test(s) || /^[A-Za-z]:[\\/]/.test(s) || /^\\\\[^\\/]+[\\/][^\\/]+/.test(s);
}

export type WatchDirError = 'invalid' | 'duplicate' | 'limit';
const norm = (s: string) => s.trim().replace(/[\\/]+$/, '').toLowerCase();
/** Returns the error for adding `raw` to `dirs`, or null when it is acceptable. */
export function validateWatchDir(dirs: readonly string[], raw: string): WatchDirError | null {
  if (!isAbsolutePath(raw)) return 'invalid';
  if (dirs.some((d) => norm(d) === norm(raw))) return 'duplicate';
  if (dirs.length >= MAX_WATCH_DIRS) return 'limit';
  return null;
}
export const cleanDir = (raw: string): string => {
  const s = raw.trim();
  return /^[A-Za-z]:[\\/]*$/.test(s) ? `${s.slice(0, 2)}\\` : s.length > 1 ? s.replace(/[\\/]+$/, '') || '/' : s;
};

/** Opens the pages one after another with a pause in between. `isCancelled` is checked before every page. Resolves how many were opened. */
export async function openPagesSequentially(
  urls: readonly string[], open: (url: string) => unknown, delayMs = OPEN_ALL_DELAY_MS, isCancelled: () => boolean = () => false,
): Promise<number> {
  let n = 0;
  for (const u of urls) {
    if (isCancelled()) break;
    if (n > 0) await new Promise((r) => setTimeout(r, delayMs));
    if (isCancelled()) break;
    await open(u);
    n++;
  }
  return n;
}

/** "N files are blocked by CurseForge: X will come from Modrinth automatically, Y you need to download yourself" (parts with 0 are left out). */
export function planSummaryText(tp: (key: PluralKey, n: number) => string, files: readonly BlockedFile[]): string {
  const { total, matched, manual } = planSplit(files);
  const parts: string[] = [];
  if (matched) parts.push(tp('blocked.plan.matched', matched));
  if (manual) parts.push(tp('blocked.plan.manual', manual));
  return `${tp('blocked.plan.head', total)} ${parts.join(', ')}`;
}
