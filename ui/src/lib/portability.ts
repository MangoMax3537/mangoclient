// Pure helpers for Part 9: import / export / crash analysis / diagnostics / groups and tags. No stores, no DOM: everything testable.
import type { Key } from './i18n';
import type { CrashCause, CrashReport, ImportKind, ImportPlan, ProfileView } from './types';

/* ---- groups and tags (limits mirror crates/core: docs/ipc.md Part 9) ---- */
export const GROUP_MAX = 48;
export const TAG_MAX = 24;
export const TAGS_MAX = 12;

const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;

export type TagError = 'empty' | 'long' | 'control' | 'duplicate' | 'limit';
export type GroupError = 'long' | 'control';

/** Trimmed value, or an error code. Duplicates are compared case-insensitively against `existing` (the first spelling wins). */
export function checkTag(raw: string, existing: readonly string[]): { ok: true; tag: string } | { ok: false; error: TagError } {
  const tag = raw.trim();
  if (!tag) return { ok: false, error: 'empty' };
  if (CONTROL.test(tag)) return { ok: false, error: 'control' };
  if ([...tag].length > TAG_MAX) return { ok: false, error: 'long' };
  if (existing.some((x) => x.toLowerCase() === tag.toLowerCase())) return { ok: false, error: 'duplicate' };
  if (existing.length >= TAGS_MAX) return { ok: false, error: 'limit' };
  return { ok: true, tag };
}

/** `null` = no group (blank). */
export function checkGroup(raw: string): { ok: true; group: string | null } | { ok: false; error: GroupError } {
  const g = raw.trim();
  if (!g) return { ok: true, group: null };
  if (CONTROL.test(g)) return { ok: false, error: 'control' };
  if ([...g].length > GROUP_MAX) return { ok: false, error: 'long' };
  return { ok: true, group: g };
}

/** What the service stores for a tag list: trimmed, no control characters, no case-insensitive duplicates, at most 12 of at most 24 chars. */
export function normalizeTags(list: readonly string[]): string[] {
  const out: string[] = [];
  for (const raw of list) {
    const r = checkTag(raw, out);
    if (r.ok) out.push(r.tag);
  }
  return out;
}

export const tagErrorKey = (e: TagError): Key => `tags.err.${e}` as Key;
export const groupErrorKey = (e: GroupError): Key => `group.err.${e}` as Key;

/** Distinct existing groups (case-insensitive, first spelling), alphabetical: the suggestions of the group field. */
export function knownGroups(profiles: readonly Pick<ProfileView, 'group'>[]): string[] {
  const seen = new Map<string, string>();
  for (const p of profiles) if (p.group && !seen.has(p.group.toLowerCase())) seen.set(p.group.toLowerCase(), p.group);
  return [...seen.values()].sort((a, b) => a.localeCompare(b));
}

/** Tags in use with their counts, most used first. */
export function knownTags(profiles: readonly Pick<ProfileView, 'tags'>[]): { tag: string; count: number }[] {
  const m = new Map<string, { tag: string; count: number }>();
  for (const p of profiles) for (const t of p.tags) {
    const e = m.get(t.toLowerCase());
    if (e) e.count++; else m.set(t.toLowerCase(), { tag: t, count: 1 });
  }
  return [...m.values()].sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag));
}

type Searchable = Pick<ProfileView, 'name' | 'mcVersion' | 'loader' | 'group' | 'tags'>;

/** Search across name, version, loader, group and tags; `tag` additionally requires that tag (case-insensitive). */
export function filterProfiles<T extends Searchable>(list: readonly T[], query: string, tag: string | null = null): T[] {
  const q = query.trim().toLowerCase();
  const tg = tag?.toLowerCase() ?? null;
  return list.filter((p) => {
    if (tg && !p.tags.some((x) => x.toLowerCase() === tg)) return false;
    if (!q) return true;
    return p.name.toLowerCase().includes(q) || p.mcVersion.includes(q) || p.loader.includes(q)
      || (p.group?.toLowerCase().includes(q) ?? false) || p.tags.some((x) => x.toLowerCase().includes(q));
  });
}

export interface GroupSection<T> { /** `null` = the "Ungrouped" section. */ group: string | null; key: string; items: T[] }

/**
 * Library sections: named groups alphabetically, "Ungrouped" last. Item order inside a section is the order of `items`.
 * Without any group there is a single ungrouped section (the UI then draws no header).
 */
export function groupSections<T extends Pick<ProfileView, 'group'>>(items: readonly T[]): GroupSection<T>[] {
  const named = new Map<string, GroupSection<T>>();
  const ungrouped: T[] = [];
  for (const p of items) {
    if (!p.group) { ungrouped.push(p); continue; }
    const k = p.group.toLowerCase();
    const s = named.get(k) ?? { group: p.group, key: `g:${k}`, items: [] };
    s.items.push(p);
    named.set(k, s);
  }
  const out = [...named.values()].sort((a, b) => (a.group ?? '').localeCompare(b.group ?? ''));
  if (ungrouped.length) out.push({ group: null, key: 'ungrouped', items: ungrouped });
  return out;
}

/* ---- import ---- */
export type PlanKind = ImportKind;
export const IMPORT_EXTENSIONS = ['.mrpack', '.zip'] as const;
export const IMAGE_EXTENSIONS = ['.png', '.jpg', '.jpeg', '.webp', '.gif'] as const;

const ext = (p: string): string => { const m = /\.[A-Za-z0-9]+$/.exec(p.trim()); return m ? m[0].toLowerCase() : ''; };
export const baseName = (p: string): string => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
export const stem = (p: string): string => baseName(p).replace(/\.[^.]+$/, '');
export const isImportFile = (p: string): boolean => (IMPORT_EXTENSIONS as readonly string[]).includes(ext(p));
export const isImageFile = (p: string): boolean => (IMAGE_EXTENSIONS as readonly string[]).includes(ext(p));

/** What a drop means: the first modpack file (the rest is mentioned, never silently imported), and whether anything else was ignored. */
export function classifyDrop(paths: readonly string[]): { file: string | null; extra: number; ignored: number } {
  const packs = paths.filter(isImportFile);
  return { file: packs[0] ?? null, extra: Math.max(0, packs.length - 1), ignored: paths.length - packs.length };
}

/** Error code -> localized message key for the import dialog; unknown codes fall back to the generic mapping of `errorText`. */
const IMPORT_ERRORS: Record<string, Key> = {
  'path.not_granted': 'import.err.notGranted',
  'path.unsupported': 'import.err.unsupportedPath',
  'import.not_found': 'import.err.notFound',
  'import.not_a_file': 'import.err.notAFile',
  'import.too_large': 'import.err.tooLarge',
  'import.not_an_archive': 'import.err.notAnArchive',
  'import.unsupported_format': 'import.err.unsupportedFormat',
  'import.invalid_pack': 'import.err.invalidPack',
  'import.unsafe_archive': 'import.err.unsafeArchive',
  'import.unsupported_curseforge': 'err.import.unsupported_curseforge',
  'loaders.installer_unverified': 'import.err.installerUnverified',
  'net.host_not_allowed': 'import.err.hostNotAllowed',
};
export const importErrorKey = (code: string): Key | null => IMPORT_ERRORS[code] ?? null;
/** Codes where choosing the file again can help. */
export const importRetryable = (code: string): boolean => code === 'path.not_granted' || code === 'import.not_found';

const EXPORT_ERRORS: Record<string, Key> = {
  'path.not_granted': 'export.err.notGranted',
  'path.unsupported': 'export.err.unsupportedPath',
  'export.invalid_path': 'export.err.invalidPath',
  'export.lookup_failed': 'export.err.lookupFailed',
  'export.loader_unresolved': 'export.err.loaderUnresolved',
  'export.too_large': 'export.err.tooLarge',
  'export.failed': 'export.err.failed',
  'net.host_not_allowed': 'import.err.hostNotAllowed',
};
export const exportErrorKey = (code: string): Key | null => EXPORT_ERRORS[code] ?? null;

export const KIND_LABEL: Record<PlanKind, Key> = { mrpack: 'import.kind.mrpack', prism: 'import.kind.prism' };

/** The import dialog as a state machine (the component only renders it). */
export type ImportState =
  | { s: 'planning'; path: string }
  | { s: 'plan'; path: string; plan: ImportPlan; name: string }
  | { s: 'importing'; path: string; plan: ImportPlan; name: string }
  | { s: 'error'; path: string; code: string; during: 'plan' | 'import'; plan?: ImportPlan; name?: string }
  | { s: 'done'; profileId: string };
export type ImportEvent =
  | { e: 'planned'; plan: ImportPlan }
  | { e: 'rename'; name: string }
  | { e: 'start' }
  | { e: 'failed'; code: string }
  | { e: 'imported'; profileId: string }
  | { e: 'back' };

export const NAME_MAX = 48;
export const nameValid = (n: string): boolean => n.trim().length > 0 && [...n.trim()].length <= NAME_MAX;

export const initialImport = (path: string): ImportState => ({ s: 'planning', path });

export function importReduce(st: ImportState, ev: ImportEvent): ImportState {
  switch (ev.e) {
    case 'planned':
      return st.s === 'planning' ? { s: 'plan', path: st.path, plan: ev.plan, name: ev.plan.name } : st;
    case 'rename':
      return st.s === 'plan' ? { ...st, name: ev.name } : st;
    case 'start':
      return st.s === 'plan' && nameValid(st.name) ? { s: 'importing', path: st.path, plan: st.plan, name: st.name } : st;
    case 'failed':
      if (st.s === 'planning') return { s: 'error', path: st.path, code: ev.code, during: 'plan' };
      if (st.s === 'importing') return { s: 'error', path: st.path, code: ev.code, during: 'import', plan: st.plan, name: st.name };
      return st;
    case 'imported':
      return st.s === 'importing' ? { s: 'done', profileId: ev.profileId } : st;
    case 'back':
      // From an import error the plan is still valid: go back to it. A failed plan has nowhere to go back to.
      return st.s === 'error' && st.during === 'import' && st.plan ? { s: 'plan', path: st.path, plan: st.plan, name: st.name ?? st.plan.name } : st;
  }
}

/** The name an imported instance gets when the plan has none. */
export const importDefaultName = (plan: ImportPlan, path: string): string => plan.name.trim() || stem(path);

/* ---- export / diagnostics ---- */
const FORBIDDEN_FILE_CHARS = /[\\/:*?"<>|\u0000-\u001f]/g;
export const safeFileStem = (name: string): string => name.replace(FORBIDDEN_FILE_CHARS, '_').replace(/\s+/g, ' ').trim().replace(/^\.+/, '').slice(0, 80) || 'instance';
export const defaultExportName = (name: string): string => `${safeFileStem(name)}.mrpack`;
export const defaultDiagnosticsName = (name: string): string => `mangoclient-diagnostics-${safeFileStem(name).replace(/ /g, '-')}.zip`;

export interface ExportOptions { includeConfig: boolean; outPath: string }
/** Why the export button is dark (a reason, not a silent disabled state). `null` = ready. */
export function exportBlocker(o: ExportOptions): 'noPath' | 'badPath' | null {
  if (!o.outPath.trim()) return 'noPath';
  return isAbsolutePathLike(o.outPath) ? null : 'badPath';
}
export function isAbsolutePathLike(raw: string): boolean {
  const s = raw.trim();
  if (!s || s.length > 1024 || s.includes('\0')) return false;
  return /^\//.test(s) || /^[A-Za-z]:[\\/]/.test(s) || /^\\\\[^\\/]+[\\/][^\\/]+/.test(s);
}
export function withExtension(path: string, extension: string): string {
  return path.toLowerCase().endsWith(extension) ? path : `${path}${extension}`;
}

/* ---- crash analysis ---- */
export const CONFIDENCE_ORDER = { high: 0, medium: 1, low: 2 } as const;
/** Known rule codes (docs/ipc.md Part 9). Titles/details/suggestions are localized by code, the Rust text is the fallback. */
export const CRASH_CODES = [
  'java_version', 'jvm_heap_reserve', 'out_of_memory', 'wrong_mc_version', 'mod_dependency', 'loader_mismatch', 'duplicate_mods',
  'mixin_failure', 'gl_driver', 'shader_failure', 'corrupt_world', 'disk_full', 'java_path', 'jvm_args', 'account_session',
] as const;
export const isKnownCrashCode = (c: string): boolean => (CRASH_CODES as readonly string[]).includes(c);

/** Highest confidence first; equal confidence keeps the service's order (it ranks by likelihood). */
export const rankCauses = (c: readonly CrashCause[]): CrashCause[] =>
  c.map((x, i) => ({ x, i })).sort((a, b) => CONFIDENCE_ORDER[a.x.confidence] - CONFIDENCE_ORDER[b.x.confidence] || a.i - b.i).map((v) => v.x);

export const hasCauses = (r: CrashReport): boolean => r.causes.length > 0;

/** Plain-text report for the clipboard (English, meant for support threads; no home directory beyond what the service scrubbed). */
export function reportText(r: CrashReport, instance: string): string {
  const lines = [`MangoClient crash report: ${instance}`];
  if (r.mcVersion) lines.push(`Minecraft: ${r.mcVersion}`);
  if (r.javaVersion) lines.push(`Java: ${r.javaVersion}`);
  if (r.sourceLog) lines.push(`Log: ${r.sourceLog}`);
  lines.push('', `Summary: ${r.summary}`);
  rankCauses(r.causes).forEach((c, i) => {
    lines.push('', `${i + 1}. ${c.title} (${c.confidence})`, `   ${c.detail}`);
    for (const s of c.suggestions) lines.push(`   - ${s}`);
    if (c.relatedMods.length) lines.push(`   Mods: ${c.relatedMods.join(', ')}`);
  });
  if (r.excerpt.trim()) lines.push('', 'Excerpt:', r.excerpt.trimEnd());
  return lines.join('\n');
}

/** The Logs tab only offers "Analyze" on crash reports and the latest log. */
export const analyzable = (e: { kind: string; id: string }): boolean => e.kind === 'crash' || e.id === 'game:latest.log' || /(^|[:/])latest\.log$/.test(e.id);

export interface CauseView { title: string; detail: string; suggestions: string[]; /** The service's own (English) detail, shown as "from the log" when the UI text is localized. */ fromLog: string | null }
/** English shows the service's texts (they carry the specifics); other languages get the localized text for known codes plus the original as a quote. */
export function causeView(c: CrashCause, lang: string, tr: (key: Key) => string, has: (key: string) => boolean): CauseView {
  const base = `crash.code.${c.code}`;
  if (lang === 'en' || !has(`${base}.title`)) return { title: c.title, detail: c.detail, suggestions: c.suggestions, fromLog: null };
  const tips = [`${base}.s1`, `${base}.s2`, `${base}.s3`].filter(has).map((k) => tr(k as Key));
  const localized = tr(`${base}.detail` as Key);
  return { title: tr(`${base}.title` as Key), detail: localized, suggestions: tips.length ? tips : c.suggestions, fromLog: c.detail && c.detail !== localized ? c.detail : null };
}
