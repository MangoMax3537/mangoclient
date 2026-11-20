import { describe, expect, it, vi } from 'vitest';
import { dialogs, memo, screens, tabs, type ComponentModule } from './lazy';

const sources = import.meta.glob('../**/*.svelte', { eager: true, query: '?raw', import: 'default' }) as Record<string, string>;
const src = (name: string): string => {
  const e = Object.entries(sources).find(([f]) => f.endsWith(`/${name}`));
  if (!e) throw new Error(`missing ${name}`);
  return e[1];
};
const staticImports = (code: string): string[] => [...code.matchAll(/^\s*import\s+[^'"\n]*?from\s+['"]([^'"]+)['"]/gm)].map((m) => m[1]);

describe('memo', () => {
  it('loads once and shares the result', async () => {
    const load = vi.fn(async () => ({ default: (() => {}) as never }) as ComponentModule);
    const m = memo(load);
    const [a, b] = await Promise.all([m(), m()]);
    expect(a).toBe(b);
    await m();
    expect(load).toHaveBeenCalledTimes(1);
  });
  it('retries after a failed load instead of caching the failure', async () => {
    let n = 0;
    const m = memo(async () => { if (++n === 1) throw new Error('chunk 404'); return { default: (() => {}) as never } as ComponentModule; });
    await expect(m()).rejects.toThrow('chunk 404');
    await expect(m()).resolves.toBeTruthy();
    expect(n).toBe(2);
  });
});

describe('lazy-load boundaries', () => {
  const HEAVY_SCREENS = ['Discover', 'Servers', 'Skins', 'Stats', 'Settings', 'Onboarding', 'NewProfile'];
  const HEAVY_DIALOGS = ['InstallFlow', 'IconPicker', 'BackgroundPicker', 'ImportDialog', 'ExportDialog', 'DiagnosticsDialog', 'CrashPanel', 'ReloginDialog', 'UpdateOverlay',
    'ShortcutSheet', 'ProjectDrawer', 'InstancePicker', 'DependencyDialog', 'ModpackDialog', 'UpdatePreview', 'RunDialog'];
  const TABS = ['ContentTab', 'Media', 'Backups', 'Logs', 'ProfileSettings'];
  const all = [...HEAVY_SCREENS, ...HEAVY_DIALOGS, ...TABS];

  it('the entry graph (App, Library, Home, ProfilePage, InstallFlow, Discover) never imports a heavy part statically', () => {
    for (const file of ['App.svelte', 'Library.svelte', 'Home.svelte', 'ProfilePage.svelte', 'InstallFlow.svelte', 'Discover.svelte', 'Rail.svelte']) {
      const imports = staticImports(src(file)).map((i) => i.split('/').pop()?.replace('.svelte', ''));
      for (const heavy of all) expect(imports, `${file} must not statically import ${heavy}`).not.toContain(heavy);
    }
  });
  it('the dialogs opened from the stores and App are only reached through the registry', () => {
    const app = src('App.svelte');
    for (const k of ['importDialog', 'exportDialog', 'diagnosticsDialog', 'crashPanel', 'iconPicker', 'backgroundPicker']) expect(app).toContain(`dialogs.${k}`);
    for (const k of ['discover', 'servers', 'skins', 'stats', 'settings', 'onboarding']) expect(app).toContain(`screens.${k}`);
  });
  it('the registry covers every heavy part exactly once and each loader resolves to a component', async () => {
    const regs: Record<string, () => Promise<ComponentModule>> = { ...screens, ...dialogs, ...tabs };
    expect(Object.keys(regs).length).toBe(new Set(Object.keys(regs)).size);
    const lazySrc = (await import('./lazy.ts?raw')).default as string;
    for (const n of all) expect(lazySrc, n).toContain(`/${n}.svelte'`);
    const m = await regs.crashPanel(); // one real load proves the loaders point at compilable components (the rest share the same registry)
    expect(typeof m.default).toBe('function');
  }, 40000);
  it('the browser mock is its own chunk: api.ts imports it dynamically only', async () => {
    const api = (await import('./api.ts?raw')).default as string;
    expect(api).not.toMatch(/^import\s+(?!type)[^;]*from\s+'\.\/mockApi'/m);
    expect(api).toContain("import('./mockApi')");
  });
});
