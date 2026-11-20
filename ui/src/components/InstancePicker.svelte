<script lang="ts">
  import { untrack } from 'svelte';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ProhibitIcon from 'phosphor-svelte/lib/ProhibitIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import { allBlocked, nextIndex, preselect, REASON_KEY, selectAll, selectionSummary, targetRows, toggleSelection, type TargetRow } from '../lib/install';
  import { actions, app, errorText, getApi, nav, prefs, selectedProfile, t, tp, type FlowProject } from '../lib/stores.svelte';
  import type { InstallTarget } from '../lib/types';

  interface Props {
    project: FlowProject;
    versionId?: string;
    onnext: (profileIds: string[]) => void;
    oncreate: () => void;
    onclose: () => void;
  }
  let { project, versionId, onnext, oncreate, onclose }: Props = $props();

  let targets = $state<InstallTarget[] | null>(null);
  let error = $state('');
  let selected = $state<string[]>([]);
  let focusIdx = $state(0);
  let listEl: HTMLUListElement | undefined = $state();

  const rows = $derived(targets ? targetRows(targets, app.profiles) : []);
  const sum = $derived(selectionSummary(rows, selected));
  const blocked = $derived(allBlocked(rows));

  async function load() {
    targets = null; error = '';
    try {
      const l = await getApi().installTargets({ provider: project.provider, projectId: project.projectId, ...(versionId ? { versionId } : {}) });
      targets = l;
      const open = nav.view === 'profile' ? nav.profileId : null;
      selected = preselect(targetRows(l, app.profiles), [open, prefs.lastInstallProfile, selectedProfile()?.id]);
      focusIdx = Math.max(0, targetRows(l, app.profiles).findIndex((r) => selected.includes(r.profile.id)));
    } catch (e) { error = errorText(e); }
  }
  $effect(() => { void project.projectId; void versionId; untrack(() => void load()); });

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
  async function openWebsite() {
    const first = rows[0]?.profile.id;
    if (!first) return;
    try {
      const plan = await getApi().installPlan({ provider: project.provider, projectId: project.projectId, profileId: first, ...(versionId ? { versionId } : {}) });
      const url = plan.items.find((i) => i.role === 'requested')?.downloadUrl;
      if (url) await actions.openExternal(url);
    } catch { /* the reason is already visible in the list */ }
  }
</script>

<Modal title={t('picker.title', { name: project.title })} {onclose} size="lg" onenter={() => { if (sum.count) onnext(selected); }}>
  <div class="head">
    <Tile kind="mod" id={project.projectId} src={project.iconUrl} size={40} radius={8} />
    <p class="hint">{t('picker.hint')}</p>
  </div>

  {#if error}
    <p class="err" role="alert">{error} <button class="btn small" type="button" onclick={load}>{t('common.retry')}</button></p>
  {:else if targets === null}
    <ul class="list" aria-busy="true" aria-label={t('common.loading')}>{#each Array(4) as _, i (i)}<li class="row sk"><span class="img notch skel" style:--n="8px"></span><span class="line skel"></span></li>{/each}</ul>
  {:else if rows.length === 0}
    <p class="hint">{t('picker.none')}</p>
  {:else}
    {#if blocked}
      <p class="note warn" role="note"><ProhibitIcon weight="bold" size={16} aria-hidden="true" /><span>{t('picker.blockedAll')}</span>
        <button class="btn small" type="button" onclick={openWebsite}><ArrowSquareOutIcon weight="bold" size={14} aria-hidden="true" />{t('flow.openWebsite')}</button></p>
    {/if}
    <ul class="list" role="listbox" aria-multiselectable="true" aria-label={t('picker.list')} bind:this={listEl}>
      {#each rows as r, i (r.profile.id)}
        {@const on = selected.includes(r.profile.id)}
        <li role="option" aria-selected={r.selectable ? on : undefined} aria-disabled={!r.selectable} tabindex={i === focusIdx ? 0 : -1}
          class="row" class:dark={r.dark} class:on class:inst={r.target.state === 'installed'}
          onclick={() => toggle(r.profile.id)} onkeydown={(e) => key(e, i, r)} onfocus={() => (focusIdx = i)}>
          <Tile kind="profile" id={r.profile.id} src={r.profile.coverUrl} tint={r.profile.color} size={44} notch={8} />
          <span class="who">
            <b class="nm">{r.profile.name}</b>
            <span class="mono facts2">{r.profile.mcVersion}<span class="sep">/</span>{r.profile.loader === 'vanilla' ? t('profile.vanilla') : r.profile.loader}</span>
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
  .list { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; }
  .row { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 12px; align-items: center; padding: 8px 12px; border-bottom: 1px solid var(--line-soft); cursor: pointer; background: var(--ink-2); transition: background var(--dur) var(--ease); }
  .row:last-child { border-bottom: 0; }
  .row:hover { background: var(--ink-3); }
  .row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
  .row.on { background: color-mix(in srgb, var(--accent-fill) 12%, var(--ink-2)); box-shadow: inset 3px 0 var(--accent-fill); }
  /* dark: not selectable. Image and text dimmed, the reason stays readable. */
  .row.dark { cursor: not-allowed; background: var(--ink-1); }
  .row.dark:hover { background: var(--ink-1); }
  .row.dark :global(.tile) { opacity: 0.4; filter: grayscale(1); }
  .row.dark .who { opacity: 0.6; }
  .who { display: grid; gap: 1px; min-width: 0; }
  .nm { font: 700 var(--fs-lg) var(--font-display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .facts2 { font-size: var(--fs-sm); color: var(--paper-dim); }
  .sep { margin: 0 6px; opacity: 0.5; }
  .state { display: inline-flex; align-items: center; gap: 10px; justify-content: flex-end; text-align: right; }
  .why { max-width: 250px; font-size: var(--fs-sm); color: var(--paper-dim); gap: 6px; }
  .inst .why { color: var(--leaf); }
  .box { width: 20px; height: 20px; display: grid; place-items: center; border: 1.5px solid var(--paper-mute); border-radius: 4px; color: var(--on-fill); }
  .row.on .box { background: var(--accent-fill); border-color: var(--accent-fill); }
  .dep { font-size: var(--fs-xs); color: var(--paper-dim); }
  .note { display: flex; align-items: center; gap: 10px; margin: 0 0 10px; padding: 8px 10px 8px 12px; border-left: 3px solid var(--mango); background: color-mix(in srgb, var(--mango) 9%, transparent); font-size: var(--fs-sm); }
  .note span { flex: 1; }
  .sk { cursor: default; }
  .sk .img { width: 44px; height: 44px; }
  .sk .line { height: 14px; width: 55%; }
  .count { margin: 10px 0 0; font-size: var(--fs-sm); color: var(--paper-dim); }
  .create { margin-right: auto; }
</style>
