import { describe, expect, it } from 'vitest';
import { NavHistory, HISTORY_CAP, type NavSnapshot } from './history';

const s = (view: string, o: Partial<NavSnapshot> = {}): NavSnapshot => ({ view, profileId: null, profileTab: 'overview', discoverProfile: null, discoverType: 'mod', ...o });

describe('NavHistory', () => {
  it('records states, goes back and forward', () => {
    const h = new NavHistory();
    h.record(s('home')); h.record(s('library')); h.record(s('servers'));
    expect(h.canBack).toBe(true); expect(h.canForward).toBe(false);
    expect(h.back()?.view).toBe('library');
    expect(h.back()?.view).toBe('home');
    expect(h.back()).toBeNull();
    expect(h.canBack).toBe(false);
    expect(h.forward()?.view).toBe('library');
    expect(h.canForward).toBe(true);
  });
  it('ignores the same state and irrelevant fields of other views', () => {
    const h = new NavHistory();
    h.record(s('home')); h.record(s('home', { profileId: 'x', profileTab: 'mods' })); h.record(s('library', { discoverProfile: 'y' }));
    expect(h.size).toBe(2);
  });
  it('profile pages and tabs are separate states; discover scope too', () => {
    const h = new NavHistory();
    h.record(s('profile', { profileId: 'a' })); h.record(s('profile', { profileId: 'a', profileTab: 'mods' })); h.record(s('profile', { profileId: 'b', profileTab: 'mods' }));
    h.record(s('discover', { discoverProfile: 'a', discoverType: 'shader' }));
    expect(h.size).toBe(4);
    expect(h.back()).toMatchObject({ view: 'profile', profileId: 'b', profileTab: 'mods' });
    expect(h.back()).toMatchObject({ profileId: 'a', profileTab: 'mods' });
  });
  it('a new state after going back drops the forward entries', () => {
    const h = new NavHistory();
    h.record(s('home')); h.record(s('library')); h.record(s('servers'));
    h.back(); h.back();
    h.record(s('stats'));
    expect(h.canForward).toBe(false);
    expect(h.back()?.view).toBe('home');
  });
  it('skips entries that are no longer valid (deleted profile)', () => {
    const h = new NavHistory();
    h.record(s('home')); h.record(s('profile', { profileId: 'gone' })); h.record(s('library'));
    expect(h.back((x) => x.profileId !== 'gone')?.view).toBe('home');
    expect(h.forward((x) => x.profileId !== 'gone')?.view).toBe('library');
  });
  it('restoring does not record: record(current) is a no-op', () => {
    const h = new NavHistory();
    h.record(s('home')); h.record(s('library'));
    const b = h.back() as NavSnapshot;
    h.record(b);
    expect(h.canForward).toBe(true);
  });
  it('is capped', () => {
    const h = new NavHistory();
    for (let i = 0; i < HISTORY_CAP + 20; i++) h.record(s('profile', { profileId: `p${i}` }));
    expect(h.size).toBe(HISTORY_CAP);
  });
});
