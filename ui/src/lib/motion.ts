// Page / panel motion: a short fade plus a few pixels of translate (opacity and transform only), ease-out. Only an `in` transition is used
// for views, so the leaving view is removed at once and navigation never waits on an animation. prefers-reduced-motion = instant.
import { cubicOut } from 'svelte/easing';

export const ENTER_MS = 180;
export const reducedMotion = (): boolean => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

export interface EnterParams { y?: number; duration?: number; delay?: number }
/** Svelte transition: fade in while moving up from `y` px (default 8). */
export function enter(_node: Element, { y = 8, duration = ENTER_MS, delay = 0 }: EnterParams = {}) {
  const d = reducedMotion() ? 0 : duration;
  return { delay: reducedMotion() ? 0 : delay, duration: d, easing: cubicOut, css: (t: number, u: number) => `opacity: ${t}; transform: translate3d(0, ${(u * y).toFixed(2)}px, 0);` };
}

/** Leave transition for dialogs and drawers (short, a little translate; the backdrop fades via the `closing` class). */
export function leave(node: Element, { y = 6, duration = 120 }: EnterParams = {}) {
  node.classList.add('closing');
  const d = reducedMotion() ? 0 : duration;
  return { duration: d, easing: cubicOut, css: (t: number, u: number) => `opacity: ${t}; transform: translate3d(0, ${(u * y).toFixed(2)}px, 0);` };
}

/** Drawer leave: slides out to the right. */
export function leaveDrawer(node: Element, { duration = 160 }: EnterParams = {}) {
  node.classList.add('closing');
  const d = reducedMotion() ? 0 : duration;
  return { duration: d, easing: cubicOut, css: (t: number, u: number) => `opacity: ${t}; transform: translate3d(${(u * 28).toFixed(2)}px, 0, 0);` };
}

/** Leave transition of the shared Modal: drawers slide out, dialogs sink a few pixels. */
export const leaveDialog = (node: Element, p: { drawer?: boolean } = {}) => (p.drawer ? leaveDrawer(node) : leave(node));
