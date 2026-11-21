// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Harness from './testing/ServersHarness.svelte';
import { createMockApi } from '../lib/mockApi';
import { app, init, nav, resetStores, servers } from '../lib/stores.svelte';

beforeEach(() => {
  Object.defineProperty(HTMLImageElement.prototype, 'complete', { configurable: true, get: () => true });
  Object.defineProperty(HTMLImageElement.prototype, 'naturalWidth', { configurable: true, get: () => 64 });
  (HTMLImageElement.prototype as unknown as { decode: () => Promise<void> }).decode = () => Promise.resolve();
  (HTMLDialogElement.prototype as unknown as { showModal: () => void }).showModal = function (this: HTMLDialogElement) { this.setAttribute('open', ''); };
  (HTMLDialogElement.prototype as unknown as { close: () => void }).close = function (this: HTMLDialogElement) { this.removeAttribute('open'); };
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => setTimeout(() => f(0), 16));
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  (HTMLCanvasElement.prototype as unknown as { getContext: () => unknown }).getContext = () => new Proxy({}, { get: () => () => undefined, set: () => true });
  (HTMLCanvasElement.prototype as unknown as { toDataURL: () => string }).toDataURL = () => 'data:image/png;base64,';
  (HTMLElement.prototype as unknown as { animate: () => unknown }).animate = () => ({ cancel() {}, finish() {}, play() {}, pause() {}, finished: Promise.resolve(), onfinish: null });
  vi.useFakeTimers();
  history.replaceState(null, '', '/');
  resetStores();
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); document.body.innerHTML = ''; });

async function open(view: 'home' | 'servers', opts: Parameters<typeof createMockApi>[0] = {}) {
  await init(createMockApi({ timeScale: 0.05, ...opts }));
  nav.view = view;
  const target = document.createElement('div'); document.body.append(target);
  const a = mount(Harness, { target });
  flushSync();
  await vi.advanceTimersByTimeAsync(3000);
  return { target, a };
}
const text = (el: Element | null | undefined) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim();

describe('Home quick join', () => {
  it('uses the same MediaCard / strip as the instances, with the favicon area, name, version range and players + ping only', async () => {
    const { target, a } = await open('home');
    const strips = target.querySelectorAll('ul.strip');
    expect(strips.length).toBe(2); // instances + servers
    const cards = [...strips[1].querySelectorAll('[data-media-card]')];
    expect(cards.length).toBe(servers.list.length);
    const base = (el?: Element | null) => (el?.className ?? '').replace(/ ?(sel|live|dimmed)\b/g, '');
    expect(base(strips[0].querySelector('[data-media-card]'))).toBe(base(cards[0]));
    const first = cards[0];
    expect(first.querySelector('.art img')).toBeTruthy();                       // real favicon image in the image area
    expect(text(first.querySelector('.nm'))).toBe('VincentVanilla');
    const lines = [...first.querySelectorAll('[data-info-line]')];
    expect(text(lines[0])).toContain('1.21.5 - 26.x');
    expect(text(lines[1])).toMatch(/0\/500.*\d+ ms/);
    expect(lines[0].querySelector('[data-fact="version"] svg')).toBeTruthy();
    expect(lines[1].querySelector('[data-fact="players"] svg')).toBeTruthy();
    expect(lines[1].querySelector('[data-fact="ping"] svg')).toBeTruthy();
    // nothing else: no MOTD, no description text
    expect(text(first)).not.toMatch(/Survival$|Gate|Folia|VincentVanilla 1/);
    for (const c of cards) expect(text(c)).not.toContain(' / ');
    unmount(a);
  });

  it('a click joins directly when the selected instance fits, else the chooser opens', async () => {
    const { target, a } = await open('home');
    const sel = app.profiles.find((p) => p.id === app.config?.selectedProfile) ?? app.profiles[0];
    expect(sel.mcVersion).toBe('1.21.4');
    const card = (name: string) => [...target.querySelectorAll('ul.strip')[1].querySelectorAll<HTMLElement>('[data-media-card]')].find((c) => text(c.querySelector('.nm')) === name)!;
    // Iron Hollow SMP is 1.21.4 only: fits the selected 1.21.4 instance -> no dialog, launch starts
    const api = (await import('../lib/stores.svelte')).getApi();
    const launch = vi.spyOn(api, 'gameLaunch');
    card('Iron Hollow SMP').querySelector<HTMLButtonElement>('button.open')!.click();
    await vi.advanceTimersByTimeAsync(50);
    expect(servers.join).toBeNull();
    expect(launch).toHaveBeenCalledWith({ profileId: sel.id, quickJoin: 'smp.ironhollow.example:25570' });
    launch.mockClear();
    // VincentVanilla needs 1.21.5+: the chooser opens, nothing starts
    nav.view = 'home';
    card('VincentVanilla').querySelector<HTMLButtonElement>('button.open')!.click();
    flushSync();
    await vi.advanceTimersByTimeAsync(50);
    expect(servers.join?.name).toBe('VincentVanilla');
    expect(launch).not.toHaveBeenCalled();
    // unknown range (Creative Plots, Folia): chooser too
    servers.join = null;
    card('Creative Plots').querySelector<HTMLButtonElement>('button.open')!.click();
    expect((servers.join as { name: string } | null)?.name).toBe('Creative Plots');
    unmount(a);
  });
});

describe('Servers view', () => {
  it('has no "join with" dropdown, shows only icon, name, address, players, ping, star and the Partner badge, and VincentVanilla first', async () => {
    const { target, a } = await open('servers');
    expect(target.querySelector('label.f select')).toBeNull();
    const rows = [...target.querySelectorAll('li.row')];
    expect(text(rows[0].querySelector('.nm'))).toBe('VincentVanilla');
    expect(rows[0].querySelector('.tag')).toBeTruthy();
    expect(rows[0].querySelector('.star.pinned')).toBeTruthy();
    expect(rows[0].querySelector('button[aria-label]:not(.join)')).toBeNull();          // the partner cannot be edited, starred or removed
    expect(text(rows[0].querySelector('.addr'))).toBe('vincentvanilla.net');
    for (const r of rows) {
      expect(r.querySelector('img')).toBeTruthy();                                      // favicon (real or the MangoClient fallback)
      expect(text(r)).not.toMatch(/Season 14|whitelisted|Free plots|Gate|Folia|Velocity|Bedwars, Skyblock/);
    }
    // starred first, then the rest, in list order
    expect(rows.map((r) => text(r.querySelector('.nm')))).toEqual(['VincentVanilla', 'Iron Hollow SMP', 'Bedwars Arena', 'Skyblock Realms', 'Creative Plots', 'Old Survival 1.8', 'Modern Realm']);
    unmount(a);
  });

  it('sorts by ping and name but the partner stays first; search filters', async () => {
    const { target, a } = await open('servers');
    const names = () => [...target.querySelectorAll('li.row .nm')].map(text);
    const sort = target.querySelector<HTMLSelectElement>('.sort select')!;
    sort.value = 'name'; sort.dispatchEvent(new Event('change', { bubbles: true })); flushSync();
    expect(names()[0]).toBe('VincentVanilla');
    expect(names().slice(2)).toEqual(['Bedwars Arena', 'Creative Plots', 'Modern Realm', 'Old Survival 1.8', 'Skyblock Realms']);
    sort.value = 'ping'; sort.dispatchEvent(new Event('change', { bubbles: true })); flushSync();
    expect(names().slice(0, 2)).toEqual(['VincentVanilla', 'Iron Hollow SMP']);
    expect(names().slice(-2).sort()).toEqual(['Old Survival 1.8', 'Skyblock Realms']);    // offline last
    const q = target.querySelector<HTMLInputElement>('input[type=search]')!;
    q.value = 'arena'; q.dispatchEvent(new Event('input', { bubbles: true })); flushSync();
    expect(names()).toEqual(['VincentVanilla', 'Bedwars Arena']);
    unmount(a);
  });

  it('Join opens the instance chooser: fitting first, others disabled with the reason', async () => {
    const { target, a } = await open('servers');
    const row = [...target.querySelectorAll('li.row')].find((r) => text(r.querySelector('.nm')) === 'VincentVanilla')!;
    row.querySelector<HTMLButtonElement>('button.join')!.click();
    flushSync();
    await vi.advanceTimersByTimeAsync(300);
    const dlg = document.querySelector('dialog')!;
    expect(dlg).toBeTruthy();
    expect(text(dlg)).toContain('1.21.5 - 26.x');
    const rows = [...dlg.querySelectorAll<HTMLButtonElement>('ul.list button.row')];
    expect(rows.length).toBe(app.profiles.length);
    const disabled = rows.filter((r) => r.disabled);
    expect(disabled.length).toBeGreaterThan(0);
    for (const r of disabled) expect(text(r)).toMatch(/1\.21\.5 - 26\.x/);
    // enabled rows come before disabled ones
    const firstDisabled = rows.findIndex((r) => r.disabled);
    expect(rows.slice(firstDisabled).every((r) => r.disabled)).toBe(true);
    unmount(a);
  });

  it('adds, stars, renames and removes a server; the partner cannot be removed', async () => {
    const { target, a } = await open('servers');
    const names = () => [...target.querySelectorAll('li.row .nm')].map(text);
    const api = (await import('../lib/stores.svelte')).getApi();
    const { actions } = await import('../lib/stores.svelte');
    await actions.serverAdd('My Server', 'my.example.net');
    flushSync();
    expect(names()).toContain('My Server');
    await actions.serverStar('user:my.example.net', true);
    flushSync();
    expect(names().slice(0, 3)).toEqual(['VincentVanilla', 'Iron Hollow SMP', 'My Server']);
    await actions.serverUpdate('user:my.example.net', 'Renamed', 'moved.example.net');
    flushSync();
    expect(servers.list.find((s) => s.name === 'Renamed')?.address).toBe('moved.example.net');
    await expect(actions.serverAdd('Dup', 'MOVED.example.net')).rejects.toMatchObject({ code: 'servers.duplicate' });
    await actions.serverRemove('user:moved.example.net');
    flushSync();
    expect(names()).not.toContain('Renamed');
    await expect(api.serversRemove({ id: 'vincentvanilla' })).rejects.toMatchObject({ code: 'servers.partner' });
    await expect(api.serversStar({ id: 'vincentvanilla', starred: false })).rejects.toMatchObject({ code: 'servers.partner' });
    unmount(a);
  });

  it('states: empty list, all offline, unknown versions', async () => {
    let r = await open('servers', { servers: 'none' });
    expect(r.target.querySelectorAll('li.row').length).toBe(0);
    expect(r.target.textContent).toBeTruthy();
    unmount(r.a); resetStores();
    r = await open('servers', { servers: 'offline' });
    expect(r.target.querySelectorAll('li.row.off').length).toBe(servers.list.length);
    unmount(r.a); resetStores();
    r = await open('home', { servers: 'unknown' });
    const cards = r.target.querySelectorAll('ul.strip')[1].querySelectorAll('[data-media-card] [data-fact="version"]');
    expect(cards.length).toBeGreaterThan(0);
    for (const c of cards) expect(text(c)).toContain('–');
    unmount(r.a);
  });
});
