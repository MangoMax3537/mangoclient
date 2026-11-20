<script lang="ts">
  // Result of a file export (export / diagnostics): where it was written, how big it is and the `warnings` of the service in an expandable list.
  import CheckCircleIcon from 'phosphor-svelte/lib/CheckCircleIcon';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
  import CaretRightIcon from 'phosphor-svelte/lib/CaretRightIcon';
  import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
  import { fmtBytes } from '../lib/format';
  import { baseName } from '../lib/portability';
  import { lang, t, toast, tp } from '../lib/stores.svelte';
  import type { FileExport } from '../lib/types';

  let { result, title, line }: { result: FileExport; title: string; /** "Exported X (12 MB)." */ line: string } = $props();
  let open = $state(false);
  const warnings = $derived(result.warnings ?? []);
  async function copy() {
    try { await navigator.clipboard.writeText(result.path); toast(t('export.pathCopied'), 'ok'); } catch { toast(t('err.unknown'), 'error'); }
  }
</script>

<div class="res" role="status">
  <div class="hd">
    <CheckCircleIcon weight="bold" size={20} aria-hidden="true" />
    <div class="tx">
      <b>{title}</b>
      <span>{line}</span>
      <span class="mono path" title={result.path}>{t('export.saved', { path: result.path })}</span>
      <span class="hint mono">{baseName(result.path)} / {fmtBytes(result.sizeBytes, lang())}</span>
    </div>
    <button class="btn small" type="button" onclick={copy}><CopyIcon weight="bold" size={14} aria-hidden="true" />{t('export.copyPath')}</button>
  </div>
  {#if warnings.length}
    <div class="warn">
      <button class="tog" type="button" aria-expanded={open} aria-controls="fr-warn" onclick={() => (open = !open)}>
        <WarningIcon weight="bold" size={16} aria-hidden="true" />
        <b>{tp('export.warnings', warnings.length)}</b>
        {#if open}<CaretDownIcon weight="bold" size={14} aria-hidden="true" />{:else}<CaretRightIcon weight="bold" size={14} aria-hidden="true" />{/if}
      </button>
      {#if open}<ul id="fr-warn">{#each warnings as w, i (i)}<li>{w}</li>{/each}</ul>{/if}
    </div>
  {:else}
    <p class="hint none">{t('export.noWarnings')}</p>
  {/if}
</div>

<style>
  .res { display: grid; gap: 10px; }
  .hd { display: flex; align-items: flex-start; gap: 12px; padding: 12px; border: 1px solid color-mix(in srgb, var(--leaf) 55%, var(--line)); background: color-mix(in srgb, var(--leaf) 8%, transparent); }
  .hd > :global(svg) { flex: none; color: var(--leaf); margin-top: 2px; }
  .tx { display: grid; gap: 2px; min-width: 0; flex: 1; }
  .path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-sm); color: var(--paper-dim); }
  .warn { border: 1px solid color-mix(in srgb, var(--mango) 45%, transparent); background: color-mix(in srgb, var(--mango) 9%, transparent); font-size: var(--fs-sm); }
  .tog { display: flex; align-items: center; gap: 8px; width: 100%; padding: 8px 12px; text-align: left; }
  .tog :global(svg:first-child) { color: var(--mango); }
  .tog b { flex: 1; }
  ul { margin: 0; padding: 0 12px 10px 34px; display: grid; gap: 6px; }
  .none { margin: 0; }
</style>
