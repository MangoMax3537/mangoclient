// Window size fields of the Settings page: whole pixels, bounded to what a Minecraft window can sensibly be.
export const WINDOW_MIN = 320;
export const WINDOW_MAX = 7680;
const whole = (s: string): number | null => (/^\s*\d{1,5}\s*$/.test(s) ? Number(s) : null);
/** `null` when either side is not a whole number in range (the field then shows its error and nothing is saved). */
export function parseWindowSize(w: string, h: string): { width: number; height: number } | null {
  const width = whole(w); const height = whole(h);
  if (width === null || height === null) return null;
  if (width < WINDOW_MIN || width > WINDOW_MAX || height < WINDOW_MIN || height > WINDOW_MAX) return null;
  return { width, height };
}
