import { describe, expect, it, vi } from 'vitest';
import { INITIAL_WIN, MAC_INSET, RESIZE_ZONES, stripMouseAction, titleBarModel, windowControls } from './titlebar';
import { detectPlatform } from './platform';

const w = (o: Partial<typeof INITIAL_WIN> = {}) => ({ ...INITIAL_WIN, ...o });

describe('titleBarModel', () => {
  it('Windows and Linux: own controls, drag region, resize zones', () => {
    for (const m of ['win', 'linux'] as const) {
      expect(titleBarModel(m, w())).toMatchObject({ showControls: true, dragRegion: true, resizeZones: true, leftInset: 0, dimmed: false, maxIcon: 'maximize' });
    }
  });
  it('maximized: restore icon, no resize zones; fullscreen: no resize zones', () => {
    expect(titleBarModel('win', w({ maximized: true }))).toMatchObject({ maxIcon: 'restore', resizeZones: false, showControls: true });
    expect(titleBarModel('linux', w({ fullscreen: true })).resizeZones).toBe(false);
  });
  it('unfocused window dims the strip', () => {
    expect(titleBarModel('win', w({ focused: false })).dimmed).toBe(true);
  });
  it('macOS: native traffic lights, no controls, no resize zones, left inset', () => {
    expect(titleBarModel('mac', w())).toMatchObject({ showControls: false, resizeZones: false, dragRegion: true, leftInset: MAC_INSET });
    expect(MAC_INSET).toBeGreaterThanOrEqual(76);
  });
  it('plain browser: nothing', () => {
    expect(titleBarModel('browser', w({ focused: false }))).toMatchObject({ showControls: false, dragRegion: false, resizeZones: false, leftInset: 0, dimmed: false });
  });
  it('has the 8 resize directions', () => {
    expect(RESIZE_ZONES.map((z) => z.dir).sort()).toEqual(['east', 'north', 'northeast', 'northwest', 'south', 'southeast', 'southwest', 'west']);
  });
});

describe('strip mouse actions', () => {
  const t = (inButton: boolean) => ({ closest: (s: string) => (inButton && s.includes('button') ? {} : null) }) as unknown as EventTarget;
  it('left button on the empty strip drags, double click toggles maximize', () => {
    expect(stripMouseAction({ button: 0, detail: 1, target: t(false) }, true)).toBe('drag');
    expect(stripMouseAction({ button: 0, detail: 2, target: t(false) }, true)).toBe('toggle');
  });
  it('not on buttons, other mouse buttons, or without a drag region', () => {
    expect(stripMouseAction({ button: 0, detail: 1, target: t(true) }, true)).toBe('none');
    expect(stripMouseAction({ button: 2, detail: 1, target: t(false) }, true)).toBe('none');
    expect(stripMouseAction({ button: 0, detail: 1, target: t(false) }, false)).toBe('none');
  });
});

describe('windowControls', () => {
  const mk = () => ({
    windowMinimize: vi.fn(async () => {}), windowToggleMaximize: vi.fn(async () => {}), windowClose: vi.fn(async () => {}),
    windowIsMaximized: vi.fn(async () => true), windowStartDrag: vi.fn(async () => {}), windowStartResize: vi.fn(async () => {}),
  });
  it('calls the matching commands', async () => {
    const api = mk(); const c = windowControls(api);
    await c.minimize(); await c.toggleMaximize(); await c.close(); await c.startDrag(); await c.startResize('southeast');
    expect(api.windowMinimize).toHaveBeenCalledTimes(1);
    expect(api.windowToggleMaximize).toHaveBeenCalledTimes(1);
    expect(api.windowClose).toHaveBeenCalledTimes(1);
    expect(api.windowStartDrag).toHaveBeenCalledTimes(1);
    expect(api.windowStartResize).toHaveBeenCalledWith({ direction: 'southeast' });
    expect(await c.isMaximized()).toBe(true);
  });
  it('swallows window.unsupported (dev server) and window.forbidden', async () => {
    const api = mk();
    api.windowClose.mockRejectedValue({ code: 'window.unsupported' });
    api.windowIsMaximized.mockRejectedValue({ code: 'window.forbidden' });
    const c = windowControls(api);
    await expect(c.close()).resolves.toBeUndefined();
    expect(await c.isMaximized()).toBe(false);
  });
});

describe('detectPlatform', () => {
  it('Tauri: the real OS decides the mode', () => {
    expect(detectPlatform({ tauri: true, navPlatform: 'MacIntel' })).toMatchObject({ mode: 'mac', mac: true });
    expect(detectPlatform({ tauri: true, uaPlatform: 'Windows' })).toMatchObject({ mode: 'win', mac: false });
    expect(detectPlatform({ tauri: true, navPlatform: 'Linux x86_64' })).toMatchObject({ mode: 'linux' });
  });
  it('browser: no shell mode, keybinds still follow the real OS', () => {
    expect(detectPlatform({ tauri: false, navPlatform: 'MacIntel' })).toMatchObject({ mode: 'browser', mac: true });
  });
  it('?mockPlatform simulates a shell only without Tauri', () => {
    expect(detectPlatform({ tauri: false, search: '?mockPlatform=mac', navPlatform: 'Linux' })).toMatchObject({ mode: 'mac', mac: true });
    expect(detectPlatform({ tauri: false, search: '?mockPlatform=browser', navPlatform: 'Linux' })).toMatchObject({ mode: 'browser' });
    expect(detectPlatform({ tauri: true, search: '?mockPlatform=mac', navPlatform: 'Linux' })).toMatchObject({ mode: 'linux' });
  });
});
