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
  import { fmtBytes, fmtDateTime } from '../../lib/format';
  import { actions, app, errorText, getApi, isBusy, lang, t, toast, tp } from '../../lib/stores.svelte';
  import type { ProfileView, Screenshot } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
  let shots = $state<Screenshot[] | null>(null);
  let open = $state<number | null>(null);
  let confirmDel = $state(false);

  $effect(() => {
    const id = profile.id;
    let dead = false;
    shots = null; open = null;
    getApi().screenshotsList({ profileId: id }).then((l) => { if (!dead) shots = l; }, (e) => { if (!dead) { shots = []; toast(errorText(e), 'error'); } });
    return () => { dead = true; };
  });

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
  <header>
    <h2 class="eyebrow">{t('profile.tab.media')}</h2>
    {#if shots}<span class="hint mono">{tp('media.count', shots.length)}</span>{/if}
  </header>
  {#if shots === null}
    <ul class="grid">{#each Array(8) as _, i (i)}<li class="sk"></li>{/each}</ul>
  {:else if shots.length === 0}
    <EmptyState title={t('media.emptyTitle')} hint={t('media.empty')}>
      <button class="btn accent" type="button" disabled={!app.accounts.length || isBusy(profile.id)} onclick={() => actions.play(profile.id)}><PlayIcon weight="bold" size={16} aria-hidden="true" />{t('home.play')}</button>
    </EmptyState>
  {:else}
    <ul class="grid">
      {#each shots as s, i (s.name)}
        <li>
          <button type="button" onclick={() => (open = i)} aria-label={t('media.open', { name: s.name })}>
            <img src={s.url} alt="" loading="lazy" decoding="async" draggable="false" />
            <span class="cap mono">{fmtDateTime(s.taken, lang())}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#if cur}
  <Modal variant="lightbox" title={cur.name} onclose={() => (open = null)}>
    <div class="lb">
      <button class="nav" type="button" aria-label={t('media.prev')} onclick={() => step(-1)}><CaretLeftIcon weight="bold" size={24} aria-hidden="true" /></button>
      <figure>
        <img src={cur.url} alt={cur.name} draggable="false" />
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
  .grid img { width: 100%; height: 100%; object-fit: cover; display: block; transition: transform 200ms var(--ease); }
  .grid button:hover img { transform: scale(1.04); }
  .grid button:hover { border-color: var(--mango); }
  .cap { position: absolute; left: 0; right: 0; bottom: 0; padding: 14px 8px 5px; font-size: 12px; color: var(--paper); background: linear-gradient(0deg, rgba(0, 0, 0, 0.75), transparent); text-align: left; }
  .sk { aspect-ratio: 16 / 9; background: var(--ink-2); border-radius: var(--r-m); animation: sh 1.4s ease-in-out infinite; }
  @keyframes sh { 50% { opacity: 0.5; } }
  .lb { display: grid; grid-template-columns: 44px minmax(0, 1fr) 44px; align-items: center; gap: 8px; }
  .nav { height: 56px; border-radius: var(--r-m); display: grid; place-items: center; color: var(--paper-dim); }
  .nav:hover { background: var(--ink-3); color: var(--paper); }
  figure { margin: 0; display: grid; gap: 8px; justify-items: center; }
  figure img { width: 100%; max-height: calc(100vh - 300px); object-fit: contain; border-radius: var(--r-s); background: #000; }
  figcaption { font-size: 12.5px; color: var(--paper-mute); }
</style>
