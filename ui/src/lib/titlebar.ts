// The custom title bar as a state machine plus the thin window-command wrapper. Pure and testable: the component only renders what
// `titleBarModel` says and calls what `windowControls` offers.
import type { Api } from './api';
import type { ShellMode } from './platform';

export interface WinState { maximized: boolean; fullscreen: boolean; focused: boolean }
export const INITIAL_WIN: WinState = { maximized: false, fullscreen: false, focused: true };

/** Room for the native traffic lights on macOS (Overlay title bar), in px. */
export const MAC_INSET = 78;
export const EDGE_PX = 6;
export const CORNER_PX = 10;

export interface TitleBarModel {
  mode: ShellMode;
  /** Our own minimize / maximize / close buttons (Windows, Linux). */
  showControls: boolean;
  /** Mouse down on the empty strip starts a window drag, double click toggles maximize. */
  dragRegion: boolean;
  /** The 8 invisible resize hit zones. */
  resizeZones: boolean;
  /** Left padding that keeps the Brand clear of the traffic lights. */
  leftInset: number;
  /** The window is not focused: controls and title are dimmed. */
  dimmed: boolean;
  maxIcon: 'maximize' | 'restore';
}

export function titleBarModel(mode: ShellMode, w: WinState): TitleBarModel {
  const shell = mode !== 'browser';
  const custom = mode === 'win' || mode === 'linux';
  return {
    mode,
    showControls: custom,
    dragRegion: shell,
    resizeZones: custom && !w.maximized && !w.fullscreen,
    leftInset: mode === 'mac' ? MAC_INSET : 0,
    dimmed: shell && !w.focused,
    maxIcon: w.maximized ? 'restore' : 'maximize',
  };
}

export type ResizeDirection = 'north' | 'south' | 'east' | 'west' | 'northeast' | 'northwest' | 'southeast' | 'southwest';
export const RESIZE_ZONES: readonly { dir: ResizeDirection; cursor: string }[] = [
  { dir: 'north', cursor: 'ns-resize' }, { dir: 'south', cursor: 'ns-resize' },
  { dir: 'east', cursor: 'ew-resize' }, { dir: 'west', cursor: 'ew-resize' },
  { dir: 'northeast', cursor: 'nesw-resize' }, { dir: 'southwest', cursor: 'nesw-resize' },
  { dir: 'northwest', cursor: 'nwse-resize' }, { dir: 'southeast', cursor: 'nwse-resize' },
];

/** Elements inside the strip that must keep their own mouse behaviour (no drag, no maximize on double click). */
export const NO_DRAG = 'button, a, input, select, textarea, [role="button"], [data-nodrag]';

export type MouseLike = { button: number; detail: number; target: EventTarget | null };
/** What a mouse down on the strip does: `none` for the wrong button or an interactive child. */
export function stripMouseAction(e: MouseLike, dragRegion: boolean): 'drag' | 'toggle' | 'none' {
  if (!dragRegion || e.button !== 0) return 'none';
  const el = e.target as Element | null;
  if (el && typeof el.closest === 'function' && el.closest(NO_DRAG)) return 'none';
  return e.detail >= 2 ? 'toggle' : 'drag';
}

/** The window commands, errors swallowed (the dev server answers `window.unsupported`, a missing window is not worth a toast). */
export function windowControls(api: Pick<Api, 'windowMinimize' | 'windowToggleMaximize' | 'windowClose' | 'windowIsMaximized' | 'windowStartDrag' | 'windowStartResize'>) {
  const safe = async (p: () => Promise<unknown>): Promise<void> => { try { await p(); } catch { /* unsupported or gone */ } };
  return {
    minimize: () => safe(() => api.windowMinimize()),
    toggleMaximize: () => safe(() => api.windowToggleMaximize()),
    close: () => safe(() => api.windowClose()),
    startDrag: () => safe(() => api.windowStartDrag()),
    startResize: (direction: ResizeDirection) => safe(() => api.windowStartResize({ direction })),
    isMaximized: async (): Promise<boolean> => { try { return !!(await api.windowIsMaximized()); } catch { return false; } },
  };
}
