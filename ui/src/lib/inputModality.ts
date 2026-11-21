// Focus rings only for real keyboard use: a mouse / touch / pen press switches <html data-modality> to "pointer" (rings hidden, see
// app.css), a navigation key switches it back to "keyboard". WebKitGTK draws :focus-visible after clicks and on programmatic focus
// (dialogs focusing their first control), so the browser heuristic alone is not enough.
export type Modality = 'pointer' | 'keyboard';

/** Keys that move or act on focus. Typing text into a field does not count as keyboard navigation. */
const NAV_KEYS = new Set(['Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown', 'Enter', ' ', 'Escape', 'F6']);

export const isNavKey = (e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey'>): boolean => NAV_KEYS.has(e.key) && !e.ctrlKey && !e.metaKey && !e.altKey;

export function modality(root: HTMLElement = document.documentElement): Modality {
  return root.dataset.modality === 'keyboard' ? 'keyboard' : 'pointer';
}

/** Starts in "pointer" (no ring until the keyboard is really used). Returns the uninstall function. */
export function installInputModality(doc: Document = document): () => void {
  const root = doc.documentElement;
  const set = (m: Modality) => { if (root.dataset.modality !== m) root.dataset.modality = m; };
  set('pointer');
  const onPointer = () => set('pointer');
  const onKey = (e: KeyboardEvent) => { if (isNavKey(e)) set('keyboard'); };
  doc.addEventListener('pointerdown', onPointer, true);
  doc.addEventListener('mousedown', onPointer, true);
  doc.addEventListener('keydown', onKey, true);
  return () => {
    doc.removeEventListener('pointerdown', onPointer, true);
    doc.removeEventListener('mousedown', onPointer, true);
    doc.removeEventListener('keydown', onKey, true);
  };
}
