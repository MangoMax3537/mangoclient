import { describe, expect, it } from 'vitest';
import { heroLine, lastPlayedText, versionLoader } from './infoLine';
import { fmtShortDate } from './format';
import { translate } from './i18n';
const tde = ((k: string, v?: never) => translate('de', k as never, v)) as never;
const ten = ((k: string, v?: never) => translate('en', k as never, v)) as never;

const now = new Date(2026, 10, 21).getTime();
const played = new Date(2026, 10, 16).getTime();
describe('info line', () => {
  it('short date: de 16.11., en Nov 16, year only when not current', () => {
    expect(fmtShortDate(played, 'de', now)).toBe('16.11.');
    expect(fmtShortDate(played, 'en', now)).toBe('Nov 16');
    expect(fmtShortDate(new Date(2025, 10, 16).getTime(), 'de', now)).toBe('16.11.2025');
    expect(fmtShortDate(new Date(2025, 10, 16).getTime(), 'en', now)).toBe('Nov 16, 2025');
  });
  it('hero line has version / loader / last played and no Java or RAM, en and de', () => {
    const p = { mcVersion: '1.21.11', loader: 'fabric', lastPlayed: played };
    expect(heroLine(p, 'de', tde, now)).toBe('1.21.11 / fabric / Zuletzt gespielt: 16.11.');
    expect(heroLine({ ...p, lastPlayed: null }, 'de', tde, now)).toBe('1.21.11 / fabric / Noch nie gespielt');
    expect(heroLine(p, 'en', ten, now)).toBe('1.21.11 / fabric / Last played: Nov 16');
    expect(lastPlayedText({ ...p, lastPlayed: null }, 'en', ten)).toBe('Never played');
    expect(versionLoader({ ...p, loader: 'vanilla' }, ten)).toBe('1.21.11 / Vanilla');
  });
});
