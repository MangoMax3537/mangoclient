<script lang="ts">
  import CaretLeftIcon from 'phosphor-svelte/lib/CaretLeftIcon';
  import CaretRightIcon from 'phosphor-svelte/lib/CaretRightIcon';
  import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import EmptyState from '../EmptyState.svelte';
  import Modal from '../Modal.svelte';
  import ConfirmDialog from '../ConfirmDialog.svelte';
  import Worlds from './Worlds.svelte';
  import Img from '../Img.svelte';
  import LoadError from '../LoadError.svelte';
  import ScreenshotGridSkeleton from '../skeleton/ScreenshotGridSkeleton.svelte';
  import { createLoadState } from '../../lib/loadState.svelte';
  import { wb } from '../../lib/worlds.svelte';
  import { fallbackImage } from '../../lib/fallbackImage';
  import { fmtBytes, fmtDateTime } from '../../lib/format';
  import { actions, app, errorText, getApi, isBusy, lang, t, toast, tp } from '../../lib/stores.svelte';
  import type { ProfileView, Screenshot } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
  let shots = $state<Screenshot[] | null>(null);
  const shotsLoad = createLoadState<Screenshot[]>({ isEmpty: () => false });
  let open = $state<number | null>(null);
  let confirmDel = $state(false);

  $effect(() => {
    const id = profile.id;
    open = null; shots = null;
    shotsLoad.run(() => getApi().screenshotsList({ profileId: id }));
    return () => shotsLoad.cancel();
  });
  $effect(() => { if (shotsLoad.data) shots = shotsLoad.data; });

  const noWorlds = $derived(wb.worlds[profile.id]?.length === 0);
  const noShots = $derived(shots !== null && shots.length === 0);
  const cur = $derived(shots && open !== null ? shots[open] ?? null : null);
  function step(d: number) {
    if (!shots?.length || open === null) return;
    open = (open + d + shots.length) % shots.length;
  }
  function onKey(e: KeyboardEvent) {
    if (open === null || confirmDel) return;
    if (e.key === 'ArrowRight') { e.preventDefault(); step(1); }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); step(-1); }
  }
  async function act(action: 'copy' | 'reveal') {
    if (!cur) return;
    try { await getApi().screenshotAction({ profileId: profile.id, name: cur.name, action }); toast(t(action === 'copy' ? 'media.copied' : 'media.revealed'), 'ok'); }
    catch (e) { toast(errorText(e), 'error'); }
  }
  async function del() {
    if (!cur || !shots) return;
    const name = cur.name;
    try {
      await getApi().screenshotAction({ profileId: profile.id, name, action: 'delete' });
      const i = open ?? 0;
      shots = shots.filter((s) => s.name !== name);
      open = shots.length ? Math.min(i, shots.length - 1) : null;
      toast(t('media.deleted'), 'ok');
    } catch (e) { toast(errorText(e), 'error'); }
    confirmDel = false;
  }
</script>

<svelte:window onkeydown={onKey} />
<div class="md">
  {#if noWorlds && noShots}
    <!-- neither worlds nor screenshots: one empty state, not two stacked empty sections -->
    <EmptyState title={t('media.bothEmptyTitle')} hint={t('media.bothEmpty')}>
      <button class="btn accent" type="button" disabled={!app.accounts.length || isBusy(profile.id)} onclick={() => actions.play(profile.id)}><PlayIcon weight="bold" size={16} aria-hidden="true" />{t('home.play')}</button>
    </EmptyState>
  {:else}
    <Worlds {profile} hideWhenEmpty />
    {#if !noShots}
      <header>
        <h2 class="eyebrow">{t('media.title')}</h2>
        {#if shots}<span class="hint mono">{tp('media.count', shots.length)}</span>{/if}
      </header>
    {/if}
    {#if shots === null}
      {#if shotsLoad.phase === 'error'}<LoadError error={shotsLoad.error} onretry={() => shotsLoad.retry()} />
      {:else if shotsLoad.showSkeleton}<ScreenshotGridSkeleton count={8} />{/if}
    {:else if shots.length > 0}
    <ul class="grid">
      {#each shots as s, i (s.name)}
        <li>
          <button type="button" onclick={() => (open = i)} aria-label={t('media.open', { name: s.name })}>
            <Img src={s.url} lazy fallback={fallbackImage('world', s.name, 420)} />
            <span class="cap mono">{fmtDateTime(s.taken, lang())}</span>
          </button>
        </li>
      {/each}
    </ul>
    {/if}
  {/if}
</div>

{#if cur}
  <Modal variant="lightbox" title={cur.name} onclose={() => (open = null)}>
    <div class="lb">
      <button class="nav" type="button" aria-label={t('media.prev')} onclick={() => step(-1)}><CaretLeftIcon weight="bold" size={24} aria-hidden="true" /></button>
      <figure>
        <div class="shot"><Img src={cur.url} alt={cur.name} fit="contain" fallback={fallbackImage('world', cur.name, 512)} /></div>
        <figcaption class="mono">{open !== null ? open + 1 : 0} / {shots?.length} &middot; {fmtDateTime(cur.taken, lang())} &middot; {fmtBytes(cur.size, lang())}</figcaption>
      </figure>
      <button class="nav" type="button" aria-label={t('media.next')} onclick={() => step(1)}><CaretRightIcon weight="bold" size={24} aria-hidden="true" /></button>
    </div>
    {#snippet footer()}
      <button class="btn" type="button" onclick={() => act('copy')}><CopyIcon weight="bold" size={16} aria-hidden="true" />{t('media.copy')}</button>
      <button class="btn" type="button" onclick={() => act('reveal')}><FolderOpenIcon weight="bold" size={16} aria-hidden="true" />{t('media.reveal')}</button>
      <button class="btn danger" type="button" onclick={() => (confirmDel = true)}><TrashIcon weight="bold" size={16} aria-hidden="true" />{t('media.delete')}</button>
    {/snippet}
  </Modal>
{/if}
{#if confirmDel && cur}
  <ConfirmDialog danger title={t('media.deleteTitle')} body={t('media.deleteBody', { name: cur.name })} confirmLabel={t('media.delete')} onconfirm={del} oncancel={() => (confirmDel = false)} />
{/if}

<style>
  header { display: flex; align-items: baseline; justify-content: space-between; margin-bottom: 10px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 10px; list-style: none; margin: 0; padding: 0; }
  .grid button { position: relative; display: block; width: 100%; aspect-ratio: 16 / 9; border-radius: var(--r-m); overflow: hidden; border: 1px solid var(--line); background: var(--ink-2); }
  .grid button :global(.im) { transition: transform 200ms var(--ease); }
  .grid button:hover :global(.im) { transform: scale(1.04); }
  .grid button:hover { border-color: var(--mango); }
  .cap { position: absolute; left: 0; right: 0; bottom: 0; padding: 14px 8px 5px; font-size: 12px; color: var(--paper); background: linear-gradient(0deg, rgba(0, 0, 0, 0.75), transparent); text-align: left; }
  .lb { display: grid; grid-template-columns: 44px minmax(0, 1fr) 44px; align-items: center; gap: 8px; }
  .nav { height: 56px; border-radius: var(--r-m); display: grid; place-items: center; color: var(--paper-dim); }
  .nav:hover { background: var(--ink-3); color: var(--paper); }
  figure { margin: 0; display: grid; gap: 8px; justify-items: center; }
  .shot { width: 100%; aspect-ratio: 16 / 9; max-height: calc(100vh - 300px); border-radius: var(--r-s); background: var(--ink-1); overflow: hidden; }
  figcaption { font-size: 12.5px; color: var(--paper-mute); }
</style>
