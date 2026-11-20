<script lang="ts">
  import { untrack } from 'svelte';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import ClockCounterClockwiseIcon from 'phosphor-svelte/lib/ClockCounterClockwiseIcon';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import LoadError from './LoadError.svelte';
  import DialogBodySkeleton from './skeleton/DialogBodySkeleton.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { updateRows, updateSteps, type RunStep, type UpdateRow } from '../lib/install';
  import { fmtBytes } from '../lib/format';
  import { app, getApi, lang, t, tp } from '../lib/stores.svelte';
  import type { InstallPlan, UpdateInfo } from '../lib/types';

  interface Props { profileId: string; infos: UpdateInfo[]; onconfirm: (steps: RunStep[]) => void; onclose: () => void }
  let { profileId, infos, onconfirm, onclose }: Props = $props();

  const rl = createLoadState<UpdateRow[]>({ isEmpty: () => false });
  const rows = $derived(rl.data);
  let skip = $state<string[]>([]);
  const profile = $derived(app.profiles.find((p) => p.id === profileId));

  $effect(() => {
    const list = infos;
    untrack(() => { skip = []; });
    rl.run(async () => {
      const plans = await Promise.all(list.map((i): Promise<InstallPlan | null> =>
        getApi().installPlan({ projectId: i.projectId, versionId: i.versionId, profileId }).catch(() => null)));
      return updateRows(list, plans);
    });
    return () => rl.cancel();
  });

  const active = $derived((rows ?? []).filter((r) => !skip.includes(r.info.projectId)));
  const bytes = $derived(active.reduce((a, r) => a + r.bytes, 0));
  const newDeps = $derived(active.reduce((a, r) => a + r.newDeps.length, 0));
  function confirm() {
    if (!rows || !active.length) return;
    onconfirm(updateSteps(profileId, active));
  }
  function flip(id: string, on: boolean) { skip = on ? skip.filter((x) => x !== id) : [...skip, id]; }
</script>

<Modal title={t('flow.upd.title', { name: profile?.name ?? '' })} {onclose} size="lg" onenter={confirm}>
  <p class="hint lead">{t('flow.upd.lead')}</p>
  {#if rl.phase === 'error'}
    <LoadError error={rl.error} onretry={() => rl.retry()} />
  {:else if rows === null}
    {#if rl.showSkeleton}<DialogBodySkeleton rows={4} img={36} />{/if}
  {:else}
    <ul class="rows">
      {#each rows as r (r.info.projectId)}
        {@const off = skip.includes(r.info.projectId)}
        <li class="row" class:off={off}>
          <Tile kind="mod" id={r.info.projectId} src={null} size={36} radius={6} />
          <div class="t">
            <div class="l1"><b>{r.info.title}</b></div>
            <div class="ver mono">{r.info.from ?? '?'}<ArrowRightIcon weight="bold" size={12} aria-hidden="true" />{r.info.to}</div>
            {#if r.newDeps.length}
              <div class="deps"><PlusIcon weight="bold" size={12} aria-hidden="true" /><span class="hint">{tp('flow.upd.newDeps', r.newDeps.length)}</span>
                {#each r.newDeps as d (d.projectId)}<span class="chip sm">{d.title}</span>{/each}</div>
            {/if}
            {#if r.conflicts.length}
              <div class="w"><WarningIcon weight="bold" size={13} aria-hidden="true" />{t('flow.upd.conflict', { list: r.conflicts.map((c) => c.title).join(', ') })}</div>
            {/if}
            {#if !r.plan}<div class="w"><WarningIcon weight="bold" size={13} aria-hidden="true" />{t('flow.upd.noPlan')}</div>{/if}
          </div>
          <span class="sz mono">{r.bytes ? fmtBytes(r.bytes, lang()) : ''}</span>
          <input type="checkbox" checked={!off} aria-label={t('flow.upd.include', { name: r.info.title })} onchange={(e) => flip(r.info.projectId, e.currentTarget.checked)} />
        </li>
      {/each}
    </ul>
    <p class="hint note">{t('flow.upd.removedNote')}</p>
    {#if app.config?.autoSnapshot}<p class="hint note snap"><ClockCounterClockwiseIcon weight="bold" size={14} aria-hidden="true" />{t('flow.snapshotFirst')}</p>{/if}
  {/if}

  {#snippet footer()}
    <span class="tot mono" aria-live="polite">{#if rows}{[tp('flow.upd.count', active.length), ...(newDeps ? [tp('flow.upd.newDeps', newDeps)] : []), fmtBytes(bytes, lang())].join(' · ')}{/if}</span>
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={!rows || !active.length} onclick={confirm}>
      {#if rows === null}<span class="spin"><CircleNotchIcon weight="bold" size={14} aria-hidden="true" /></span>{/if}{t('flow.upd.confirm')}
    </button>
  {/snippet}
</Modal>

<style>
  .lead { margin: 0 0 10px; }
  .rows { list-style: none; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r-m); }
  .row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto 20px; gap: 12px; align-items: start; padding: 10px 12px; border-bottom: 1px solid var(--line-soft); }
  .row:last-child { border-bottom: 0; }
  .row.off .t { opacity: 0.5; }
  .row > input { margin-top: 10px; }
  .row > .sz { margin-top: 9px; }
  .t { display: grid; gap: 3px; min-width: 0; }
  .l1 { display: flex; align-items: center; gap: 8px; }
  .l1 b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ver { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-sm); color: var(--paper-dim); }
  .ver :global(svg) { color: var(--mango); }
  .deps { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; font-size: var(--fs-sm); }
  .chip.sm { height: 20px; padding: 0 7px; font-size: var(--fs-xs); }
  .w { display: flex; gap: 6px; align-items: flex-start; font-size: var(--fs-sm); color: var(--mango); }
  .w :global(svg) { margin-top: 3px; flex: none; }
  .sz { font-size: var(--fs-sm); color: var(--paper-dim); white-space: nowrap; }
  .note { margin: 10px 0 0; }
  .note.snap { display: flex; gap: 6px; align-items: flex-start; margin-top: 4px; }
  .note.snap :global(svg) { flex: none; margin-top: 3px; }
  .tot { margin-right: auto; align-self: center; font-size: var(--fs-sm); color: var(--paper-dim); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
