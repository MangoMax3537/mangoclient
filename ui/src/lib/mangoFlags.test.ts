import { describe, expect, it } from 'vitest';
import { mangoFlags, mangoSupported } from './mockData';
import { createMockApi } from './mockApi';
import { translate } from './i18n';

describe('MangoConfig flags (mock mirrors the core)', () => {
  it('supported only for fabric/quilt on 1.20.5, 1.20.6, 1.21 to 1.21.11', () => {
    expect(mangoSupported('1.21.4', 'fabric')).toBe(true);
    expect(mangoSupported('1.21', 'quilt')).toBe(true);
    expect(mangoSupported('1.21.11', 'fabric')).toBe(true);
    expect(mangoSupported('1.21.12', 'fabric')).toBe(false);
    expect(mangoSupported('1.20.4', 'fabric')).toBe(false);
    expect(mangoSupported('1.21.4', 'forge')).toBe(false);
    expect(mangoSupported('1.21.4', 'vanilla')).toBe(false);
  });
  it('active = supported and not switched off by the profile or the global setting', () => {
    const p = { mcVersion: '1.21.4', loader: 'fabric', mangoConfig: null as boolean | null };
    expect(mangoFlags(p, true).mangoConfigActive).toBe(true);
    expect(mangoFlags(p, false).mangoConfigActive).toBe(false);
    expect(mangoFlags({ ...p, mangoConfig: true }, false).mangoConfigActive).toBe(true);
    expect(mangoFlags({ ...p, mangoConfig: false }, true).mangoConfigActive).toBe(false);
    expect(mangoFlags({ ...p, loader: 'forge', mangoConfig: true }, true)).toEqual({ mangoConfigSupported: false, mangoConfigActive: false });
  });
  it('the mock state carries the flags and the German reason is localized', async () => {
    const s = await createMockApi().appState();
    expect(s.profiles.find((p) => p.loader === 'forge')?.mangoConfigSupported).toBe(false);
    expect(s.profiles.find((p) => p.loader === 'fabric' && p.mcVersion === '1.21.4')?.mangoConfigActive).toBe(true);
    expect(translate('de', 'psettings.mangoUnsupported')).toBe('Aus (nicht unterstützt)');
  });
});
