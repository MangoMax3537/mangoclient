<script lang="ts">
  import WarningCircleIcon from 'phosphor-svelte/lib/WarningCircleIcon';
  import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon';
  import type { LoginErrorInfo } from '../lib/auth';
  import { actions, t } from '../lib/stores.svelte';
  import type { Key } from '../lib/i18n';

  let { error }: { error: LoginErrorInfo } = $props();
  const site = $derived(error.help ? t(`login.site.${error.help.site}` as Key) : '');
</script>

<div class="le" role="alert">
  <WarningCircleIcon weight="bold" size={18} aria-hidden="true" />
  <div class="txt">
    <b>{t('login.err.title')}</b>
    <span>{t(`login.err.${error.kind}` as Key)}</span>
    {#if error.help}
      <button class="btn ghost help" type="button" aria-label={t('login.helpOpen', { site })} onclick={() => actions.openExternal(error.help!.url)}>
        {t('login.help')} ({site})<ArrowSquareOutIcon weight="bold" size={14} aria-hidden="true" />
      </button>
    {/if}
  </div>
</div>

<style>
  .le { display: flex; gap: 10px; padding: 10px 12px; border: 1px solid var(--tomato); background: rgba(237, 102, 92, 0.08); color: var(--paper); text-align: left; font-size: 13.5px; line-height: 1.4; }
  .le > :global(svg) { flex: none; color: var(--tomato); margin-top: 1px; }
  .txt { display: grid; gap: 3px; justify-items: start; min-width: 0; }
  .txt span { color: var(--paper-dim); }
  .help { height: 28px; padding: 0 8px; margin: 2px 0 0 -8px; color: var(--mango); font-size: 13px; }
  .help:hover:not(:disabled) { color: var(--mango); }
</style>
