import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockApi } from './mockApi';
import { actions, app, errorText, init, resetStores } from './stores.svelte';

describe('background + icon commands (Part 8)', () => {
  beforeEach(() => { vi.useFakeTimers(); resetStores(); });
  afterEach(() => vi.useRealTimers());

  async function boot() {
    const api = createMockApi();
    await init(api);
    return { api, id: app.profiles[0].id };
  }

  it('wallpaper and panorama are persisted kinds, not a local preference', async () => {
    const { id } = await boot();
    await actions.setBackgroundKind(id, 'wallpaper', 'deep-dark');
    expect(app.profiles.find((p) => p.id === id)?.background).toEqual({ kind: 'wallpaper', id: 'deep-dark' });
    await actions.setBackgroundKind(id, 'panorama');
    expect(app.profiles.find((p) => p.id === id)?.background).toEqual({ kind: 'panorama' });
    await actions.setBackgroundKind(id, 'auto');
    expect(app.profiles.find((p) => p.id === id)?.background).toEqual({ kind: 'auto' });
  });

  it('sends exactly the documented args (the dispatch rejects unknown keys)', async () => {
    const { api, id } = await boot();
    const kind = vi.spyOn(api, 'profileBackgroundSetKind');
    const style = vi.spyOn(api, 'profileBackgroundSetStyle');
    await actions.setBackgroundKind(id, 'none');
    await actions.setBackgroundKind(id, 'wallpaper', 'deep-dark');
    await actions.setBackgroundStyle(id, { dim: 0.3 });
    expect(kind.mock.calls).toEqual([[{ id, kind: 'none' }], [{ id, kind: 'wallpaper', wallpaper: 'deep-dark' }]]);
    expect(style.mock.calls).toEqual([[{ id, dim: 0.3 }]]);
  });

  it('dim and blur persist, survive a kind change and are reset by clear', async () => {
    const { id } = await boot();
    await actions.setBackgroundKind(id, 'panorama');
    await actions.setBackgroundStyle(id, { dim: 0.25, blur: 10 });
    expect(app.profiles[0].background).toEqual({ kind: 'panorama', dim: 0.25, blur: 10 });
    await actions.setBackgroundStyle(id, { blur: 3 }); // absent field stays
    expect(app.profiles[0].background).toEqual({ kind: 'panorama', dim: 0.25, blur: 3 });
    await actions.setBackgroundKind(id, 'wallpaper', 'deep-dark');
    expect(app.profiles[0].background).toEqual({ kind: 'wallpaper', id: 'deep-dark', dim: 0.25, blur: 3 });
    await actions.clearBackground(id);
    expect(app.profiles[0].background).toEqual({ kind: 'auto' });
  });

  it('rejects out-of-range style and bad kinds with stable codes, never clamping', async () => {
    const { id } = await boot();
    for (const style of [{ dim: 1.5 }, { dim: -0.1 }, { blur: 25 }, { blur: Number.NaN }]) {
      await expect(actions.setBackgroundStyle(id, style)).rejects.toMatchObject({ code: 'background.bad_style' });
    }
    await expect(actions.setBackgroundKind(id, 'wallpaper', 'Bad_Name')).rejects.toMatchObject({ code: 'background.bad_wallpaper' });
    await expect(actions.setBackgroundKind(id, 'wallpaper')).rejects.toMatchObject({ code: 'background.bad_wallpaper' });
    expect(app.profiles[0].background).toEqual({ kind: 'auto' });
  });

  it('the new error codes have localized messages', async () => {
    expect(errorText({ code: 'background.bad_style', message: '', needsRelogin: false })).not.toBe(errorText({ code: 'zzz', message: '', needsRelogin: false }));
    expect(errorText({ code: 'icon.no_modpack_logo', message: '', needsRelogin: false })).toMatch(/modpack/i);
  });

  it('"use modpack logo" restores the pack image after a custom one and tracks iconSource', async () => {
    const { api } = await boot();
    const search = api.searchProjects({ params: { projectType: 'modpack', sort: 'relevance', offset: 0, limit: 5 } });
    await vi.advanceTimersByTimeAsync(500);
    const hit = (await search).hits[0];
    expect(hit).toBeTruthy();
    const install = api.modpackInstall({ projectId: hit.projectId });
    await vi.advanceTimersByTimeAsync(3000);
    const pack = await install;
    expect(pack.iconSource).toBe('modpack');
    const custom = await api.profileIconSetImage({ id: pack.id, filePath: 'blob:custom.png' });
    expect(custom.iconSource).toBe('custom');
    const back = await api.profileIconUseModpackLogo({ id: pack.id });
    expect(back.iconSource).toBe('modpack');
    expect(back.coverUrl).toBe(pack.coverUrl);
  });

  it('"use modpack logo" without a pack logo fails with icon.no_modpack_logo', async () => {
    const { api, id } = await boot();
    await expect(api.profileIconUseModpackLogo({ id })).rejects.toMatchObject({ code: 'icon.no_modpack_logo' });
  });

  it('block and clear drop iconSource', async () => {
    const { api, id } = await boot();
    await api.profileIconSetImage({ id, filePath: 'blob:a.png' });
    expect((await api.appState()).profiles[0].iconSource).toBe('custom');
    await actions.clearIcon(id);
    expect(app.profiles[0].iconSource).toBeNull();
  });
});
