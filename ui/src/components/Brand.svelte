<script lang="ts">
  // The product name as a label or title: the real Mango Client icon next to the wordmark in --brand-gold. In running sentences write the
  // name as plain text instead.
  import Img from './Img.svelte';
  import { brandIcon } from '../lib/fallbackImage';
  import { t } from '../lib/stores.svelte';

  let { size = 'sm', icon = true, drag = false, class: cls = '' }: { size?: 'sm' | 'lg'; icon?: boolean; drag?: boolean; class?: string } = $props();
  const px = $derived(size === 'lg' ? 44 : 18);
</script>

<span class="brand {size} {cls}" class:drag data-tauri-drag-region={drag ? true : undefined}>
  {#if icon}<span class="ic px" style:width="{px}px" style:height="{px}px" aria-hidden="true"><Img src={brandIcon(size === 'lg' ? 128 : 48)} fallback={brandIcon(size === 'lg' ? 128 : 48)} fit="contain" pixel fbPad="0" /></span>{/if}
  <span class="wm">{t('app.name')}</span>
</span>

<style>
  .brand { display: inline-flex; align-items: center; gap: 0.5em; vertical-align: middle; font-family: var(--font-display); letter-spacing: -0.02em; white-space: nowrap; }
  .brand.sm { font-size: 14px; font-weight: 700; }
  .brand.lg { font-size: 30px; font-weight: 800; letter-spacing: -0.03em; gap: 12px; }
  .ic { flex: none; display: inline-block; }
  .drag > * { pointer-events: none; }
  .wm { color: var(--brand-gold); }
</style>
