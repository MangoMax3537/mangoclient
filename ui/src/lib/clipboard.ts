// Clipboard text with a visible result: a toast on success, a toast on failure (no permission), never silent.
import { t, toast } from './stores.svelte';

export async function copyText(text: string, what = text): Promise<void> {
  try { await navigator.clipboard.writeText(text); toast(t('menu.copied', { what }), 'ok', 2500); }
  catch { toast(t('menu.copyFailed'), 'error'); }
}

/** Plain text to the clipboard without any toast: navigator.clipboard, else a hidden textarea + execCommand. Resolves whether it worked. */
export async function writeClipboard(text: string): Promise<boolean> {
  try { await navigator.clipboard.writeText(text); return true; } catch { /* fall through to the legacy path */ }
  try {
    const ta = document.createElement('textarea');
    ta.value = text; ta.setAttribute('readonly', ''); ta.style.cssText = 'position:fixed;left:-9999px;top:0;opacity:0';
    document.body.append(ta); ta.select();
    const ok = document.execCommand('copy');
    ta.remove();
    return ok;
  } catch { return false; }
}
