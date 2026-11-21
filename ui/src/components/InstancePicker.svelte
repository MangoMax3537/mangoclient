<script lang="ts">
  import { untrack } from 'svelte';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ProhibitIcon from 'phosphor-svelte/lib/ProhibitIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import InfoLine from './InfoLine.svelte';
  import { versionLoaderFacts } from '../lib/infoLine';
  import InstanceIcon from './InstanceIcon.svelte';
  import LoadError from './LoadError.svelte';
  import DialogBodySkeleton from './skeleton/DialogBodySkeleton.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { nextIndex, preselect, REASON_KEY, selectAll, selectionSummary, targetRows, toggleSelection, type TargetRow } from '../lib/install';
  import { app, getApi, nav, prefs, selectedProfile, t, tp, type FlowProject } from '../lib/stores.svelte';
  import type { InstallTarget } from '../lib/types';

  interface Props {
    project: FlowProject;
    versionId?: string;
    onnext: (profileIds: string[]) => void;
    oncreate: () => void;
    onclose: () => void;
  }
  let { project, versionId, onnext, oncreate, onclose }: Props = $props();

  const tl = createLoadState<InstallTarget[]>({ isEmpty: () => false });
  const targets = $derived(tl.data);
  let selected = $state<string[]>([]);
  let focusIdx = $state(0);
  let listEl: HTMLUListElement | undefined = $state();

  const rows = $derived(targets ? targetRows(targets, app.profiles) : []);
  const sum = $derived(selectionSummary(rows, selected));

  $effect(() => {
    void project.projectId; void versionId;
    tl.run(() => getApi().installTargets({ projectId: project.projectId, ...(versionId ? { versionId } : {}) }));
    return () => tl.cancel();
  });
  $effect(() => {
    const l = tl.data;
    if (!l) return;
    untrack(() => {
      const open = nav.view === 'profile' ? nav.profileId : null;
      selected = preselect(targetRows(l, app.profiles), [open, prefs.lastInstallProfile, selectedProfile()?.id]);
      focusIdx = Math.max(0, targetRows(l, app.profiles).findIndex((r) => selected.includes(r.profile.id)));
    });
  });

  function reasonText(r: TargetRow): string {
    if (r.target.state === 'installed') return t('picker.installedV', { v: r.target.installedVersion ?? '?' });
    return r.target.reason ? t(REASON_KEY[r.target.reason]) : '';
  }
  function toggle(id: string) { selected = toggleSelection(selected, id, rows); }
  function focusRow(i: number) {
    focusIdx = i;
    queueMicrotask(() => (listEl?.querySelectorAll<HTMLElement>('[role="option"]')[i])?.focus());
  }
  function key(e: KeyboardEvent, i: number, r: TargetRow) {
    const n = nextIndex(i, e.key, rows.length);
    if (n !== null) { e.preventDefault(); focusRow(n); return; }
    if (e.key === ' ') { e.preventDefault(); toggle(r.profile.id); }
    else if (e.key === 'Enter') { e.preventDefault(); if (sum.count) onnext(selected); else toggle(r.profile.id); }
    else if ((e.key === 'a' || e.key === 'A') && (e.ctrlKey || e.metaKey)) { e.preventDefault(); selected = selectAll(rows); }
  }
</script>

<Modal title={t('picker.title', { name: project.title })} {onclose} size="lg" onenter={() => { if (sum.count) onnext(selected); }}>
  <div class="head">
    <Tile kind="mod" id={project.projectId} src={project.iconUrl} size={40} radius={8} />
    <p class="hint">{t('picker.hint')}</p>
  </div>

  {#if tl.phase === 'error'}
    <LoadError error={tl.error} onretry={() => tl.retry()} />
  {:else if targets === null}
    {#if tl.showSkeleton}<DialogBodySkeleton rows={Math.max(1, Math.min(8, app.profiles.length))} />{/if}
  {:else if rows.length === 0}
    <p class="hint">{t('picker.none')}</p>
  {:else}
    <ul class="list" role="listbox" aria-multiselectable="true" aria-label={t('picker.list')} bind:this={listEl}>
      {#each rows as r, i (r.profile.id)}
        {@const on = selected.includes(r.profile.id)}
        <li role="option" aria-selected={r.selectable ? on : undefined} aria-disabled={!r.selectable} tabindex={i === focusIdx ? 0 : -1}
          class="row" class:dark={r.dark} class:on class:inst={r.target.state === 'installed'}
          onclick={() => toggle(r.profile.id)} onkeydown={(e) => key(e, i, r)} onfocus={() => (focusIdx = i)}>
          <InstanceIcon profile={r.profile} size={44} notch={8} />
          <span class="who">
            <b class="nm">{r.profile.name}</b>
            <span class="mono facts2"><InfoLine facts={versionLoaderFacts(r.profile, t)} /></span>
          </span>
          {#if r.selectable}
            <span class="state"><span class="box" aria-hidden="true">{#if on}<CheckIcon weight="bold" size={14} />{/if}</span>
              {#if r.target.requiredDeps > 0}<span class="dep mono">{tp('picker.plusDeps', r.target.requiredDeps)}</span>{/if}</span>
          {:else}
            <span class="state why">
              {#if r.target.state === 'installed'}<CheckIcon weight="bold" size={14} aria-hidden="true" />{:else}<ProhibitIcon weight="bold" size={14} aria-hidden="true" />{/if}
              <span>{reasonText(r)}</span>
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if targets && rows.length}
    <p class="count mono" aria-live="polite">{tp('picker.selected', sum.count) + (sum.requiredDeps > 0 ? ' · ' + tp('picker.requiredDeps', sum.requiredDeps) : '')}</p>
  {/if}

  {#snippet footer()}
    <button class="btn ghost create" type="button" onclick={oncreate} disabled={!targets}><PlusIcon weight="bold" size={14} aria-hidden="true" />{t('picker.create')}</button>
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={sum.count === 0} onclick={() => onnext(selected)}>{t('common.continue')}</button>
  {/snippet}
</Modal>

<style>
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }
  .head p { margin: 0; }
  .list { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); overflow: hidden; }
  .row { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--line-soft); background: var(--ink-2); transition: background var(--dur) var(--ease); }
  .row:last-child { border-bottom: 0; }
  .row:hover { background: var(--ink-3); }
  .row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
  .row.on { background: color-mix(in srgb, var(--accent-fill) 12%, var(--ink-2)); }
  /* dark: not selectable. Image and text dimmed, the reason stays readable. */
  .row.dark { background: var(--ink-1); }
  .row.dark:hover { background: var(--ink-1); }
  .row.dark :global(.tile) { opacity: 0.4; filter: grayscale(1); }
  .row.dark .who { opacity: 0.6; }
  .who { display: grid; gap: 1px; min-width: 0; }
  .nm { font: 700 var(--fs-lg) var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .facts2 { font-size: var(--fs-sm); color: var(--paper-dim); }
  .state { display: inline-flex; align-items: center; gap: 10px; justify-content: flex-end; text-align: right; }
  .why { max-width: 250px; font-size: var(--fs-sm); color: var(--paper-dim); gap: 6px; }
  .inst .why { color: var(--leaf); }
  .box { width: 20px; height: 20px; display: grid; place-items: center; border: 1.5px solid var(--paper-mute); color: var(--on-fill); }
  .row.on .box { background: var(--accent-fill); border-color: var(--accent-fill); }
  .dep { font-size: var(--fs-xs); color: var(--paper-dim); }
  .count { margin: 10px 0 0; font-size: var(--fs-sm); color: var(--paper-dim); }
  .create { margin-right: auto; }
</style>
