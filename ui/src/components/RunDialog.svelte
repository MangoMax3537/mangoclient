<script lang="ts">
  import CheckCircleIcon from 'phosphor-svelte/lib/CheckCircleIcon';
  import WarningCircleIcon from 'phosphor-svelte/lib/WarningCircleIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import ClockIcon from 'phosphor-svelte/lib/ClockIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import { app, errorText, installs, openProfile, runs, t, tp } from '../lib/stores.svelte';

  let { runId, onclose }: { runId: number; onclose: () => void } = $props();
  const run = $derived(runs.find((r) => r.id === runId) ?? null);
  const done = $derived(run?.jobs.filter((j) => j.status === 'done').length ?? 0);
  const failed = $derived(run?.jobs.filter((j) => j.status === 'error').length ?? 0);
  const prof = (id: string) => app.profiles.find((p) => p.id === id);
  const pct = (projectId: string) => { const e = installs[projectId]; return e && e.total ? Math.round((e.done / e.total) * 100) : 0; };
  const label = (projectId: string) => installs[projectId]?.label ?? '';
</script>

{#if run}
  <Modal title={run.kind === 'update' ? t('flow.run.titleUpdate', { name: run.title }) : t('flow.run.title', { name: run.title })} {onclose} onenter={onclose}>
    <ul class="jobs">
      {#each run.jobs as j, i (j.profileId + j.projectId + i)}
        <li class="job {j.status}">
          {#if run.kind === 'update'}
            <Tile kind="mod" id={j.projectId} src={null} size={36} radius={6} />
          {:else}
            <InstanceIcon profile={prof(j.profileId)} id={j.profileId} size={36} notch={7} />
          {/if}
          <span class="t">
            <b>{run.kind === 'update' ? j.title : (prof(j.profileId)?.name ?? run.title)}</b>
            {#if j.status === 'running'}
              <span class="bar step-end" role="progressbar" aria-label={run.title} aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct(j.projectId)}><span style:transform="scaleX({pct(j.projectId) / 100})"></span></span>
              <span class="hint mono">{label(j.projectId)}</span>
            {:else if j.status === 'error'}
              <span class="err">{errorText({ code: j.error ?? 'unknown' })}</span>
            {:else if j.status === 'queued'}
              <span class="hint">{t('flow.run.queued')}</span>
            {:else}
              <span class="hint">{t('flow.run.done')}</span>
            {/if}
          </span>
          <span class="st" aria-hidden="true">
            {#if j.status === 'running'}<span class="spin"><CircleNotchIcon weight="bold" size={18} /></span>
            {:else if j.status === 'done'}<CheckCircleIcon weight="bold" size={18} />
            {:else if j.status === 'error'}<WarningCircleIcon weight="bold" size={18} />
            {:else}<ClockIcon weight="bold" size={18} />{/if}
          </span>
        </li>
      {/each}
    </ul>
    {#if run.finished}
      <p class="sum" class:bad={failed > 0} role="status">{failed ? t('flow.done.partial', { ok: done, total: run.jobs.length, name: run.title }) : tp(run.kind === 'update' ? 'flow.done.updated' : 'flow.done.installed', done, { name: run.title })}</p>
    {:else}
      <p class="hint bg">{t('flow.run.background')}</p>
    {/if}
    {#snippet footer()}
      {#if run.finished && run.kind !== 'update' && run.jobs.length === 1 && run.jobs[0].status === 'done'}
        <button class="btn" type="button" onclick={() => { const id = run.jobs[0].profileId; onclose(); openProfile(id, 'overview'); }}>{t('flow.run.open')}</button>
      {/if}
      <button class="btn" class:accent={run.finished} type="button" onclick={onclose}>{run.finished ? t('common.done') : t('flow.run.hide')}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .jobs { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); }
  .job { display: grid; grid-template-columns: auto minmax(0, 1fr) 24px; gap: 12px; align-items: center; padding: 10px 12px; border-bottom: 1px solid var(--line-soft); }
  .job:last-child { border-bottom: 0; }
  .t { display: grid; gap: 4px; min-width: 0; }
  .t b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bar { height: 6px; background: var(--ink-4); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent-fill); transform-origin: left; transition: transform var(--dur) var(--ease); }
  .st { color: var(--paper-mute); display: inline-flex; }
  .done .st { color: var(--leaf); }
  .error .st { color: var(--tomato); }
  .running .st { color: var(--mango); }
  .queued { opacity: 0.7; }
  .sum { margin: 12px 0 0; padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--leaf) 45%, transparent); background: color-mix(in srgb, var(--leaf) 10%, transparent); }
  .sum.bad { border-color: var(--tomato); background: color-mix(in srgb, var(--tomato) 10%, transparent); }
  .bg { margin: 12px 0 0; }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
