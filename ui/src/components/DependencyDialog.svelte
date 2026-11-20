<script lang="ts">
  import { untrack } from 'svelte';
  import LockSimpleIcon from 'phosphor-svelte/lib/LockSimpleIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import LoadError from './LoadError.svelte';
  import DialogBodySkeleton from './skeleton/DialogBodySkeleton.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { fileCount, includeFor, planBytes, planGroups, type RunStep } from '../lib/install';
  import { fmtBytes } from '../lib/format';
  import { app, errorText, getApi, lang, t, tp, type FlowProject } from '../lib/stores.svelte';
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

  type Loaded = { plans: Record<string, InstallPlan | 'error'>; errors: Record<string, string> };
  const pl = createLoadState<Loaded>({ isEmpty: () => false });
  const plans = $derived(pl.data?.plans ?? {});
  const errors = $derived(pl.data?.errors ?? {});
  /** Optional ids the user ticked, per profile. Nothing is ticked by default. */
  let chosen = $state<Record<string, string[]>>({});
  const loading = $derived(pl.phase === 'loading' || pl.phase === 'idle');

  $effect(() => {
    void profileIds.join(','); void project.projectId; void versionId;
    const ids = [...profileIds];
    untrack(() => { chosen = {}; });
    pl.run(async () => {
      const out: Loaded = { plans: {}, errors: {} };
      await Promise.all(ids.map(async (profileId) => {
        try {
          out.plans[profileId] = await getApi().installPlan({ projectId: project.projectId, profileId, ...(versionId ? { versionId } : {}) });
        } catch (e) { out.plans[profileId] = 'error'; out.errors[profileId] = errorText(e); }
      }));
      return out;
    });
    return () => pl.cancel();
  });

  const ready = $derived(profileIds.filter((id) => plans[id] && plans[id] !== 'error') as string[]);
  const usable = $derived(ready);
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
      profileId, projectId: project.projectId, versionId, title: project.title,
      include: includeFor(plans[profileId] as InstallPlan, chosen[profileId] ?? []),
    })));
  }
  const sz = (i: InstallPlanItem) => (i.sizeBytes ? fmtBytes(i.sizeBytes, lang()) : '');
</script>

<Modal title={update ? t('flow.dep.titleUpdate', { name: project.title }) : t('flow.dep.title', { name: project.title })} {onclose} size="lg" onenter={confirm}>
  {#if pl.phase === 'error'}
    <LoadError error={pl.error} onretry={() => pl.retry()} />
  {:else if loading && pl.showSkeleton}
    <DialogBodySkeleton rows={3} img={36} />
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
          <div class="item req">
            <Tile kind="mod" id={g.requested.projectId} src={g.requested.iconUrl ?? project.iconUrl} size={36} radius={6} />
            <span class="t"><b>{g.requested.title}</b><span class="hint mono">{g.requested.alreadyInstalled ? t('flow.dep.alreadyThisVersion', { v: g.requested.installedVersion ?? '' }) : update && g.requested.installedVersion ? t('flow.dep.fromTo', { from: g.requested.installedVersion }) : t('flow.dep.requested')}</span></span>
            <span class="sz mono">{sz(g.requested)}</span>
          </div>
        {/if}

        {#if g.required.length}
          <h4 class="eyebrow">{tp('flow.dep.required', g.required.length)}</h4>
          <ul class="deps">
            {#each g.required as d (d.projectId)}
              <li class="item" class:done={d.alreadyInstalled}>
                <Tile kind="mod" id={d.projectId} src={d.iconUrl} size={32} radius={6} />
                <span class="t"><b>{d.title}</b>
                  <span class="hint mono">{#if d.alreadyInstalled}{t('flow.dep.installedV', { v: d.installedVersion ?? '' })}{:else}{t('flow.dep.requiredBy', { name: project.title })}{/if}</span></span>
                <span class="sz mono">{d.alreadyInstalled ? '' : sz(d)}</span>
                <span class="lock" title={t('flow.dep.locked')}>{#if d.alreadyInstalled}<CheckIcon weight="bold" size={16} aria-hidden="true" />{:else}<input type="checkbox" checked disabled aria-label={t('flow.dep.lockedLabel', { name: d.title })} /><LockSimpleIcon weight="bold" size={12} aria-hidden="true" />{/if}</span>
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
  .inst + .inst { margin-top: 20px; padding-top: 16px; border-top: 1px solid var(--line); }
  .who { display: flex; align-items: center; gap: 8px; font-size: var(--fs-lg); margin-bottom: 8px; }
  .who .hint { margin-left: auto; }
  .to { margin: 0 0 8px; }
  .eyebrow { margin: 14px 0 4px; }
  .deps { list-style: none; margin: 0; padding: 0; display: grid; border: 1px solid var(--line); border-radius: var(--r-m); }
  .item { display: grid; grid-template-columns: auto minmax(0, 1fr) auto 34px; gap: 12px; align-items: center; padding: 8px 12px; }
  .item.req { grid-template-columns: auto minmax(0, 1fr) auto; background: var(--ink-3); border: 1px solid var(--line); border-radius: var(--r-m); }
  .deps .item { border-bottom: 1px solid var(--line-soft); }
  .deps .item:last-child { border-bottom: 0; }
  .item.done { opacity: 0.6; }
  .t { display: grid; gap: 1px; min-width: 0; cursor: default; }
  .t b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .t .hint { font-size: var(--fs-xs); }
  label.t { cursor: default; }
  .sz { font-size: var(--fs-sm); color: var(--paper-dim); white-space: nowrap; }
  .lock { display: inline-flex; align-items: center; justify-content: flex-end; gap: 4px; color: var(--paper-mute); }
  .lock input { width: 16px; height: 16px; margin: 0; }
  .item.done .lock { color: var(--leaf); }
  .warn { display: flex; gap: 10px; margin-top: 14px; padding: 10px 12px; border: 1px solid color-mix(in srgb, var(--mango) 45%, transparent); border-radius: var(--r-m); background: color-mix(in srgb, var(--mango) 9%, transparent); font-size: var(--fs-sm); }
  .warn > :global(svg) { flex: none; margin-top: 2px; color: var(--mango); }
  .warn ul { margin: 4px 0; padding-left: 18px; }
  .tot { margin-right: auto; align-self: center; font-size: var(--fs-sm); color: var(--paper-dim); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
