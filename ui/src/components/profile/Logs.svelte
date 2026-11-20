<script lang="ts">
  import UploadSimpleIcon from 'phosphor-svelte/lib/UploadSimpleIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import BugIcon from 'phosphor-svelte/lib/BugIcon';
  import FirstAidKitIcon from 'phosphor-svelte/lib/FirstAidKitIcon';
  import EmptyState from '../EmptyState.svelte';
  import VirtualList from '../VirtualList.svelte';
  import ConfirmDialog from '../ConfirmDialog.svelte';
  import LoadError from '../LoadError.svelte';
  import ProfileTabSkeleton from '../skeleton/ProfileTabSkeleton.svelte';
  import LogLineSkeleton from '../skeleton/LogLineSkeleton.svelte';
  import { BusyGate, createLoadState } from '../../lib/loadState.svelte';
  import { fmtBytes, fmtRelative } from '../../lib/format';
  import { logLevel } from '../../lib/logs';
  import { analyzable } from '../../lib/portability';
  import { openCrash, openDiagnostics } from '../../lib/portability.svelte';
  import { actions, app, errorText, getApi, isBusy, lang, t, toast } from '../../lib/stores.svelte';
  import type { LogEntry, ProfileView } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
  let list = $state<LogEntry[] | null>(null);
  let sel = $state<string | null>(null);
  let text = $state<{ lines: string[]; truncated: boolean } | null>(null);
  let reading = $state(false);
  let uploading = $state(false);
  let link = $state<string | null>(null);
  let confirmDel = $state(false);

  const entry = $derived(list?.find((l) => l.id === sel) ?? null);

  const listLoad = createLoadState<LogEntry[]>({ isEmpty: () => false });
  $effect(() => {
    const id = profile.id;
    sel = null; text = null; link = null; list = null;
    listLoad.run(() => getApi().logsList({ profileId: id }));
    return () => listLoad.cancel();
  });
  $effect(() => {
    const l = listLoad.data;
    if (l) { list = l; sel = (l.find((x) => x.kind === 'crash') ?? l[0])?.id ?? null; }
  });
  // the log viewer: its own load (the file may be large), skeleton lines after the usual delay
  const readGate = new BusyGate();
  $effect(() => { readGate.update(reading || (!text && !!sel)); return () => readGate.destroy(); });
  $effect(() => {
    const id = sel;
    const pid = profile.id;
    link = null;
    if (!id) { text = null; return; }
    let dead = false;
    reading = true; text = null;
    getApi().logsRead({ profileId: pid, id }).then((r) => { if (!dead) { text = { lines: r.text.split('\n'), truncated: r.truncated }; reading = false; } },
      (e) => { if (!dead) { text = { lines: [], truncated: false }; reading = false; toast(errorText(e), 'error'); } });
    return () => { dead = true; };
  });

  async function upload() {
    if (!sel) return;
    uploading = true;
    try { link = (await getApi().logsUpload({ profileId: profile.id, id: sel })).url; } catch (e) { toast(errorText(e), 'error'); }
    uploading = false;
  }
  async function copy() {
    if (!link) return;
    try { await navigator.clipboard.writeText(link); toast(t('logs.linkCopied'), 'ok'); } catch { toast(t('err.unknown'), 'error'); }
  }
  async function del() {
    if (!sel || !list) return;
    const id = sel;
    try {
      await getApi().logsDelete({ profileId: profile.id, id });
      const rest = list.filter((l) => l.id !== id);
      list = rest; sel = rest[0]?.id ?? null;
      toast(t('logs.deleted'), 'ok');
    } catch (e) { toast(errorText(e), 'error'); }
    confirmDel = false;
  }
</script>

<div class="lg">
  {#if list === null}
    {#if listLoad.phase === 'error'}<LoadError error={listLoad.error} onretry={() => listLoad.retry()} />
    {:else if listLoad.showSkeleton}<ProfileTabSkeleton tab="logs" bare />{/if}
  {:else if list.length === 0}
    <EmptyState title={t('logs.emptyTitle')} hint={t('logs.empty')}>
      <button class="btn accent" type="button" disabled={!app.accounts.length || isBusy(profile.id)} onclick={() => actions.play(profile.id)}><PlayIcon weight="bold" size={16} aria-hidden="true" />{t('home.play')}</button>
    </EmptyState>
  {:else}
    <ul class="files" aria-label={t('profile.tab.logs')}>
      {#each list as l (l.id)}
        <li>
          <button type="button" class:on={l.id === sel} aria-pressed={l.id === sel} onclick={() => (sel = l.id)}>
            <span class="nm mono">{#if l.kind === 'crash'}<WarningIcon weight="bold" size={14} aria-hidden="true" />{/if}{l.name}</span>
            <span class="meta"><span class="k {l.kind}">{t(`logs.kind.${l.kind}`)}</span><span class="mono">{fmtBytes(l.size, lang())}</span><span>{fmtRelative(l.modified, lang())}</span></span>
          </button>
        </li>
      {/each}
    </ul>
    <section class="view" aria-label={entry?.name ?? ''}>
      <header>
        <b class="mono nm">{entry?.name}</b>
        <div class="acts">
          {#if entry && analyzable(entry)}
            <button class="btn" class:accent={entry.kind === 'crash'} type="button" title={t('crash.analyzeHint')} onclick={() => openCrash(profile.id, entry.id)}><BugIcon weight="bold" size={16} aria-hidden="true" />{t('crash.analyze')}</button>
          {/if}
          <button class="btn" type="button" title={t('diag.lead')} onclick={() => openDiagnostics(profile.id)}><FirstAidKitIcon weight="bold" size={16} aria-hidden="true" />{t('diag.button')}</button>
          <button class="btn" type="button" disabled={uploading || !entry} onclick={upload}><UploadSimpleIcon weight="bold" size={16} aria-hidden="true" />{uploading ? t('logs.uploading') : t('logs.upload')}</button>
          <button class="btn ghost danger" type="button" disabled={!entry} onclick={() => (confirmDel = true)} aria-label={t('logs.delete')} title={t('logs.delete')}><TrashIcon weight="bold" size={16} aria-hidden="true" /></button>
        </div>
      </header>
      {#if link}
        <div class="res" role="status">
          <span>{t('logs.uploaded')}</span>
          <input class="field mono" readonly value={link} aria-label={t('logs.link')} onfocus={(e) => e.currentTarget.select()} />
          <button class="btn" type="button" onclick={copy}><CopyIcon weight="bold" size={16} aria-hidden="true" />{t('logs.copyLink')}</button>
        </div>
      {/if}
      {#if text?.truncated}<div class="trunc hint">{t('logs.truncated')}</div>{/if}
      <div class="out" class:crash={entry?.kind === 'crash'}>
        {#if reading || !text}
          {#if readGate.show}<LogLineSkeleton count={16} lineHeight={20} pad="0 12px" />{/if}
        {:else}
          <VirtualList items={text.lines} rowHeight={20} key={(_, i) => i} label={entry?.name ?? ''} overscan={20}>
            {#snippet row(l)}<div class="ln mono {logLevel(l)}">{l || ' '}</div>{/snippet}
          </VirtualList>
        {/if}
      </div>
    </section>
  {/if}
</div>

{#if confirmDel && entry}
  <ConfirmDialog danger title={t('logs.deleteTitle')} body={t('logs.deleteBody', { name: entry.name })} confirmLabel={t('logs.delete')} onconfirm={del} oncancel={() => (confirmDel = false)} />
{/if}

<style>
  .lg { height: 100%; display: grid; grid-template-columns: 260px minmax(0, 1fr); gap: 14px; min-height: 0; }
  .files { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; align-content: start; overflow: auto; }
  .files button { width: 100%; text-align: left; display: grid; gap: 3px; padding: 8px 10px; border: 1px solid transparent; }
  .files button:hover { background: var(--ink-2); }
  .files button.on { background: var(--ink-3); border-color: var(--line); }
  .nm { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: flex; align-items: center; gap: 6px; }
  .meta { display: flex; gap: 8px; align-items: center; font-size: 12px; color: var(--paper-mute); }
  .k { padding: 0 6px; background: var(--ink-4); color: var(--paper-dim); }
  .k.crash { background: rgba(237, 102, 92, 0.2); color: var(--tomato); }
  .view { display: flex; flex-direction: column; min-width: 0; min-height: 0; gap: 8px; }
  header { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 12px; }
  .acts { display: flex; gap: 6px; flex-wrap: nowrap; margin-left: auto; }
  .res { display: flex; align-items: center; gap: 8px; padding: 8px 10px; background: rgba(134, 184, 107, 0.1); border: 1px solid rgba(134, 184, 107, 0.4); }
  .res .field { flex: 1; height: 32px; }
  .trunc { font-size: 13px; }
  .out { flex: 1; min-height: 0; background: var(--ink-1); border: 1px solid var(--line); overflow: hidden; }
  .ln { height: 20px; line-height: 20px; padding: 0 12px; font-size: 12.5px; white-space: pre; color: var(--paper-dim); user-select: text; }
  .ln.warn { color: var(--mango); }
  .ln.error { color: var(--tomato); }
</style>
