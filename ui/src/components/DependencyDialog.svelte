<script lang="ts">
  import { untrack } from 'svelte';
  import LockSimpleIcon from 'phosphor-svelte/lib/LockSimpleIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ProhibitIcon from 'phosphor-svelte/lib/ProhibitIcon';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import { fileCount, includeFor, planBytes, planGroups, requestedBlocked, type RunStep } from '../lib/install';
  import { fmtBytes } from '../lib/format';
  import { actions, app, errorText, getApi, lang, t, tp, type FlowProject } from '../lib/stores.svelte';
  import type { InstallPlan, InstallPlanItem } from '../lib/types';

  interface Props {
    project: FlowProject;
    versionId?: string;
    profileIds: string[];
    /** Update mode: the button says Update and the title names the update. */
    update?: boolean;
    onconfirm: (steps: RunStep[]) => void;
    onback?: () => void;
    onclose: () => void;
  }
  let { project, versionId, profileIds, update = false, onconfirm, onback, onclose }: Props = $props();

  let plans = $state<Record<string, InstallPlan | 'error'>>({});
  let errors = $state<Record<string, string>>({});
  /** Optional ids the user ticked, per profile. Nothing is ticked by default. */
  let chosen = $state<Record<string, string[]>>({});
  let loading = $state(true);

  async function load() {
    loading = true; plans = {}; errors = {}; chosen = {};
    await Promise.all(profileIds.map(async (profileId) => {
      try {
        plans[profileId] = await getApi().installPlan({ provider: project.provider, projectId: project.projectId, profileId, ...(versionId ? { versionId } : {}) });
      } catch (e) { plans[profileId] = 'error'; errors[profileId] = errorText(e); }
    }));
    loading = false;
  }
  $effect(() => { void profileIds.join(','); void project.projectId; void versionId; untrack(() => void load()); });

  const ready = $derived(profileIds.filter((id) => plans[id] && plans[id] !== 'error') as string[]);
  const usable = $derived(ready.filter((id) => !requestedBlocked(plans[id] as InstallPlan)));
  const files = $derived(usable.reduce((a, id) => a + fileCount(plans[id] as InstallPlan, chosen[id] ?? []), 0));
  const bytes = $derived(usable.reduce((a, id) => a + planBytes(plans[id] as InstallPlan, chosen[id] ?? []), 0));
  const nothingNew = $derived(!loading && usable.length > 0 && files === 0);
  const name = (id: string) => app.profiles.find((p) => p.id === id)?.name ?? id;
  const prof = (id: string) => app.profiles.find((p) => p.id === id);

  function tick(profileId: string, projectId: string, on: boolean) {
    const cur = chosen[profileId] ?? [];
    chosen[profileId] = on ? [...cur.filter((x) => x !== projectId), projectId] : cur.filter((x) => x !== projectId);
  }
  function confirm() {
    if (!usable.length || loading) return;
    onconfirm(usable.map((profileId) => ({
      profileId, provider: project.provider, projectId: project.projectId, versionId, title: project.title,
      include: includeFor(plans[profileId] as InstallPlan, chosen[profileId] ?? []),
    })));
  }
  const sz = (i: InstallPlanItem) => (i.sizeBytes ? fmtBytes(i.sizeBytes, lang()) : '');
</script>

<Modal title={update ? t('flow.dep.titleUpdate', { name: project.title }) : t('flow.dep.title', { name: project.title })} {onclose} size="lg" onenter={confirm}>
  {#if loading}
    <ul class="skl" aria-busy="true" aria-label={t('common.loading')}>{#each Array(3) as _, i (i)}<li><span class="img skel notch" style:--n="8px"></span><span class="ln skel"></span></li>{/each}</ul>
  {/if}

  {#each profileIds as pid (pid)}
    {@const plan = plans[pid]}
    {#if plan && plan !== 'error'}
      {@const g = planGroups(plan)}
      <section class="inst" aria-labelledby="dp-{pid}">
        {#if profileIds.length > 1}
          <h3 id="dp-{pid}" class="who"><InstanceIcon profile={prof(pid)} id={pid} size={24} notch={5} />{name(pid)}
            <span class="mono hint">{prof(pid)?.mcVersion} / {prof(pid)?.loader}</span></h3>
        {:else}
          <h3 id="dp-{pid}" class="sr-only">{name(pid)}</h3>
          <p class="to hint">{t('flow.dep.into')} <b>{name(pid)}</b> <span class="mono">({prof(pid)?.mcVersion} / {prof(pid)?.loader})</span></p>
        {/if}

        {#if g.requested}
          <div class="item req" class:blocked={g.requested.distributionBlocked}>
            <Tile kind="mod" id={g.requested.projectId} src={g.requested.iconUrl ?? project.iconUrl} size={36} radius={6} />
            <span class="t"><b>{g.requested.title}</b><span class="hint mono">{g.requested.alreadyInstalled ? t('flow.dep.alreadyThisVersion', { v: g.requested.installedVersion ?? '' }) : update && g.requested.installedVersion ? t('flow.dep.fromTo', { from: g.requested.installedVersion }) : t('flow.dep.requested')}</span></span>
            <span class="sz mono">{sz(g.requested)}</span>
          </div>
        {/if}

        {#if g.required.length}
          <h4 class="eyebrow">{tp('flow.dep.required', g.required.length)}</h4>
          <ul class="deps">
            {#each g.required as d (d.projectId)}
              <li class="item" class:done={d.alreadyInstalled} class:blocked={d.distributionBlocked}>
                <Tile kind="mod" id={d.projectId} src={d.iconUrl} size={32} radius={6} />
                <span class="t"><b>{d.title}</b>
                  <span class="hint mono">{#if d.alreadyInstalled}{t('flow.dep.installedV', { v: d.installedVersion ?? '' })}{:else if d.distributionBlocked}{t('flow.dep.blockedShort')}{:else}{t('flow.dep.requiredBy', { name: project.title })}{/if}</span></span>
                <span class="sz mono">{d.alreadyInstalled ? '' : sz(d)}</span>
                <span class="lock" title={t('flow.dep.locked')}>{#if d.alreadyInstalled}<CheckIcon weight="bold" size={16} aria-hidden="true" />{:else if d.distributionBlocked}<ProhibitIcon weight="bold" size={16} aria-hidden="true" />{:else}<input type="checkbox" checked disabled aria-label={t('flow.dep.lockedLabel', { name: d.title })} /><LockSimpleIcon weight="bold" size={12} aria-hidden="true" />{/if}</span>
              </li>
            {/each}
          </ul>
        {/if}

        {#if g.optional.length}
          <h4 class="eyebrow">{tp('flow.dep.optional', g.optional.length)}</h4>
          <ul class="deps">
            {#each g.optional as d (d.projectId)}
              <li class="item" class:done={d.alreadyInstalled}>
                <Tile kind="mod" id={d.projectId} src={d.iconUrl} size={32} radius={6} />
                <label class="t" for="opt-{pid}-{d.projectId}"><b>{d.title}</b>
                  <span class="hint mono">{d.alreadyInstalled ? t('flow.dep.installedV', { v: d.installedVersion ?? '' }) : t('flow.dep.optionalHint')}</span></label>
                <span class="sz mono">{d.alreadyInstalled ? '' : sz(d)}</span>
                <span class="lock">{#if d.alreadyInstalled}<CheckIcon weight="bold" size={16} aria-hidden="true" />{:else}
                  <input id="opt-{pid}-{d.projectId}" type="checkbox" checked={(chosen[pid] ?? []).includes(d.projectId)} onchange={(e) => tick(pid, d.projectId, e.currentTarget.checked)} />{/if}</span>
              </li>
            {/each}
          </ul>
        {/if}

        {#if plan.conflicts.length}
          <div class="warn" role="alert">
            <WarningIcon weight="bold" size={16} aria-hidden="true" />
            <div><b>{tp('flow.dep.conflicts', plan.conflicts.length)}</b>
              <ul>{#each plan.conflicts as c (c.projectId)}<li>{t('flow.dep.conflictLine', { installed: c.title, name: c.with })}</li>{/each}</ul>
              <span class="hint">{t('flow.dep.conflictHint')}</span></div>
          </div>
        {/if}

        {#if g.blocked.length}
          <div class="warn block" role="note">
            <WarningIcon weight="bold" size={16} aria-hidden="true" />
            <div><b>{tp('flow.dep.blocked', g.blocked.length)}</b>
              <p class="hint">{t('flow.dep.blockedNote')}</p>
              <ul class="bl">{#each g.blocked as d (d.projectId)}
                <li><span>{d.title}</span>
                  {#if d.downloadUrl}<button class="btn small" type="button" onclick={() => d.downloadUrl && actions.openExternal(d.downloadUrl)}><ArrowSquareOutIcon weight="bold" size={14} aria-hidden="true" />{t('flow.openWebsite')}</button>{/if}</li>
              {/each}</ul>
            </div>
          </div>
        {/if}
      </section>
    {:else if plan === 'error'}
      <p class="err" role="alert">{name(pid)}: {errors[pid]}</p>
    {/if}
  {/each}

  {#snippet footer()}
    <span class="tot mono" aria-live="polite">{#if !loading && usable.length}{tp('flow.dep.files', files) + ' · ' + fmtBytes(bytes, lang())}{/if}</span>
    {#if onback}<button class="btn ghost" type="button" onclick={onback}>{t('common.back')}</button>{/if}
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={loading || !usable.length || nothingNew} onclick={confirm}>
      {#if loading}<span class="spin"><CircleNotchIcon weight="bold" size={14} aria-hidden="true" /></span>{/if}
      {update ? t('flow.dep.confirmUpdate') : t('flow.dep.confirm')}
    </button>
  {/snippet}
</Modal>

<style>
  .skl { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  .skl li { display: flex; gap: 12px; align-items: center; }
  .skl .img { width: 36px; height: 36px; }
  .skl .ln { height: 14px; width: 50%; }
  .inst + .inst { margin-top: 20px; padding-top: 16px; border-top: 1px solid var(--line); }
  .who { display: flex; align-items: center; gap: 8px; font-size: var(--fs-lg); margin-bottom: 8px; }
  .who .hint { margin-left: auto; }
  .to { margin: 0 0 8px; }
  .eyebrow { margin: 14px 0 4px; }
  .deps { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); border-radius: var(--r-m); }
  .item { display: grid; grid-template-columns: auto minmax(0, 1fr) auto 34px; gap: 12px; align-items: center; padding: 8px 12px; }
  .item.req { grid-template-columns: auto minmax(0, 1fr) auto; background: var(--ink-3); border: 1px solid var(--line); border-left: 3px solid var(--accent-fill); border-radius: var(--r-m); }
  .deps .item { border-bottom: 1px solid var(--line-soft); }
  .deps .item:last-child { border-bottom: 0; }
  .item.done { opacity: 0.6; }
  .item.blocked :global(.tile) { opacity: 0.5; filter: grayscale(1); }
  .t { display: grid; gap: 1px; min-width: 0; cursor: default; }
  .t b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .t .hint { font-size: var(--fs-xs); }
  label.t { cursor: pointer; }
  .sz { font-size: var(--fs-sm); color: var(--paper-dim); white-space: nowrap; }
  .lock { display: inline-flex; align-items: center; justify-content: flex-end; gap: 4px; color: var(--paper-mute); }
  .lock input { width: 16px; height: 16px; margin: 0; }
  .item.done .lock { color: var(--leaf); }
  .warn { display: flex; gap: 10px; margin-top: 14px; padding: 10px 12px; border-left: 3px solid var(--mango); background: color-mix(in srgb, var(--mango) 9%, transparent); font-size: var(--fs-sm); }
  .warn > :global(svg) { flex: none; margin-top: 2px; color: var(--mango); }
  .warn.block { border-left-color: var(--tomato); background: color-mix(in srgb, var(--tomato) 9%, transparent); }
  .warn.block > :global(svg) { color: var(--tomato); }
  .warn ul { margin: 4px 0; padding-left: 18px; }
  .warn p { margin: 2px 0 6px; }
  .bl { list-style: none; padding: 0 !important; display: grid; gap: 6px; }
  .bl li { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .tot { margin-right: auto; align-self: center; font-size: var(--fs-sm); color: var(--paper-dim); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
