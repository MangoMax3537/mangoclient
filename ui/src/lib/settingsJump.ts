// Jump from a settings search result to its row: scroll it into view, flash it (outline pulse, opacity only) and focus its control.
import { tick } from 'svelte';
import type { SettingEntry } from './settingsIndex';

const FOCUSABLE = '[tabindex="0"], input:not([type="hidden"]), select, textarea, button';
export const FLASH_MS = 1500;

/** Waits one tick (the page shows all sections again once the query is cleared), then scrolls to and highlights the entry's row. */
export async function jumpTo(entry: SettingEntry, root: ParentNode = document): Promise<boolean> {
  await tick();
  const el = root.querySelector<HTMLElement>(`#${CSS.escape(entry.target)}`);
  if (!el) return false;
  const calm = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  el.scrollIntoView({ block: 'center', behavior: calm ? 'auto' : 'smooth' });
  el.classList.remove('set-flash');
  void el.offsetWidth; // restart the animation when the same row is picked twice
  el.classList.add('set-flash');
  setTimeout(() => el.classList.remove('set-flash'), FLASH_MS);
  el.querySelector<HTMLElement>(FOCUSABLE)?.focus({ preventScroll: true });
  return true;
}
