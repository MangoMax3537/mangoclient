// Settings search: normalization, tokenized matching with scoring and small typo tolerance, and the keyboard step of the result list.
// Pure functions (no DOM, no Svelte), unit-tested. The index data lives in settingsIndex.ts.
import type { SettingEntry } from './settingsIndex';

/** Lowercase, ß -> ss, umlauts expanded (ä -> ae), all other diacritics dropped. "Aufräumen" -> "aufraeumen", "Café" -> "cafe". */
export function normalize(s: string): string {
  return s.toLowerCase().replace(/ß/g, 'ss').replace(/ä/g, 'ae').replace(/ö/g, 'oe').replace(/ü/g, 'ue').normalize('NFD').replace(/[̀-ͯ]/g, '');
}
/** The other spelling of an umlaut: ä -> a (people without a German keyboard type "aufraumen"). */
export function fold(s: string): string {
  return s.toLowerCase().replace(/ß/g, 'ss').replace(/[äöü]/g, (c) => ({ ä: 'a', ö: 'o', ü: 'u' })[c as 'ä'] as string).normalize('NFD').replace(/[̀-ͯ]/g, '');
}
const split = (s: string): string[] => s.split(/[^a-z0-9]+/).filter(Boolean);
/** Words of a text in both spellings (expanded and folded), deduplicated. */
export function wordsOf(s: string): string[] {
  return [...new Set([...split(normalize(s)), ...split(fold(s))])];
}

/** Optimal string alignment distance (insert, delete, substitute, swap of two neighbours each cost 1), capped: returns max+1 when larger. */
export function editDistance(a: string, b: string, max = 1): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  const d: number[][] = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) {
      const c = a[i - 1] === b[j - 1] ? 0 : 1;
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + c);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
    }
  }
  return d[a.length][b.length];
}

export type Via = 'label' | 'alias' | 'section' | 'desc';
const WEIGHT: Record<Via, number> = { label: 100, alias: 80, section: 25, desc: 20 };
/** How well one query token matches one word: exact > prefix > substring (4+ letters) > one typo (5+ letters). 0 = no match. */
export function tokenTier(token: string, word: string): number {
  if (word === token) return 1;
  if (word.startsWith(token)) return 0.9;
  if (token.length >= 4 && word.includes(token)) return 0.6; // 3-letter tokens would hit inside words (ram in parameter)
  if (token.length >= 5 && editDistance(token, word) <= 1) return 0.5;
  return 0;
}

export interface Hit {
  entry: SettingEntry;
  score: number;
  /** Where the best token matched. */
  via: Via;
  /** The alias phrase that matched (only for `via: 'alias'`), shown as "matched: darkmode". */
  alias: string | null;
}
export interface Resolver {
  label: (e: SettingEntry) => string;
  desc: (e: SettingEntry) => string;
  section: (e: SettingEntry) => string;
}

interface Field { via: Via; words: string[]; phrase: string }
function fieldsOf(e: SettingEntry, r: Resolver): Field[] {
  const out: Field[] = [
    { via: 'label', words: wordsOf(r.label(e)), phrase: r.label(e) },
    { via: 'section', words: wordsOf(r.section(e)), phrase: r.section(e) },
  ];
  const d = r.desc(e);
  if (d) out.push({ via: 'desc', words: wordsOf(d), phrase: d });
  for (const a of e.aliases) {
    // "dark mode" also matches as the one word "darkmode"
    const w = wordsOf(a);
    const joined = w.length > 1 ? [w.join('')] : [];
    out.push({ via: 'alias', words: [...w, ...joined], phrase: a });
  }
  return out;
}

/** Entries matching every token of the query, best first. An empty query returns nothing. */
export function searchSettings(entries: readonly SettingEntry[], query: string, r: Resolver): Hit[] {
  const tokens = [...new Set(split(normalize(query)))];
  if (!tokens.length) return [];
  const variants = tokens.map((t) => [...new Set([t, ...split(fold(t))])]);
  const hits: (Hit & { order: number })[] = [];
  entries.forEach((entry, order) => {
    const fields = fieldsOf(entry, r);
    let total = 0;
    let best: { score: number; via: Via; alias: string | null } = { score: 0, via: 'label', alias: null };
    for (const vs of variants) {
      let tokBest = 0; let via: Via = 'label'; let alias: string | null = null;
      for (const f of fields) {
        for (const w of f.words) for (const v of vs) {
          const s = tokenTier(v, w) * WEIGHT[f.via];
          if (s > tokBest) { tokBest = s; via = f.via; alias = f.via === 'alias' ? f.phrase : null; }
        }
      }
      if (tokBest === 0) { total = 0; break; }
      total += tokBest;
      if (tokBest > best.score) best = { score: tokBest, via, alias };
    }
    if (total === 0) return;
    // the whole query is the label: this is the one the user means
    if (normalize(r.label(entry)) === normalize(query).trim()) total += 50;
    hits.push({ entry, score: total, via: best.via, alias: best.alias, order });
  });
  return hits.sort((a, b) => b.score - a.score || a.order - b.order).map(({ order: _o, ...h }) => h);
}

/** Keyboard step of the result list: arrows move (wrapping), Home/End jump, Enter picks, Escape clears. */
export type KeyAction = 'none' | 'pick' | 'clear';
export function keyStep(key: string, active: number, len: number): { active: number; action: KeyAction } {
  if (key === 'Escape') return { active: 0, action: 'clear' };
  if (len <= 0) return { active, action: 'none' };
  if (key === 'ArrowDown') return { active: (active + 1) % len, action: 'none' };
  if (key === 'ArrowUp') return { active: (active - 1 + len) % len, action: 'none' };
  if (key === 'Home') return { active: 0, action: 'none' };
  if (key === 'End') return { active: len - 1, action: 'none' };
  if (key === 'Enter') return { active: Math.min(active, len - 1), action: 'pick' };
  return { active, action: 'none' };
}
