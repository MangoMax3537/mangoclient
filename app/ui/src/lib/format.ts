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
