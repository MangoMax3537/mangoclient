import type { Lang } from './i18n';

export function fmtDuration(ms: number): string {
  const totalMin = Math.floor(ms / 60000);
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  if (h === 0) return `${m} min`;
  return `${h} h ${String(m).padStart(2, '0')} min`;
}

export function fmtRelative(ts: number | null, lang: Lang, now = Date.now()): string | null {
  if (ts == null) return null;
  const diff = ts - now;
  const rtf = new Intl.RelativeTimeFormat(lang, { numeric: 'auto' });
  const abs = Math.abs(diff);
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ['day', 86400000], ['hour', 3600000], ['minute', 60000],
  ];
  for (const [u, ms] of units) if (abs >= ms) return rtf.format(Math.round(diff / ms), u);
  return rtf.format(0, 'second');
}

export const fmtRam = (mb: number): string => (mb >= 1024 ? `${+(mb / 1024).toFixed(1)} GB` : `${mb} MB`);

export function fmtBytes(n: number, lang: Lang = 'en'): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  const f = new Intl.NumberFormat(lang, { maximumFractionDigits: v >= 100 || i === 0 ? 0 : 1 });
  return `${f.format(v)} ${units[i]}`;
}

export const fmtCompact = (n: number, lang: Lang): string =>
  new Intl.NumberFormat(lang, { notation: 'compact', maximumFractionDigits: 1 }).format(n);

export const fmtDate = (ts: number | string, lang: Lang): string =>
  new Intl.DateTimeFormat(lang, { dateStyle: 'medium' }).format(typeof ts === 'string' ? new Date(ts) : ts);

export const fmtDateTime = (ts: number, lang: Lang): string =>
  new Intl.DateTimeFormat(lang, { dateStyle: 'medium', timeStyle: 'short' }).format(ts);

/** Strips Minecraft section-sign colour codes from a MOTD. */
export const stripMotd = (s: string): string => s.replace(/§[0-9a-fk-or]/gi, '').trim();

/** Short day + month for "last played": de `16.11.`, en `Nov 16`; the year is added only when it is not the current one (`16.11.2025`, `Nov 16, 2025`). */
export function fmtShortDate(ts: number, lang: Lang, now = Date.now()): string {
  const d = new Date(ts);
  const withYear = d.getFullYear() !== new Date(now).getFullYear();
  if (lang === 'de') {
    const base = `${String(d.getDate()).padStart(2, '0')}.${String(d.getMonth() + 1).padStart(2, '0')}.`;
    return withYear ? `${base}${d.getFullYear()}` : base;
  }
  return new Intl.DateTimeFormat('en', withYear ? { month: 'short', day: 'numeric', year: 'numeric' } : { month: 'short', day: 'numeric' }).format(d);
}
