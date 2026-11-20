import { fmtShortDate } from './format';
import type { Lang } from './i18n';

interface P { mcVersion: string; loader: string; lastPlayed: number | null }
type T = (k: 'info.lastPlayed' | 'info.never' | 'profile.vanilla', v?: Record<string, string | number>) => string;
const loaderName = (p: P, t: T): string => (p.loader === 'vanilla' ? t('profile.vanilla') : p.loader);

/** `1.21.11 / fabric` */
export const versionLoader = (p: P, t: T): string => `${p.mcVersion} / ${loaderName(p, t)}`;
/** `Zuletzt gespielt: 16.11.` or `Noch nie gespielt` */
export const lastPlayedText = (p: P, lang: Lang, t: T, now = Date.now()): string =>
  p.lastPlayed ? t('info.lastPlayed', { date: fmtShortDate(p.lastPlayed, lang, now) }) : t('info.never');
/** The single hero line: `1.21.11 / fabric / Zuletzt gespielt: 16.11.` */
export const heroLine = (p: P, lang: Lang, t: T, now = Date.now()): string => `${versionLoader(p, t)} / ${lastPlayedText(p, lang, t, now)}`;
