<script lang="ts">
  // The "N files are blocked by CurseForge" note of a modpack plan with an expandable file list. Shared by the modpack and the import dialog.
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
  import CaretRightIcon from 'phosphor-svelte/lib/CaretRightIcon';
  import Tile from './Tile.svelte';
  import { planSummaryText } from '../lib/blocked';
  import type { Key } from '../lib/i18n';
  import { t, tp } from '../lib/stores.svelte';
  import type { BlockedFile } from '../lib/types';

  let { files, id = 'bp-blocked' }: { files: BlockedFile[]; id?: string } = $props();
  let show = $state(false);
</script>

{#if files.length}
  <div class="warn" role="note">
    <WarningIcon weight="bold" size={16} aria-hidden="true" />
    <div class="wb">
      <b>{planSummaryText((k, n) => tp(k, n), files)}</b>
      <p class="hint">{t('blocked.plan.note')}</p>
      <button class="btn small" type="button" aria-expanded={show} aria-controls={id} onclick={() => (show = !show)}>
        {#if show}<CaretDownIcon weight="bold" size={14} aria-hidden="true" />{:else}<CaretRightIcon weight="bold" size={14} aria-hidden="true" />{/if}{show ? t('blocked.plan.hide') : t('blocked.plan.show')}
      </button>
      {#if show}
        <ul {id} class="bl" aria-label={t('blocked.plan.listLabel')}>
          {#each files as f (f.fileName)}
            <li>
              <Tile kind="mod" id={f.fileName} src={null} size={28} radius={5} />
              <span class="mono nm" title={f.fileName}>{f.fileName}</span>
              <span class="st {f.state}">{t(`blocked.state.${f.state}` as Key)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/if}

<style>
  .warn { display: flex; gap: 10px; margin-top: 14px; padding: 10px 12px; border-left: 3px solid var(--mango); background: color-mix(in srgb, var(--mango) 9%, transparent); font-size: var(--fs-sm); }
  .warn > :global(svg) { flex: none; margin-top: 2px; color: var(--mango); }
  .warn p { margin: 2px 0 8px; }
  .wb { min-width: 0; flex: 1; }
  .wb > b { display: block; }
  .bl { list-style: none; margin: 10px 0 0; padding: 0; display: grid; gap: 0; border: 1px solid var(--line); border-radius: var(--r-s); background: var(--ink-1); }
  .bl li { display: grid; grid-template-columns: 28px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 5px 10px; border-bottom: 1px solid var(--line-soft); }
  .bl li:last-child { border-bottom: 0; }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .st { font-size: var(--fs-xs); font-weight: 600; white-space: nowrap; color: var(--mango); }
  .st.matched_modrinth { color: var(--sky); }
</style>
