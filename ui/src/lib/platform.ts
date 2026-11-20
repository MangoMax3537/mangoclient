// Which operating system the UI runs on and whether the window is drawn by us. Pure: the inputs are passed in, so tests can feed any
// combination. `?mockPlatform=mac|win|linux|browser` simulates a shell in the plain browser (dev only).
export type Os = 'mac' | 'win' | 'linux';
/** `browser` = no Tauri shell (the page has its own browser chrome): no custom title bar logic. */
export type ShellMode = Os | 'browser';

export interface PlatformInput {
  tauri: boolean;
  search?: string;
  /** `navigator.userAgentData?.platform` */
  uaPlatform?: string;
  /** `navigator.platform` */
  navPlatform?: string;
  userAgent?: string;
}

export function detectOs(i: Pick<PlatformInput, 'uaPlatform' | 'navPlatform' | 'userAgent'>): Os {
  const s = `${i.uaPlatform ?? ''} ${i.navPlatform ?? ''} ${i.userAgent ?? ''}`;
  if (/mac|iphone|ipad/i.test(s)) return 'mac';
  if (/win/i.test(s)) return 'win';
  return 'linux';
}

export interface Platform { os: Os; mode: ShellMode; mac: boolean }

export function detectPlatform(i: PlatformInput): Platform {
  const real = detectOs(i);
  let mock: string | null = null;
  try { mock = new URLSearchParams(i.search ?? '').get('mockPlatform'); } catch { /* none */ }
  if (!i.tauri && (mock === 'mac' || mock === 'win' || mock === 'linux')) return { os: mock, mode: mock, mac: mock === 'mac' };
  if (!i.tauri) return { os: real, mode: 'browser', mac: real === 'mac' };
  return { os: real, mode: real, mac: real === 'mac' };
}

/** Reads the real environment (browser only). */
export function currentPlatform(tauri: boolean): Platform {
  const nav = typeof navigator !== 'undefined' ? (navigator as Navigator & { userAgentData?: { platform?: string } }) : undefined;
  return detectPlatform({
    tauri, search: typeof location !== 'undefined' ? location.search : '',
    uaPlatform: nav?.userAgentData?.platform, navPlatform: nav?.platform, userAgent: nav?.userAgent,
  });
}
