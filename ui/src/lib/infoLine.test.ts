import { describe, expect, it } from 'vitest';
import { heroFacts, lastPlayedText, playedShortFact, versionLoaderFacts } from './infoLine';
import { fmtShortDate } from './format';
import { translate } from './i18n';
const tde = ((k: string, v?: never) => translate('de', k as never, v)) as never;
const ten = ((k: string, v?: never) => translate('en', k as never, v)) as never;

const now = new Date(2026, 10, 21).getTime();
const played = new Date(2026, 10, 16).getTime();
describe('info facts', () => {
  it('short date: de 16.11., en Nov 16, year only when not current', () => {
    expect(fmtShortDate(played, 'de', now)).toBe('16.11.');
    expect(fmtShortDate(played, 'en', now)).toBe('Nov 16');
    expect(fmtShortDate(new Date(2025, 10, 16).getTime(), 'de', now)).toBe('16.11.2025');
    expect(fmtShortDate(new Date(2025, 10, 16).getTime(), 'en', now)).toBe('Nov 16, 2025');
  });
  it('the hero line is three facts (version, loader, last played), no separators, no Java or RAM', () => {
    const p = { mcVersion: '1.21.11', loader: 'fabric', lastPlayed: played };
    expect(heroFacts(p, 'de', tde, now)).toEqual([
      { kind: 'version', text: '1.21.11' }, { kind: 'loader', text: 'fabric' }, { kind: 'played', text: 'Zuletzt gespielt: 16.11.', dim: false },
    ]);
    expect(heroFacts({ ...p, lastPlayed: null }, 'de', tde, now)[2].text).toBe('Noch nie gespielt');
    expect(heroFacts(p, 'en', ten, now)[2].text).toBe('Last played: Nov 16');
    expect(lastPlayedText({ ...p, lastPlayed: null }, 'en', ten)).toBe('Never played');
    expect(versionLoaderFacts({ ...p, loader: 'vanilla' }, ten).map((f) => f.text)).toEqual(['1.21.11', 'Vanilla']);
    for (const f of heroFacts(p, 'en', ten, now)) expect(f.text).not.toContain(' / ');
    // cards: the date only, the icon (with its label) says what it is
    expect(playedShortFact(p, 'de', tde, now)).toEqual({ kind: 'played', text: '16.11.', dim: true });
    expect(playedShortFact({ ...p, lastPlayed: null }, 'en', ten, now).text).toBe('Never played');
  });
});
