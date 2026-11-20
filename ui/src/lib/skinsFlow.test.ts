import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import { activeCape, canEditSkin, checkUsername, nameFromFile, sortLibrary } from './skinFlows';
import { app, errorText, init, nav, navBack, navForward, openProfile, resetStores, skinRev, hist, recordNav } from './stores.svelte';
import { checkInstanceSize, missingMajor, requiredJavaMajor, windowPatch, javaLabel, normalizeJavaPath } from './javaHint';

const ONLINE = 'b0b7c2d4-7a41-4c3e-9f5a-12de5a3c9e01';
const OFFLINE = '5d2e8f19-0c36-3a7b-b4d1-9e7f6a2c1b88';
const api = (o = {}) => createMockApi({ timeScale: 0, ...o });

describe('username validation (like Rust)', () => {
  it('3..16 of A-Za-z0-9_', () => {
    for (const ok of ['Dream', 'a_b', 'Name_1234567890', 'x'.repeat(16)]) expect(checkUsername(ok), ok).toBe(true);
    for (const bad of ['ab', 'x'.repeat(17), 'with space', 'ümlaut', 'a-b', '', 'name!']) expect(checkUsername(bad), bad).toBe(false);
  });
  it('file name to library name', () => { expect(nameFromFile('C:\\skins\\my_cool-skin.png')).toBe('my cool skin'); expect(nameFromFile('.png')).toBe('Skin'); });
});

describe('skins against the mock', () => {
  it('online account: editable, capes with one active; offline: not editable, no capes', async () => {
    const a = api();
    const on = await a.skinGet({ uuid: ONLINE });
    expect(canEditSkin(on)).toBe(true); expect(on.capes.length).toBeGreaterThan(0); expect(activeCape(on)?.id).toBe(on.capeUrl ? activeCape(on)?.id : undefined);
    const off = await a.skinGet({ uuid: OFFLINE });
    expect(canEditSkin(off)).toBe(false); expect(off.capes).toEqual([]); expect(off.source).toBe('default'); expect(off.capeUrl).toBeNull();
  });
  it('offline edits are refused with skin.offline_account', async () => {
    const a = api();
    await expect(a.skinSetCape({ uuid: OFFLINE, capeId: null })).rejects.toMatchObject({ code: 'skin.offline_account' });
    await expect(a.skinUseUsername({ uuid: OFFLINE, name: 'Dream' })).rejects.toMatchObject({ code: 'skin.offline_account' });
    await expect(a.skinUpload({ uuid: OFFLINE, filePath: 'blob:x', variant: 'slim' })).rejects.toMatchObject({ code: 'skin.offline_account' });
  });
  it('capes: set, switch, hide; unknown id is a validation error', async () => {
    const a = api();
    const before = await a.skinGet({ uuid: ONLINE });
    const other = before.capes.find((c) => !c.active)!;
    const set = await a.skinSetCape({ uuid: ONLINE, capeId: other.id });
    expect(activeCape(set)?.id).toBe(other.id); expect(set.capeUrl).toBe(other.url);
    const none = await a.skinSetCape({ uuid: ONLINE, capeId: null });
    expect(none.capeUrl).toBeNull(); expect(activeCape(none)).toBeNull();
    await expect(a.skinSetCape({ uuid: ONLINE, capeId: 'nope' })).rejects.toMatchObject({ code: 'validation' });
  });
  it('library: save current, apply, remove; newest first', async () => {
    const a = api();
    const n = (await a.skinLibraryList()).length;
    const saved = await a.skinLibrarySaveCurrent({ uuid: ONLINE, name: 'Mine' });
    const list = sortLibrary(await a.skinLibraryList());
    expect(list).toHaveLength(n + 1); expect(list[0].id).toBe(saved.id);
    const applied = await a.skinLibraryApply({ uuid: ONLINE, id: list[2].id });
    expect(applied.slim).toBe(list[2].variant === 'slim');
    const rest = await a.skinLibraryRemove({ id: saved.id });
    expect(rest).toHaveLength(n);
    await expect(a.skinLibraryRemove({ id: 'gone' })).rejects.toMatchObject({ code: 'skin.not_found' });
  });
  it('library rename: trims, persists, validates, unknown id', async () => {
    const a = api();
    const e = (await a.skinLibraryList())[0];
    const r = await a.skinLibraryRename({ id: e.id, name: '  Fresh  ' });
    expect(r).toMatchObject({ id: e.id, name: 'Fresh' });
    expect((await a.skinLibraryList()).find((x) => x.id === e.id)?.name).toBe('Fresh');
    for (const bad of ['', '   ', 'x'.repeat(49), 'a\nb']) await expect(a.skinLibraryRename({ id: e.id, name: bad })).rejects.toMatchObject({ code: 'skin.invalid_name' });
    await expect(a.skinLibraryRename({ id: 'gone', name: 'Ok' })).rejects.toMatchObject({ code: 'skin.not_found' });
  });
  it('username copy: invalid name, unknown player, ok with model', async () => {
    const a = api();
    await expect(a.skinUseUsername({ uuid: ONLINE, name: 'a' })).rejects.toMatchObject({ code: 'auth.invalid_username' });
    await expect(a.skinUseUsername({ uuid: ONLINE, name: 'nobody_here' })).rejects.toMatchObject({ code: 'skin.not_found' });
    expect((await a.skinUseUsername({ uuid: ONLINE, name: 'Dream', variant: 'slim' })).slim).toBe(true);
    expect((await a.skinUseUsername({ uuid: ONLINE, name: 'Dream', variant: 'classic' })).slim).toBe(false);
  });
  it('upload accepts a picked file and the chosen model; a non-PNG path is skin.invalid_png', async () => {
    const a = api();
    const info = await a.skinUpload({ uuid: ONLINE, filePath: 'blob:abc', variant: 'slim' });
    expect(info.slim).toBe(true); expect(info.skinUrl).toBe('blob:abc');
    await expect(a.skinUpload({ uuid: ONLINE, filePath: '/x/skin.jpg', variant: 'classic' })).rejects.toMatchObject({ code: 'skin.invalid_png' });
  });
  it('?mockSkinError makes edits fail with that code; the error maps to a localized sentence', async () => {
    const a = api({ skinError: 'skin.too_large' });
    const e = await a.skinUpload({ uuid: ONLINE, filePath: 'blob:abc', variant: 'classic' }).catch((x) => x);
    expect(e.code).toBe('skin.too_large');
    resetStores(); app.config = { language: 'en' } as never;
    expect(errorText(e)).toMatch(/too large/i);
    for (const c of ['skin.offline_account', 'skin.invalid_png', 'skin.not_found', 'skin.upload_failed', 'skin.library_full', 'auth.invalid_username']) expect(errorText({ code: c }), c).not.toMatch(/Something went wrong/);
  });
  it('a dead session answers needsRelogin', async () => {
    const a = api({ needsRelogin: true });
    await expect(a.skinSetCape({ uuid: ONLINE, capeId: null })).rejects.toMatchObject({ needsRelogin: true });
  });
  it('every change emits skin:updated, and the store bumps the account revision', async () => {
    resetStores();
    const a = api();
    await init(a);
    const seen = vi.fn();
    await a.listen('skin:updated', seen);
    await a.skinSetCape({ uuid: ONLINE, capeId: null });
    expect(seen).toHaveBeenCalledWith({ uuid: ONLINE });
    expect(skinRev[ONLINE]).toBe(1);
  });
});

describe('navigation history in the store', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());
  it('records views and profile tabs, back / forward restore them', async () => {
    await init(api()); recordNav(); // (the store records through an effect in the browser; effects do not run in the node test build)
    const id = app.profiles[0].id;
    nav.view = 'library'; recordNav();
    openProfile(id, 'mods'); recordNav();
    nav.profileTab = 'logs'; recordNav();
    nav.view = 'skins'; recordNav();
    expect(hist.back).toBe(true); expect(hist.forward).toBe(false);
    navBack(); recordNav();
    expect(nav).toMatchObject({ view: 'profile', profileId: id, profileTab: 'logs' });
    navBack(); recordNav();
    expect(nav).toMatchObject({ view: 'profile', profileTab: 'mods' });
    navBack(); recordNav();
    expect(nav.view).toBe('library');
    expect(hist.forward).toBe(true);
    navForward(); recordNav();
    expect(nav).toMatchObject({ view: 'profile', profileTab: 'mods' });
    // restoring did not add entries
    navForward(); navForward(); recordNav();
    expect(nav.view).toBe('skins'); expect(hist.forward).toBe(false);
  });
  it('a dialog flag is not history', async () => {
    await init(api()); recordNav();
    nav.view = 'library'; recordNav();
    nav.newProfile = true; nav.accounts = true; recordNav();
    navBack(); recordNav();
    expect(nav.view).toBe('home');
  });
});

describe('per-instance Java and window rules', () => {
  it('Java major per Minecraft version', () => {
    expect(requiredJavaMajor('1.21.4')).toBe(21); expect(requiredJavaMajor('1.20.6')).toBe(21); expect(requiredJavaMajor('1.20.1')).toBe(17);
    expect(requiredJavaMajor('1.18.2')).toBe(17); expect(requiredJavaMajor('1.17.1')).toBe(16); expect(requiredJavaMajor('1.16.5')).toBe(8); expect(requiredJavaMajor('1.8.9')).toBe(8);
    expect(requiredJavaMajor('26.1')).toBe(25); expect(requiredJavaMajor('24w40a')).toBe(21); expect(requiredJavaMajor('b1.7.3')).toBeNull();
  });
  const j = (major: number) => ({ path: `/j${major}`, major, raw: '', vendor: 'Temurin', source: 'managed' as const });
  it('missing major and labels', () => {
    expect(missingMajor([j(17)], 21)).toBe(true); expect(missingMajor([j(21)], 21)).toBe(false); expect(missingMajor([], null)).toBe(false);
    expect(javaLabel(j(21))).toBe('Java 21 (Temurin, managed)');
    expect(javaLabel({ ...j(17), vendor: null, source: 'system' })).toBe('Java 17');
  });
  it('window size bounds are the Rust ones and never clamped', () => {
    expect(checkInstanceSize('854', '480').ok).toBe(true); expect(checkInstanceSize('640', '480').ok).toBe(true); expect(checkInstanceSize('16384', '16384').ok).toBe(true);
    expect(checkInstanceSize('639', '720')).toMatchObject({ ok: false, width: false, height: true });
    expect(checkInstanceSize('1280', '479')).toMatchObject({ ok: false, width: true, height: false });
    expect(checkInstanceSize('16385', '720').ok).toBe(false); expect(checkInstanceSize('12x0', '720').ok).toBe(false); expect(checkInstanceSize('', '').ok).toBe(false);
    expect(checkInstanceSize('1280', '720').value).toEqual({ width: 1280, height: 720 });
  });
  it('default and fullscreen carry no size; blank Java path = automatic', () => {
    expect(windowPatch('size', { width: 1600, height: 900 })).toEqual({ mode: 'size', width: 1600, height: 900 });
    expect(windowPatch('fullscreen', { width: 1, height: 1 })).toEqual({ mode: 'fullscreen', width: null, height: null });
    expect(windowPatch('default')).toEqual({ mode: 'default', width: null, height: null });
    expect(normalizeJavaPath('  ')).toBeNull(); expect(normalizeJavaPath(' /usr/bin/java ')).toBe('/usr/bin/java');
  });
  it('the mock saves and validates like Rust', async () => {
    const a = api();
    const id = (await a.appState()).profiles[0].id;
    expect((await a.profileUpdate({ id, patch: { window: { mode: 'size', width: 1600, height: 900 } } })).window).toEqual({ mode: 'size', width: 1600, height: 900 });
    await expect(a.profileUpdate({ id, patch: { window: { mode: 'size', width: 600, height: 900 } } })).rejects.toMatchObject({ code: 'validation' });
    expect((await a.profileUpdate({ id, patch: { window: { mode: 'fullscreen', width: null, height: null } } })).window).toEqual({ mode: 'fullscreen', width: null, height: null });
    await expect(a.profileUpdate({ id, patch: { javaPath: 'not a path' } })).rejects.toMatchObject({ code: 'java.not_runnable' });
    expect((await a.profileUpdate({ id, patch: { javaPath: '/usr/lib/jvm/java-21-openjdk/bin/java' } })).javaPath).toBe('/usr/lib/jvm/java-21-openjdk/bin/java');
    expect((await a.profileUpdate({ id, patch: { javaPath: null } })).javaPath).toBeNull();
    expect((await a.javaList()).length).toBeGreaterThan(0);
  });
});
