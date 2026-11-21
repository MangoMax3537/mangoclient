import { fmtShortDate } from './format';
import type { Lang } from './i18n';

/** What an info fact is about; each kind has one icon and one accessible label (see InfoLine.svelte). */
export type FactKind = 'version' | 'loader' | 'played' | 'players' | 'ping';
export interface Fact { kind: FactKind; text: string; /** Quieter colour (secondary facts). */ dim?: boolean }

interface P { mcVersion: string; loader: string; lastPlayed: number | null }
type T = (k: 'info.lastPlayed' | 'info.never' | 'profile.vanilla', v?: Record<string, string | number>) => string;
const loaderName = (p: P, t: T): string => (p.loader === 'vanilla' ? t('profile.vanilla') : p.loader);

export const versionFact = (p: P): Fact => ({ kind: 'version', text: p.mcVersion });
export const loaderFact = (p: P, t: T): Fact => ({ kind: 'loader', text: loaderName(p, t) });
/** `Zuletzt gespielt: 16.11.` or `Noch nie gespielt` */
export const lastPlayedText = (p: P, lang: Lang, t: T, now = Date.now()): string =>
  p.lastPlayed ? t('info.lastPlayed', { date: fmtShortDate(p.lastPlayed, lang, now) }) : t('info.never');
export const playedFact = (p: P, lang: Lang, t: T, now = Date.now(), dim = false): Fact => ({ kind: 'played', text: lastPlayedText(p, lang, t, now), dim });
/** Card form: just the date (the clock icon and its label say "last played"), so the line fits a 212 px card. */
export const playedShortFact = (p: P, lang: Lang, t: T, now = Date.now()): Fact => ({ kind: 'played', text: p.lastPlayed ? fmtShortDate(p.lastPlayed, lang, now) : t('info.never'), dim: true });

/** Version and loader (card line one). */
export const versionLoaderFacts = (p: P, t: T): Fact[] => [versionFact(p), loaderFact(p, t)];
/** The hero line: version, loader, last played. No Java or RAM. */
export const heroFacts = (p: P, lang: Lang, t: T, now = Date.now()): Fact[] => [...versionLoaderFacts(p, t), playedFact(p, lang, t, now)];
