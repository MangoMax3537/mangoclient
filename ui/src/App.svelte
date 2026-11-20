<script lang="ts">
  import { onMount } from 'svelte';
  import Rail from './components/Rail.svelte';
  import Home from './components/Home.svelte';
  import Library from './components/Library.svelte';
  import ProfilePage from './components/ProfilePage.svelte';
  import Toasts from './components/Toasts.svelte';
  import UpdatePill from './components/UpdatePill.svelte';
  import Activity from './components/Activity.svelte';
  import Lazy from './components/Lazy.svelte';
  import { BusyGate } from './lib/loadState.svelte';
  import DropHint from './components/DropHint.svelte';
  import { dialogs, screens, skeletons } from './lib/lazy';
  import { pf, watchDrops } from './lib/portability.svelte';
  import { actions, app, auth, errorText, flow, init, initTheme, nav, t, updates } from './lib/stores.svelte';
  import { inHttp, inTauri, kind } from './lib/api';

  // Dev: `?view=library|discover|servers|stats|settings` opens that view at start (used to look at the skeletons).
  try {
    const v = new URLSearchParams(location.search).get('view');
    if (v && ['home', 'library', 'discover', 'servers', 'stats', 'settings'].includes(v)) nav.view = v as typeof nav.view;
  } catch { /* no location */ }
  // Boot: nothing for the first ~150 ms, then the skeleton of the screen that is about to appear.
  const boot = new BusyGate();
  $effect(() => { boot.update(!app.ready); });
  onMount(() => {
    initTheme();
    void init().then(() => { if (updates.status.state === 'idle') actions.updateCheck(); });
    return watchDrops({ tauri: inTauri(), mock: kind === 'mock' });
  });
  $effect(() => { document.documentElement.lang = app.config?.language ?? 'de'; });
</script>

{#snippet skel()}<Lazy load={skeletons.view} props={{ view: nav.view }} />{/snippet}

<div class="shell">
  <div class="strip" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>{t('app.name')}</span>
    {#if !inTauri()}<span class="dev mono" data-tauri-drag-region>{inHttp() ? 'dev server' : 'mock'}</span>{/if}
    <span class="grow" data-tauri-drag-region></span>
    <Activity />
    <UpdatePill />
    <span class="ver mono" data-tauri-drag-region>{app.version}</span>
  </div>
  <div class="body">
    <Rail />
    <main>
      {#if !app.ready}
        {#if boot.show}<Lazy load={skeletons.view} props={{ view: nav.view }} />{/if}
      {:else}
        {#if app.error}
          <div class="banner" role="alert">{errorText(app.error)}
            <button class="btn ghost" type="button" onclick={() => (app.error = null)}>{t('common.dismiss')}</button>
          </div>
        {/if}
        {#if nav.view === 'home'}<Home />
        {:else if nav.view === 'library'}<Library />
        {:else if nav.view === 'profile' && nav.profileId}<ProfilePage id={nav.profileId} />
        {:else if nav.view === 'discover'}<Lazy load={screens.discover} fallback={skel} />
        {:else if nav.view === 'servers'}<Lazy load={screens.servers} fallback={skel} />
        {:else if nav.view === 'stats'}<Lazy load={screens.stats} fallback={skel} />
        {:else}<Lazy load={screens.settings} fallback={skel} />{/if}
      {/if}
    </main>
  </div>
  {#if nav.newProfile}<Lazy load={screens.newProfile} />{/if}
  {#if flow.req}<Lazy load={dialogs.installFlow} />{/if}
  {#if nav.iconPicker}<Lazy load={dialogs.iconPicker} props={{ profileId: nav.iconPicker, onclose: () => (nav.iconPicker = null) }} />{/if}
  {#if nav.backgroundPicker}<Lazy load={dialogs.backgroundPicker} props={{ profileId: nav.backgroundPicker, onclose: () => (nav.backgroundPicker = null) }} />{/if}
  {#if pf.imp && pf.impOpen}<Lazy load={dialogs.importDialog} />{/if}
  {#if pf.exp}<Lazy load={dialogs.exportDialog} props={{ profileId: pf.exp.profileId, onclose: () => (pf.exp = null) }} />{/if}
  {#if pf.diag}<Lazy load={dialogs.diagnosticsDialog} props={{ profileId: pf.diag.profileId, onclose: () => (pf.diag = null) }} />{/if}
  {#if pf.crash}<Lazy load={dialogs.crashPanel} props={{ profileId: pf.crash.profileId, logId: pf.crash.logId, onclose: () => (pf.crash = null) }} />{/if}
  {#if updates.overlay}<Lazy load={dialogs.updateOverlay} />{/if}
  {#if app.ready && app.config && !app.config.firstRunDone}<Lazy load={screens.onboarding} />{/if}
  {#if auth.relogin}<Lazy load={dialogs.reloginDialog} />{/if}
  {#if pf.dragging}<DropHint />{/if}
  <Toasts />
</div>

<style>
  .shell { position: relative; display: flex; flex-direction: column; height: 100%; min-width: 960px; min-height: 600px; background: var(--ink-0); }
  .strip { position: relative; z-index: 20; height: 32px; flex: none; display: flex; align-items: center; gap: 10px; padding: 0 14px; background: var(--ink-0); border-bottom: 1px solid var(--line-soft); font-size: 12px; color: var(--paper-dim); }
  .brand { font: 800 13px var(--font-display); color: var(--mango); letter-spacing: 0.02em; }
  .dev { padding: 0 6px; border: 1px solid var(--line); border-radius: 3px; }
  .grow { flex: 1; }
  .ver { color: var(--paper-mute); }
  .body { flex: 1; display: flex; min-height: 0; }
  main { flex: 1; min-width: 0; position: relative; background: var(--ink-0); }
  .banner { position: absolute; z-index: 10; top: 8px; right: 20px; display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 14px; background: var(--ink-3); border: 1px solid var(--tomato); border-radius: var(--r-m); color: var(--paper); }
</style>
