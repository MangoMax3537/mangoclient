<script lang="ts">
  import { untrack } from 'svelte';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
  import CaretRightIcon from 'phosphor-svelte/lib/CaretRightIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import { fmtBytes } from '../lib/format';
  import { providerLabel } from '../lib/install';
  import { planSummaryText } from '../lib/blocked';
  import type { Key } from '../lib/i18n';
  import { errorText, getApi, installs, lang, startModpackRun, t, tp, type FlowProject, type InstallRun } from '../lib/stores.svelte';
  import type { ModpackPlan } from '../lib/types';

  interface Props { project: FlowProject; versionId?: string; onclose: () => void }
  let { project, versionId, onclose }: Props = $props();

  let plan = $state<ModpackPlan | null>(null);
  let error = $state('');
  let name = $state('');
  let run = $state<InstallRun | null>(null);
  let showBlocked = $state(false);

  async function load() {
    plan = null; error = ''; name = project.title;
    try {
      const p = await getApi().modpackPlan({ provider: project.provider, projectId: project.projectId, ...(versionId ? { versionId } : {}) });
      plan = p; name = p.name;
    } catch (e) { error = errorText(e); }
  }
  $effect(() => { void project.projectId; void versionId; untrack(() => void load()); });

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
      <span class="eyebrow">{providerLabel(project.provider)}</span>
      <h3>{project.title}</h3>
      <p class="hint">{t('flow.pack.logoHint')}</p>
    </div>
  </div>

  {#if error}
    <p class="err" role="alert">{error} <button class="btn small" type="button" onclick={load}>{t('common.retry')}</button></p>
  {:else if !plan}
    <div class="skl" aria-busy="true" aria-label={t('common.loading')}><span class="skel ln"></span><span class="skel ln"></span><span class="skel ln"></span></div>
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

    {#if plan.blockedFiles.length}
      <div class="warn" role="note">
        <WarningIcon weight="bold" size={16} aria-hidden="true" />
        <div class="wb">
          <b>{planSummaryText((k, n) => tp(k, n), plan.blockedFiles)}</b>
          <p class="hint">{t('blocked.plan.note')}</p>
          <button class="btn small" type="button" aria-expanded={showBlocked} aria-controls="mp-blocked" onclick={() => (showBlocked = !showBlocked)}>
            {#if showBlocked}<CaretDownIcon weight="bold" size={14} aria-hidden="true" />{:else}<CaretRightIcon weight="bold" size={14} aria-hidden="true" />{/if}{showBlocked ? t('blocked.plan.hide') : t('blocked.plan.show')}
          </button>
          {#if showBlocked}
            <ul id="mp-blocked" class="bl" aria-label={t('blocked.plan.listLabel')}>
              {#each plan.blockedFiles as f (f.fileName)}
                <li>
                  <Tile kind="mod" id={f.fileName} src={null} size={28} radius={5} />
                  <span class="mono nm" title={f.fileName}>{f.fileName}</span>
                  <span class="st {f.state}">{t(`blocked.state.${f.state}` as Key)}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </div>
    {/if}

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
  .warn { display: flex; gap: 10px; margin-top: 14px; padding: 10px 12px; border-left: 3px solid var(--mango); background: color-mix(in srgb, var(--mango) 9%, transparent); font-size: var(--fs-sm); }
  .warn > :global(svg) { flex: none; margin-top: 2px; color: var(--mango); }
  .warn p { margin: 2px 0 8px; }
  .wb { min-width: 0; flex: 1; }
  .wb > b { display: block; }
  .bl { list-style: none; margin: 10px 0 0; padding: 0; display: grid; gap: 0; border: 1px solid var(--line); border-radius: var(--r-s); background: var(--ink-1); }
  .bl li { display: grid; grid-template-columns: 28px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 5px 10px; border-bottom: 1px solid var(--line-soft); }
  .bl li:last-child { border-bottom: 0; }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .st { font-size: var(--fs-xs); font-weight: 600; white-space: nowrap; color: var(--mango); }
  .st.matched_modrinth { color: var(--sky); }
  .skl { display: grid; gap: 10px; }
  .ln { height: 14px; width: 60%; }
  .prog { display: flex; align-items: center; gap: 12px; margin-top: 14px; }
  .bar { flex: 1; height: 6px; background: var(--ink-4); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent-fill); transform-origin: left; transition: transform var(--dur) var(--ease); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
