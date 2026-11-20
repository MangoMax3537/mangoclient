<script lang="ts">
  import { untrack } from 'svelte';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import LoadError from './LoadError.svelte';
  import DialogBodySkeleton from './skeleton/DialogBodySkeleton.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import PlanRam from './PlanRam.svelte';
  import { fmtBytes } from '../lib/format';
  import { errorText, getApi, installs, lang, startModpackRun, t, tp, type FlowProject, type InstallRun } from '../lib/stores.svelte';
  import type { ModpackPlan } from '../lib/types';

  interface Props { project: FlowProject; versionId?: string; onclose: () => void }
  let { project, versionId, onclose }: Props = $props();

  const pLoad = createLoadState<ModpackPlan>({ isEmpty: () => false });
  const plan = $derived(pLoad.data);
  let name = $state('');
  let run = $state<InstallRun | null>(null);

  $effect(() => {
    void project.projectId; void versionId;
    untrack(() => { name = project.title; });
    pLoad.run(() => getApi().modpackPlan({ projectId: project.projectId, ...(versionId ? { versionId } : {}) }));
    return () => pLoad.cancel();
  });
  $effect(() => { const p = pLoad.data; if (p) untrack(() => { name = p.name; }); });

  const nameOk = $derived(name.trim().length > 0 && name.trim().length <= 48);
  const running = $derived(!!run && !run.finished);
  const pct = $derived.by(() => { const e = installs[project.projectId]; return e && e.total ? Math.round((e.done / e.total) * 100) : 0; });

  async function create() {
    if (!plan || !nameOk || run) return;
    const r = await startModpackRun(project, name.trim(), plan.versionId, true);
    run = r.run;
  }
  function close() { if (run) run.watching = false; onclose(); }
</script>

<Modal title={t('flow.pack.title', { name: project.title })} onclose={close} size="lg" onenter={running ? undefined : create}>
  <div class="top">
    <Tile kind="mod" id={project.projectId} src={plan?.iconUrl ?? project.iconUrl} size={64} radius={10} />
    <div class="hd">
      <h3>{project.title}</h3>
      <p class="hint">{t('flow.pack.logoHint')}</p>
    </div>
  </div>

  {#if pLoad.phase === 'error'}
    <LoadError error={pLoad.error} onretry={() => pLoad.retry()} />
  {:else if !plan}
    {#if pLoad.showSkeleton}<DialogBodySkeleton rows={3} img={0} framed={false} />{/if}
  {:else}
    <label class="lbl" for="mp-name">{t('flow.pack.name')}</label>
    <input id="mp-name" class="field" bind:value={name} maxlength="48" autocomplete="off" aria-invalid={!nameOk} disabled={!!run} />
    {#if !nameOk}<div class="err" role="alert">{t('new.nameRequired')}</div>{/if}

    <dl class="facts">
      <div><dt>{t('col.version')}</dt><dd class="mono">{plan.mcVersion}</dd></div>
      <div><dt>{t('col.loader')}</dt><dd class="mono">{plan.loader} {plan.loaderVersion ?? ''}</dd></div>
      <div><dt>{t('flow.pack.files')}</dt><dd class="mono">{plan.fileCount}</dd></div>
      <div><dt>{t('flow.pack.size')}</dt><dd class="mono">{plan.sizeBytes ? fmtBytes(plan.sizeBytes, lang()) : '-'}</dd></div>
    </dl>

    <PlanRam {plan} />

    {#if run}
      <div class="prog" role="status">
        <span class="bar" role="progressbar" aria-label={t('flow.pack.installing')} aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct}><span style:transform="scaleX({pct / 100})"></span></span>
        <span class="hint mono">{pct}%</span>
      </div>
      <p class="hint">{t('flow.run.background')}</p>
    {/if}
  {/if}

  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={close}>{running ? t('flow.run.hide') : t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={!plan || !nameOk || !!run} onclick={create}>
      {#if running}<span class="spin"><CircleNotchIcon weight="bold" size={14} aria-hidden="true" /></span>{t('flow.pack.installing')}{:else}{t('flow.pack.create')}{/if}
    </button>
  {/snippet}
</Modal>

<style>
  .top { display: flex; gap: 14px; align-items: center; margin-bottom: 14px; }
  h3 { font-size: var(--fs-xl); }
  .lbl { display: block; font-size: var(--fs-sm); font-weight: 600; color: var(--paper-dim); margin-bottom: 4px; }
  .field { width: 100%; }
  .facts { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin: 14px 0 0; border: 1px solid var(--line); border-radius: var(--r-m); }
  .facts div { display: flex; justify-content: space-between; gap: 12px; padding: 8px 12px; border-bottom: 1px solid var(--line-soft); }
  .facts div:nth-child(odd) { border-right: 1px solid var(--line-soft); }
  .facts div:nth-last-child(-n + 2) { border-bottom: 0; }
  dt { color: var(--paper-dim); }
  dd { margin: 0; }
  .prog { display: flex; align-items: center; gap: 12px; margin-top: 14px; }
  .bar { flex: 1; height: 6px; background: var(--ink-4); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent-fill); transform-origin: left; transition: transform var(--dur) var(--ease); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
