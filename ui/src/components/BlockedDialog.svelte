<script lang="ts">
  // Prism-style "blocked mods" dialog (docs/ipc.md Part 5): lists the files CurseForge does not let us download,
  // watches the Downloads folder while open and reacts to blocked:changed.
  import { onMount } from 'svelte';
  import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon';
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import CheckCircleIcon from 'phosphor-svelte/lib/CheckCircleIcon';
  import SkipForwardIcon from 'phosphor-svelte/lib/SkipForwardIcon';
  import ClockIcon from 'phosphor-svelte/lib/ClockIcon';
  import ArrowsLeftRightIcon from 'phosphor-svelte/lib/ArrowsLeftRightIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { canFinishSilently, missingFiles, openAllNeedsConfirm, openPagesSequentially, sortBlocked, summarize } from '../lib/blocked';
  import type { Key } from '../lib/i18n';
  import { actions, app, blocked, errorText, getApi, loadBlocked, skipBlocked, startBlockedWatch, t, toast, tp } from '../lib/stores.svelte';
  import type { BlockedFile } from '../lib/types';

  let { profileId, onclose }: { profileId: string; onclose: () => void } = $props();

  const profile = $derived(app.profiles.find((p) => p.id === profileId) ?? null);
  const files = $derived(blocked.files[profileId] ?? []);
  const sum = $derived(summarize(files));
  const rows = $derived(sortBlocked(files));
  const dirs = $derived(blocked.watching[profileId]);
  let loaded = $state(false);
  let loadError = $state(false);
  let leaving = $state(false);
  let confirmOpenAll = $state(false);
  let opening = $state<{ done: number; total: number } | null>(null);
  let cancelled = false;
  let stop: (() => Promise<void>) | null = null;
  let closed = false;

  onMount(() => {
    void (async () => {
      try {
        await loadBlocked(profileId);
        const s = await startBlockedWatch(profileId);
        if (closed) void s(); else stop = s;
      } catch { loadError = true; }
      loaded = true;
    })();
    return () => { closed = true; cancelled = true; void stop?.(); };
  });

  const allFound = $derived(sum.manual > 0 && sum.pending === 0);
  const pct = $derived(sum.manual ? sum.resolved / sum.manual : 1);

  function close() { onclose(); }
  function done() { if (canFinishSilently(files)) close(); else leaving = true; }
  async function skip(f: BlockedFile) {
    try { await skipBlocked(profileId, f.fileName); } catch (e) { toast(errorText(e), 'error'); }
  }
  async function openFolder() {
    try { toast(t('blocked.folderOpened', { path: await getApi().blockedOpenFolder() }), 'info'); } catch (e) { toast(errorText(e), 'error'); }
  }
  function askOpenAll() {
    const n = missingFiles(files).length;
    if (!n || opening) return;
    if (openAllNeedsConfirm(n)) confirmOpenAll = true; else void openAll();
  }
  async function openAll() {
    confirmOpenAll = false;
    const urls = missingFiles(files).map((f) => f.url);
    cancelled = false;
    opening = { done: 0, total: urls.length };
    let n = 0;
    await openPagesSequentially(urls, async (u) => { await actions.openExternal(u); n++; if (opening) opening = { done: n, total: urls.length }; }, undefined, () => cancelled);
    opening = null;
  }
  const stateKey = (f: BlockedFile) => `blocked.state.${f.state}` as Key;
</script>

<Modal title={t('blocked.title', { name: profile?.name ?? '' })} onclose={close} size="lg" onenter={done}>
  <p class="lead">{t('blocked.lead')}</p>

  <div class="watch">
    <div class="wl">
      <span class="status" class:on={!!dirs} role="status">
        <span class="sq" class:ok={!!dirs} class:live={!!dirs && !allFound}></span>
        <b>{dirs ? t('blocked.watching') : loaded ? t('blocked.notWatching') : t('blocked.starting')}</b>
      </span>
      {#if dirs}
        <ul class="dirs" aria-label={t('blocked.watchedIn')}>
          {#each dirs as d (d)}<li class="mono" title={d}>{d}</li>{:else}<li class="hint">{t('blocked.noFolder')}</li>{/each}
        </ul>
      {/if}
    </div>
    <button class="btn small" type="button" onclick={openFolder}><FolderOpenIcon weight="bold" size={14} aria-hidden="true" />{t('blocked.openFolder')}</button>
  </div>

  {#if loadError}
    <p class="err" role="alert">{t('blocked.loadFailed')}</p>
  {:else}
    <div class="prog" role="status">
      <div class="pl">
        <b class="mono">{sum.manual === 0 ? t('blocked.progressNone') : allFound && sum.resolved === sum.manual ? t('blocked.progressAll', { total: sum.manual }) : t('blocked.progress', { done: sum.resolved, total: sum.manual })}</b>
        {#if sum.matched}<span class="hint">{tp('blocked.matchedNote', sum.matched)}</span>{/if}
      </div>
      <span class="bar" class:full={sum.manual > 0 && sum.resolved === sum.manual} role="progressbar" aria-label={t('blocked.title', { name: profile?.name ?? '' })} aria-valuemin="0" aria-valuemax={sum.manual} aria-valuenow={sum.resolved}><span style:transform="scaleX({pct})"></span></span>
    </div>

    {#if !loaded}
      <div class="skl" aria-busy="true" aria-label={t('common.loading')}>{#each Array(4) as _, i (i)}<span class="skel row-sk"></span>{/each}</div>
    {:else}
      <ul class="list" aria-label={t('blocked.listLabel')}>
        {#each rows as f (f.fileName)}
          <li class="row {f.state}">
            <Tile kind="mod" id={f.fileName} src={null} size={40} radius={6} />
            <div class="nm">
              <b>{f.name}</b>
              <span class="hint mono" title={f.fileName}>{f.fileName}</span>
            </div>
            <span class="st {f.state}">
              {#if f.state === 'installed' || f.state === 'found'}<CheckCircleIcon weight="bold" size={14} aria-hidden="true" />
              {:else if f.state === 'matched_modrinth'}<ArrowsLeftRightIcon weight="bold" size={14} aria-hidden="true" />
              {:else if f.state === 'skipped'}<SkipForwardIcon weight="bold" size={14} aria-hidden="true" />
              {:else}<ClockIcon weight="bold" size={14} aria-hidden="true" />{/if}
              {t(stateKey(f))}
            </span>
            <span class="acts">
              {#if f.state === 'pending'}
                <button class="btn small" type="button" aria-label={t('blocked.openPageFor', { name: f.name })} onclick={() => actions.openExternal(f.url)}><ArrowSquareOutIcon weight="bold" size={14} aria-hidden="true" />{t('blocked.openPage')}</button>
                <button class="btn small ghost" type="button" aria-label={t('blocked.skipFor', { name: f.name })} onclick={() => skip(f)}>{t('blocked.skip')}</button>
              {/if}
            </span>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <p class="how hint">{t('blocked.how')}</p>

  {#snippet footer()}
    {#if opening}
      <span class="hint mono grow" role="status">{t('blocked.openAll.progress', opening)}</span>
      <button class="btn ghost" type="button" onclick={() => (cancelled = true)}>{t('blocked.openAll.stop')}</button>
    {:else}
      <button class="btn grow-l" type="button" disabled={sum.pending === 0} onclick={askOpenAll}><ArrowSquareOutIcon weight="bold" size={14} aria-hidden="true" />{t('blocked.openAll')}{#if sum.pending}<span class="mono n">{sum.pending}</span>{/if}</button>
    {/if}
    <button class="btn accent" type="button" onclick={done}>{t('common.done')}</button>
  {/snippet}
</Modal>

{#if confirmOpenAll}
  <ConfirmDialog title={tp('blocked.openAll.title', sum.pending)} body={t('blocked.openAll.body')} confirmLabel={t('blocked.openAll.confirm')}
    onconfirm={openAll} oncancel={() => (confirmOpenAll = false)} />
{/if}
{#if leaving}
  <ConfirmDialog title={tp('blocked.leave.title', sum.pending)} body={t('blocked.leave.body')} items={missingFiles(files).map((f) => f.name)} confirmLabel={t('blocked.leave.confirm')} danger
    onconfirm={() => { leaving = false; close(); }} oncancel={() => (leaving = false)} />
{/if}

<style>
  .lead { margin: 0 0 12px; color: var(--paper-dim); }
  .watch { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-1); }
  .wl { display: grid; gap: 4px; min-width: 0; }
  .status { display: inline-flex; align-items: center; gap: 8px; }
  .status.on b { color: var(--leaf); }
  .live { animation: live 1.6s ease-in-out infinite; }
  @keyframes live { 50% { opacity: 0.3; } }
  .dirs { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; font-size: var(--fs-sm); color: var(--paper-dim); }
  .dirs li { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .prog { display: grid; gap: 6px; margin: 14px 0 10px; }
  .pl { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
  .bar { height: 6px; background: var(--ink-4); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent-fill); transform-origin: left; transition: transform var(--dur) var(--ease); }
  .bar.full span { background: var(--leaf); }
  .list { list-style: none; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; }
  .row { display: grid; grid-template-columns: 40px minmax(0, 1fr) auto auto; gap: 12px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--line-soft); }
  .row:last-child { border-bottom: 0; }
  .row.installed, .row.skipped, .row.matched_modrinth { background: var(--ink-1); }
  .row.skipped .nm { opacity: 0.6; }
  .nm { display: grid; min-width: 0; }
  .nm b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nm .hint { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-xs); }
  .st { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); font-weight: 600; color: var(--paper-dim); white-space: nowrap; }
  .st.pending { color: var(--mango); }
  .st.found, .st.installed { color: var(--leaf); }
  .st.matched_modrinth { color: var(--sky); }
  .acts { display: inline-flex; gap: 4px; justify-content: flex-end; min-width: 0; }
  .skl { display: grid; gap: 6px; }
  .row-sk { height: 56px; }
  .how { margin: 12px 0 0; }
  .grow { margin-right: auto; }
  .grow-l { margin-right: auto; }
  .n { margin-left: 6px; padding: 0 6px; background: var(--ink-4); border-radius: var(--r-s); font-size: var(--fs-xs); }
  @media (max-width: 760px) {
    .row { grid-template-columns: 40px minmax(0, 1fr) auto; }
    .acts { grid-column: 2 / -1; justify-content: flex-start; }
    .watch { flex-direction: column; align-items: stretch; }
  }
</style>
