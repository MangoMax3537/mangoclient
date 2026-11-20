<script lang="ts">
  // Renders a lazily loaded component (see lib/lazy.ts). While the chunk loads the `fallback` (a skeleton of the same size) is shown, so
  // nothing jumps; a failed load shows a retry instead of a blank screen.
  import { untrack, type Snippet } from 'svelte';
  import type { Component } from 'svelte';
  import type { Loader } from '../lib/lazy';
  import { t } from '../lib/stores.svelte';

  interface Props { load: Loader; props?: Record<string, unknown>; fallback?: Snippet }
  let { load, props = {}, fallback }: Props = $props();

  let C = $state.raw<Component<any> | null>(null); // eslint-disable-line @typescript-eslint/no-explicit-any
  let failed = $state(false);
  let seq = 0;
  function run() {
    const my = ++seq;
    failed = false; C = null;
    load().then((m) => { if (my === seq) C = m.default; }, () => { if (my === seq) failed = true; });
  }
  $effect(() => { void load; untrack(run); });
</script>

{#if C}
  <C {...props} />
{:else if failed}
  <div class="lz" role="alert">{t('lazy.failed')} <button class="btn small" type="button" onclick={run}>{t('common.retry')}</button></div>
{:else if fallback}
  {@render fallback()}
{/if}

<style>
  .lz { display: flex; align-items: center; gap: 10px; padding: 16px 20px; color: var(--paper-dim); }
</style>
