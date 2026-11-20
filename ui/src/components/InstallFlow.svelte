<script lang="ts">
  // Orchestrates the install dialogs (docs/ipc.md Part 4): instance picker -> dependency plan -> progress,
  // modpack dialog, update preview. The running install itself lives in the store, so closing is safe.
  import Lazy from './Lazy.svelte';
  import { dialogs } from '../lib/lazy';
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
    <Lazy load={dialogs.instancePicker} props={{ project: req.project, versionId: req.versionId, onnext: (ids: string[]) => (picked = ids), oncreate: createInstance, onclose: closeFlow }} />
  {:else}
    <Lazy load={dialogs.dependencyDialog} props={{ project: req.project, versionId: req.versionId, profileIds: picked, update: req.update,
      onback: req.scopeProfileId ? undefined : () => (picked = null),
      onconfirm: (steps: RunStep[]) => start(req.update ? 'update' : 'content', req.project.title, req.project.iconUrl, steps), onclose: closeFlow }} />
  {/if}
{:else if req?.kind === 'modpack'}
  <Lazy load={dialogs.modpackDialog} props={{ project: req.project, versionId: req.versionId, onclose: closeFlow }} />
{:else if req?.kind === 'updates'}
  <Lazy load={dialogs.updatePreview} props={{ profileId: req.profileId, infos: req.infos, onconfirm: (steps: RunStep[]) => start('update', tp('flow.upd.items', steps.length), null, steps), onclose: closeFlow }} />
{:else if req?.kind === 'run'}
  <Lazy load={dialogs.runDialog} props={{ runId: req.runId, onclose: closeFlow }} />
{:else if req?.kind === 'blocked'}
  <Lazy load={dialogs.blockedDialog} props={{ profileId: req.profileId, onclose: closeFlow }} />
{/if}
