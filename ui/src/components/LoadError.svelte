<script lang="ts">
  // What a skeleton turns into when its load failed or timed out: a localized message and a Retry button.
  import { errorText, t } from '../lib/stores.svelte';
  import type { AppError } from '../lib/types';
  let { error, onretry, compact = false }: { error: AppError | null; onretry: () => void; compact?: boolean } = $props();
  const timeout = $derived(error?.code === 'net.timeout');
</script>

<div class="le" class:compact role="alert">
  <div class="tx">
    <b>{timeout ? t('load.timeout') : t('load.failed')}</b>
    <span class="hint">{errorText(error)}</span>
  </div>
  <button class="btn" type="button" onclick={onretry}>{t('common.retry')}</button>
</div>

<style>
  .le { display: flex; align-items: center; gap: var(--s-4); flex-wrap: wrap; padding: var(--s-5) var(--s-2); }
  .le.compact { padding: var(--s-3) 0; }
  .tx { display: grid; gap: 2px; min-width: 0; }
  b { font: 700 var(--fs-lg) var(--font-display); }
</style>
