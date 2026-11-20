// Clipboard text with a visible result: a toast on success, a toast on failure (no permission), never silent.
import { t, toast } from './stores.svelte';

export async function copyText(text: string, what = text): Promise<void> {
  try { await navigator.clipboard.writeText(text); toast(t('menu.copied', { what }), 'ok', 2500); }
  catch { toast(t('menu.copyFailed'), 'error'); }
}
