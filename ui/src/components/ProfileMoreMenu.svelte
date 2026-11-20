<script lang="ts">
  // "More" menu of the profile page header: export as .mrpack, export diagnostics, analyze the latest crash.
  import DotsThreeIcon from 'phosphor-svelte/lib/DotsThreeIcon';
  import PackageIcon from 'phosphor-svelte/lib/PackageIcon';
  import FirstAidKitIcon from 'phosphor-svelte/lib/FirstAidKitIcon';
  import BugIcon from 'phosphor-svelte/lib/BugIcon';
  import { openCrash, openDiagnostics, openExport } from '../lib/portability.svelte';
  import { t } from '../lib/stores.svelte';

  let { profileId, name }: { profileId: string; name: string } = $props();
  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();
  function run(fn: (id: string) => void) { open = false; fn(profileId); }
  function onDoc(e: MouseEvent) { if (open && root && !root.contains(e.target as Node)) open = false; }
  function onKey(e: KeyboardEvent) { if (open && e.key === 'Escape') { open = false; (root?.querySelector('button') as HTMLElement | null)?.focus(); } }
</script>

<svelte:document onclick={onDoc} onkeydown={onKey} />
<div class="more" bind:this={root}>
  <button class="btn trig" type="button" aria-haspopup="menu" aria-expanded={open} aria-label={t('library.menu', { name })} onclick={() => (open = !open)}><DotsThreeIcon weight="bold" size={18} aria-hidden="true" /></button>
  {#if open}
    <ul class="pop menu-list" role="menu" aria-label={t('library.menu', { name })}>
      <li role="none"><button type="button" role="menuitem" onclick={() => run(openExport)}><PackageIcon weight="bold" size={16} aria-hidden="true" />{t('export.menu')}</button></li>
      <li role="none"><button type="button" role="menuitem" onclick={() => run((id) => openCrash(id))}><BugIcon weight="bold" size={16} aria-hidden="true" />{t('crash.title')}</button></li>
      <li role="none"><button type="button" role="menuitem" onclick={() => run(openDiagnostics)}><FirstAidKitIcon weight="bold" size={16} aria-hidden="true" />{t('diag.button')}</button></li>
    </ul>
  {/if}
</div>

<style>
  .more { position: relative; }
  .trig { width: 34px; height: 30px; padding: 0; justify-content: center; background: rgba(var(--scrim), 0.55); border-color: var(--hero-line); color: #f4f1ea; }
  .pop { position: absolute; right: 0; top: 34px; min-width: 210px; z-index: 5; }
</style>
