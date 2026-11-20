<script lang="ts">
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import { detectOs } from '../lib/auth';
  import { auth, keystoreMissing, t } from '../lib/stores.svelte';
  import type { Key } from '../lib/i18n';

  const os = detectOs();
</script>

{#if keystoreMissing() && !auth.noticeDismissed}
  <div class="note" role="status">
    <WarningIcon weight="bold" size={18} aria-hidden="true" />
    <div class="txt">
      <b>{t('persist.title')}</b>
      <span>{t('persist.body')}</span>
      <span>{t(`persist.fix.${os}` as Key)}</span>
    </div>
    <button class="btn ghost x" type="button" aria-label={t('common.dismiss')} onclick={() => (auth.noticeDismissed = true)}><XIcon weight="bold" size={14} aria-hidden="true" /></button>
  </div>
{/if}

<style>
  .note { display: flex; gap: 10px; padding: 10px 6px 10px 12px; border: 1px solid var(--mango); background: rgba(255, 173, 50, 0.08); text-align: left; font-size: 13.5px; line-height: 1.4; }
  .note > :global(svg) { flex: none; color: var(--mango); margin-top: 1px; }
  .txt { display: grid; gap: 4px; flex: 1; min-width: 0; }
  .txt span { color: var(--paper-dim); }
  .x { width: 28px; height: 28px; padding: 0; justify-content: center; flex: none; }
</style>
