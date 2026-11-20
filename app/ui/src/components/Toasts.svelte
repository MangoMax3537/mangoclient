<script lang="ts">
  import CheckCircleIcon from 'phosphor-svelte/lib/CheckCircleIcon';
  import WarningCircleIcon from 'phosphor-svelte/lib/WarningCircleIcon';
  import InfoIcon from 'phosphor-svelte/lib/InfoIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import { dismissToast, t, toasts } from '../lib/stores.svelte';
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts as x (x.id)}
    <div class="toast {x.kind}">
      {#if x.kind === 'ok'}<CheckCircleIcon weight="bold" size={18} aria-hidden="true" />
      {:else if x.kind === 'error'}<WarningCircleIcon weight="bold" size={18} aria-hidden="true" />
      {:else}<InfoIcon weight="bold" size={18} aria-hidden="true" />{/if}
      <span>{x.text}</span>
      <button class="btn ghost" type="button" aria-label={t('common.dismiss')} onclick={() => dismissToast(x.id)}><XIcon weight="bold" size={14} aria-hidden="true" /></button>
    </div>
  {/each}
</div>

<style>
  .toasts { position: fixed; z-index: 100; left: 88px; bottom: 16px; display: grid; gap: 8px; max-width: min(460px, calc(100vw - 112px)); pointer-events: none; }
  .toast { pointer-events: auto; display: flex; align-items: center; gap: 10px; padding: 8px 6px 8px 14px; background: var(--ink-3); border: 1px solid var(--line); border-left: 3px solid var(--paper-dim); border-radius: var(--r-m); box-shadow: 0 10px 28px rgba(0, 0, 0, 0.5); animation: in 180ms var(--ease); }
  .toast span { flex: 1; font-size: 14px; }
  .toast.ok { border-left-color: var(--leaf); }
  .toast.ok :global(svg) { color: var(--leaf); }
  .toast.error { border-left-color: var(--tomato); }
  .toast.error :global(svg) { color: var(--tomato); }
  .toast .btn { width: 28px; height: 28px; padding: 0; justify-content: center; }
  @keyframes in { from { opacity: 0; transform: translateY(8px); } }
</style>
