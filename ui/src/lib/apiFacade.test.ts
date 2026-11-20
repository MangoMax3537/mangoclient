import { describe, expect, it, vi } from 'vitest';
import { COMMANDS, DEPRECATED_COMMANDS, createApi, type Invoke, type Subscribe } from './apiFacade';
import commandsSrc from './generated/commands.ts?raw';

function fake() {
  const invoke = vi.fn(async (_c: string, _a: unknown) => null as never) as unknown as Invoke & ReturnType<typeof vi.fn>;
  const listen = vi.fn(async () => () => {}) as unknown as Subscribe;
  return { invoke, listen, api: createApi({ invoke, listen }) };
}

describe('typed facade over CommandMap', () => {
  it('every generated command has exactly one UI method or is a documented deprecated alias', () => {
    const generated = [...commandsSrc.matchAll(/^ {2}"(\w+)": \{ args:/gm)].map((m) => m[1]);
    expect(generated.length).toBeGreaterThan(60);
    const covered = new Set<string>([...Object.values(COMMANDS), ...DEPRECATED_COMMANDS]);
    expect(generated.filter((c) => !covered.has(c))).toEqual([]);
    expect([...covered].filter((c) => !generated.includes(c))).toEqual([]);
    const values = Object.values(COMMANDS);
    expect(new Set(values).size).toBe(values.length);
  });

  it('maps each method to its snake_case command and passes the arguments through unchanged', async () => {
    const { api, invoke } = fake();
    await api.profileBackgroundSetStyle({ id: 'p', dim: 0.5, blur: 8 });
    await api.profileBackgroundSetKind({ id: 'p', kind: 'wallpaper', wallpaper: 'deep-dark' });
    await api.profileIconUseModpackLogo({ id: 'p' });
    await api.pickFolder({ title: 'x' });
    expect(invoke.mock.calls).toEqual([
      ['profile_background_set_style', { id: 'p', dim: 0.5, blur: 8 }],
      ['profile_background_set_kind', { id: 'p', kind: 'wallpaper', wallpaper: 'deep-dark' }],
      ['profile_icon_use_modpack_logo', { id: 'p' }],
      ['pick_folder', { title: 'x' }],
    ]);
  });

  it('argument-less calls send an empty object, never undefined', async () => {
    const { api, invoke } = fake();
    await api.appState();
    await api.versionsManifest();
    await api.pickImageFile();
    expect(invoke.mock.calls.map((c) => c[1])).toEqual([{}, {}, {}]);
  });

  it('returns the transport result and rethrows its error', async () => {
    const { invoke, listen } = fake();
    invoke.mockResolvedValueOnce(['a'] as never).mockRejectedValueOnce({ code: 'x', message: 'm', needsRelogin: false });
    const api = createApi({ invoke, listen });
    expect(await api.gameRunning()).toEqual(['a']);
    await expect(api.gameRunning()).rejects.toMatchObject({ code: 'x' });
  });

  it('listen is the transport subscription', async () => {
    const { api, listen } = fake();
    const cb = () => {};
    await api.listen('launch:state', cb);
    expect(listen).toHaveBeenCalledWith('launch:state', cb);
  });
});
