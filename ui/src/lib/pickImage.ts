// Choosing an image for an icon or a background. Inside Tauri the backend's native dialog (`pick_image_file`) and the webview drag-drop event
// give a real file path; in the browser mock an <input type=file> / DOM drop gives a File, passed on as a blob: URL (the mock accepts URLs).
import { inTauri } from './api';
import { getApi } from './stores.svelte';

export const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif'] as const;
export interface PickedImage { path: string; name: string; /** Something an <img> can show now (blob: URL in the browser), else null. */ preview: string | null }

const baseName = (p: string): string => p.split(/[\\/]/).pop() ?? p;
export const isImageName = (n: string): boolean => IMAGE_EXTENSIONS.includes((n.split('.').pop() ?? '').toLowerCase() as (typeof IMAGE_EXTENSIONS)[number]);

export function fromFile(f: File): PickedImage | null {
  if (!isImageName(f.name)) return null;
  const url = URL.createObjectURL(f);
  return { path: url, name: f.name, preview: url };
}
const fromPath = (p: string): PickedImage | null => (isImageName(p) ? { path: p, name: baseName(p), preview: null } : null);

/** Opens the file dialog. `null` = cancelled (or not an image). */
export async function pickImage(title?: string): Promise<PickedImage | null> {
  if (inTauri()) {
    const r = await getApi().pickImageFile(title ? { title } : {});
    return typeof r === 'string' ? fromPath(r) : null;
  }
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file'; input.accept = IMAGE_EXTENSIONS.map((e) => `.${e}`).join(',');
    input.onchange = () => resolve(input.files?.[0] ? fromFile(input.files[0]) : null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

export interface DropHandlers { over: (on: boolean) => void; drop: (img: PickedImage) => void; reject?: () => void }

/**
 * Svelte action: drag-and-drop of an image onto `node`. In Tauri the webview swallows DOM file drops, so its own drag-drop event
 * (real paths) is used; in the browser the DOM events give a File.
 */
export function imageDrop(node: HTMLElement, handlers: DropHandlers) {
  let h = handlers;
  let off = () => {};
  let dead = false;
  const onOver = (e: DragEvent) => { if (!e.dataTransfer?.types.includes('Files')) return; e.preventDefault(); h.over(true); };
  const onLeave = (e: DragEvent) => { if (!(e.relatedTarget instanceof Node) || !node.contains(e.relatedTarget)) h.over(false); };
  const onDrop = (e: DragEvent) => {
    e.preventDefault(); h.over(false);
    const f = e.dataTransfer?.files?.[0];
    const img = f ? fromFile(f) : null;
    if (img) h.drop(img); else h.reject?.();
  };
  node.addEventListener('dragover', onOver); node.addEventListener('dragleave', onLeave); node.addEventListener('drop', onDrop);
  if (inTauri()) {
    import('@tauri-apps/api/webview').then(({ getCurrentWebview }) => getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === 'enter' || p.type === 'over') h.over(true);
      else if (p.type === 'leave') h.over(false);
      else if (p.type === 'drop') {
        h.over(false);
        const img = p.paths.map(fromPath).find((x) => x !== null);
        if (img) h.drop(img); else h.reject?.();
      }
    })).then((u) => { if (dead) u(); else off = u; }, () => {});
  }
  return {
    update(n: DropHandlers) { h = n; },
    destroy() {
      dead = true; off();
      node.removeEventListener('dragover', onOver); node.removeEventListener('dragleave', onLeave); node.removeEventListener('drop', onDrop);
    },
  };
}

/** Native folder dialog (`pick_folder`). Only meaningful in the Tauri shell: the browser has no folder paths, callers keep a text input there. */
export async function pickFolder(title?: string): Promise<string | null> {
  const r = await getApi().pickFolder(title ? { title } : {});
  return typeof r === 'string' && r ? r : null;
}
