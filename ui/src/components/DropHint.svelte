<script lang="ts">
  // Shown while a file is dragged over the window: tells what dropping does (modpack import).
  import DownloadSimpleIcon from 'phosphor-svelte/lib/DownloadSimpleIcon';
  import { t } from '../lib/stores.svelte';
</script>

<div class="drop" role="presentation" aria-hidden="true">
  <div class="card notch frame" style:--n="18px">
    <DownloadSimpleIcon weight="bold" size={40} aria-hidden="true" />
    <b>{t('import.drop.title')}</b>
    <span class="mono">{t('import.drop.hint')}</span>
  </div>
</div>

<style>
  .drop { position: fixed; inset: 0; z-index: 90; display: grid; place-items: center; background: var(--backdrop); pointer-events: none; animation: in var(--dur) var(--ease); }
  .card { display: grid; justify-items: center; gap: 8px; padding: 36px 56px; background: var(--ink-2); --frame: var(--accent-fill); --bw: 2px; text-align: center; }
  .card :global(svg) { color: var(--accent-fill); }
  b { font: 700 var(--fs-2xl) var(--font-display); }
  .mono { color: var(--paper-dim); font-size: var(--fs-sm); }
  @keyframes in { from { opacity: 0; } }
</style>
