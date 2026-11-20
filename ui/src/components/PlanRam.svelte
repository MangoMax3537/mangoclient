<script lang="ts">
  // The memory line of a modpack / import plan: what the new instance will get, and the pack's cap when it names one.
  import MemoryIcon from 'phosphor-svelte/lib/MemoryIcon';
  import { fmtRam } from '../lib/format';
  import { planRam } from '../lib/ram';
  import { app, t } from '../lib/stores.svelte';
  import type { RamHint } from '../lib/types';

  let { plan }: { plan: { ram: RamHint | null; appliedRamMb: number | null } } = $props();
  const r = $derived(planRam(plan, app.config?.ram ?? app.defaultRamMb));
</script>

<p class="ramline" data-from={r.fromPack ? 'pack' : 'default'}>
  <MemoryIcon weight="bold" size={16} aria-hidden="true" />
  <span>{t(r.fromPack ? 'ram.plan.pack' : 'ram.plan.default', { ram: fmtRam(r.mb) })}{#if r.capMb !== null}{" "}<span class="cap">{t('ram.plan.cap', { ram: fmtRam(r.capMb) })}</span>{/if}</span>
</p>

<style>
  .ramline { display: flex; gap: 8px; align-items: flex-start; margin: 12px 0 0; font-size: 13.5px; color: var(--paper-dim); }
  .ramline :global(svg) { flex: none; margin-top: 2px; color: var(--mango); }
  .cap { color: var(--paper-mute); }
</style>
