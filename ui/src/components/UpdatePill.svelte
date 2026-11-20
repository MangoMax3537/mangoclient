<script lang="ts">
  import ArrowCircleUpIcon from 'phosphor-svelte/lib/ArrowCircleUpIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import { t, updates } from '../lib/stores.svelte';

  const u = $derived(updates.status);
  const pct = $derived(Math.round(u.percent ?? 0));
  const show = $derived(u.state === 'checking' || u.state === 'downloading' || u.state === 'ready');
</script>

{#if show}
  <button class="pill" class:ready={u.state === 'ready'} type="button" disabled={u.state !== 'ready'} onclick={() => (updates.overlay = true)}
    aria-label={u.state === 'ready' ? t('update.pill.ready', { v: u.version ?? '' }) : u.state === 'downloading' ? t('update.pill.downloading', { n: pct }) : t('update.pill.checking')}>
    {#if u.state === 'ready'}<ArrowCircleUpIcon weight="bold" size={14} aria-hidden="true" />{t('update.pill.ready', { v: u.version ?? '' })}
    {:else}<span class="spin"><CircleNotchIcon weight="bold" size={14} aria-hidden="true" /></span>{u.state === 'downloading' ? t('update.pill.downloading', { n: pct }) : t('update.pill.checking')}{/if}
  </button>
{/if}

<style>
  .pill { display: inline-flex; align-items: center; gap: 6px; height: 22px; padding: 0 10px; border-radius: 11px; border: 1px solid var(--line); color: var(--paper-dim); font-size: 12px; font-weight: 600; }
  .pill.ready { background: var(--mango); border-color: var(--mango); color: var(--mango-ink); }
  .pill.ready:hover { background: #ffbd57; }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
