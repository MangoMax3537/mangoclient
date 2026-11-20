<script lang="ts">
  // Orchestrates the install dialogs (docs/ipc.md Part 4): instance picker -> dependency plan -> progress,
  // modpack dialog, update preview. The running install itself lives in the store, so closing is safe.
  import InstancePicker from './InstancePicker.svelte';
  import DependencyDialog from './DependencyDialog.svelte';
  import ModpackDialog from './ModpackDialog.svelte';
  import UpdatePreview from './UpdatePreview.svelte';
  import RunDialog from './RunDialog.svelte';
  import { suggestInstance, type RunStep } from '../lib/install';
  import { closeFlow, flow, nav, rememberInstallProfile, startContentRun, startInstall, tp } from '../lib/stores.svelte';

  /** Profiles picked in step 1 (null = still picking). A request with a scope skips the picker. */
  let picked = $state<string[] | null>(null);
  const req = $derived(flow.req);
  $effect(() => { void req; picked = req?.kind === 'content' && req.scopeProfileId ? [req.scopeProfileId] : null; });

  function start(kind: 'content' | 'update', title: string, iconUrl: string | null, steps: RunStep[]) {
    for (const s of steps) rememberInstallProfile(s.profileId);
    const run = startContentRun(kind, title, iconUrl, steps);
    startInstall({ kind: 'run', runId: run.id });
  }
  function createInstance() {
    if (req?.kind !== 'content') return;
    const p = req.project;
    const s = suggestInstance({ loaders: p.loaders ?? [], gameVersions: p.gameVersions ?? [] });
    nav.newProfilePrefill = { name: p.title, loader: s.loader, mcVersion: s.mcVersion, then: p };
    nav.newProfile = true;
    closeFlow();
  }
</script>

{#if req?.kind === 'content'}
  {#if picked === null}
    <InstancePicker project={req.project} versionId={req.versionId} onnext={(ids) => (picked = ids)} oncreate={createInstance} onclose={closeFlow} />
  {:else}
    <DependencyDialog project={req.project} versionId={req.versionId} profileIds={picked} update={req.update}
      onback={req.scopeProfileId ? undefined : () => (picked = null)}
      onconfirm={(steps) => start(req.update ? 'update' : 'content', req.project.title, req.project.iconUrl, steps)} onclose={closeFlow} />
  {/if}
{:else if req?.kind === 'modpack'}
  <ModpackDialog project={req.project} versionId={req.versionId} onclose={closeFlow} />
{:else if req?.kind === 'updates'}
  <UpdatePreview profileId={req.profileId} infos={req.infos} onconfirm={(steps) => start('update', tp('flow.upd.items', steps.length), null, steps)} onclose={closeFlow} />
{:else if req?.kind === 'run'}
  <RunDialog runId={req.runId} onclose={closeFlow} />
{/if}
