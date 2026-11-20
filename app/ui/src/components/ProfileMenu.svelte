<script lang="ts">
  import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
  import Tile from './Tile.svelte';
  import { actions, app, t } from '../lib/stores.svelte';

  let { currentId, disabled = false }: { currentId: string; disabled?: boolean } = $props();
  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
  }
  function onDoc(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) open = false;
  }
  async function pick(id: string) {
    open = false;
    await actions.selectProfile(id);
  }
</script>

<svelte:document onclick={onDoc} />
<div class="menu" bind:this={root} onkeydown={onKey} role="presentation">
  <button class="btn" type="button" {disabled} aria-haspopup="listbox" aria-expanded={open} onclick={() => (open = !open)}>
    {t('home.switch')}<CaretDownIcon weight="bold" size={14} aria-hidden="true" />
  </button>
  {#if open}
    <ul class="list" role="listbox" aria-label={t('home.switch')}>
      {#each app.profiles as p (p.id)}
        <li role="option" aria-selected={p.id === currentId}>
          <button type="button" class:cur={p.id === currentId} onclick={() => pick(p.id)}>
            <Tile kind="profile" id={p.id} src={p.coverUrl} size={28} radius={4} />
            <span class="n">{p.name}</span>
            <span class="v mono">{p.mcVersion}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .menu { position: relative; }
  .list {
    position: absolute; bottom: calc(100% + 6px); left: 0; z-index: 20; min-width: 280px; margin: 0; padding: 4px; list-style: none;
    background: var(--ink-2); border: 1px solid var(--line); border-radius: var(--r-m); box-shadow: 0 12px 32px rgba(0, 0, 0, 0.55);
    max-height: 300px; overflow: auto;
  }
  li button { display: flex; align-items: center; gap: 10px; width: 100%; padding: 6px 8px; border-radius: var(--r-s); text-align: left; }
  li button:hover, li button.cur { background: var(--ink-4); }
  .n { flex: 1; font-weight: 500; }
  .v { color: var(--paper-mute); font-size: 12px; }
</style>
