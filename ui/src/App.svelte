<script lang="ts">
  import { onMount } from 'svelte';
  import TitleBar from './components/TitleBar.svelte';
  import ResizeBorders from './components/ResizeBorders.svelte';
  import ConfirmDialog from './components/ConfirmDialog.svelte';
  import Rail from './components/Rail.svelte';
  import Home from './components/Home.svelte';
  import Library from './components/Library.svelte';
  import ProfilePage from './components/ProfilePage.svelte';
  import Toasts from './components/Toasts.svelte';
  import Lazy from './components/Lazy.svelte';
  import { BusyGate } from './lib/loadState.svelte';
  import DropHint from './components/DropHint.svelte';
  import { dialogs, screens, skeletons } from './lib/lazy';
  import { pf, watchDrops } from './lib/portability.svelte';
  import { actions, app, auth, errorText, flow, init, initTheme, nav, platform, t, ui, updates } from './lib/stores.svelte';
  import { inTauri, kind } from './lib/api';
  import { installShortcuts } from './lib/shortcutActions';
  import { enter } from './lib/motion';

  // Dev: `?view=library|discover|servers|stats|settings` opens that view at start (used to look at the skeletons).
  try {
    const v = new URLSearchParams(location.search).get('view');
    if (v && ['home', 'library', 'discover', 'servers', 'skins', 'stats', 'settings'].includes(v)) nav.view = v as typeof nav.view;
  } catch { /* no location */ }
  // Boot: nothing for the first ~150 ms, then the skeleton of the screen that is about to appear.
  const boot = new BusyGate();
  $effect(() => { boot.update(!app.ready); });
  onMount(() => {
    initTheme();
    void init().then(() => { if (updates.status.state === 'idle') actions.updateCheck(); });
    const off = installShortcuts();
    const stop = watchDrops({ tauri: inTauri(), mock: kind === 'mock' });
    return () => { off(); stop(); };
  });
  $effect(() => { document.documentElement.dataset.shell = platform.mode; });
  const stopName = $derived(app.profiles.find((p) => p.id === ui.confirmStop)?.name ?? '');
  async function confirmStop() {
    const id = ui.confirmStop;
    ui.confirmStop = null;
    if (id) await actions.stop(id);
  }
  $effect(() => { document.documentElement.lang = app.config?.language ?? 'de'; });
</script>

{#snippet skel()}<Lazy load={skeletons.view} props={{ view: nav.view }} />{/snippet}

<div class="shell">
  <TitleBar />
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
        {#key nav.view}
          <div class="view" in:enter>
            {#if nav.view === 'home'}<Home />
            {:else if nav.view === 'library'}<Library />
            {:else if nav.view === 'profile' && nav.profileId}<ProfilePage id={nav.profileId} />
            {:else if nav.view === 'discover'}<Lazy load={screens.discover} fallback={skel} />
            {:else if nav.view === 'servers'}<Lazy load={screens.servers} fallback={skel} />
            {:else if nav.view === 'skins'}<Lazy load={screens.skins} fallback={skel} />
            {:else if nav.view === 'stats'}<Lazy load={screens.stats} fallback={skel} />
            {:else}<Lazy load={screens.settings} fallback={skel} />{/if}
          </div>
        {/key}
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
  {#if ui.shortcuts}<Lazy load={dialogs.shortcutSheet} props={{ onclose: () => (ui.shortcuts = false) }} />{/if}
  {#if ui.confirmStop}
    <ConfirmDialog danger title={t('keys.stopTitle', { name: stopName })} body={t('keys.stopBody')} confirmLabel={t('home.stop')} onconfirm={confirmStop} oncancel={() => (ui.confirmStop = null)} />
  {/if}
  {#if pf.dragging}<DropHint />{/if}
  <ResizeBorders />
  <Toasts />
</div>

<style>
  .shell { position: relative; display: flex; flex-direction: column; height: 100%; min-width: 960px; min-height: 600px; background: var(--ink-0); }
  .body { flex: 1; display: flex; min-height: 0; }
  main { flex: 1; min-width: 0; position: relative; background: var(--ink-0); }
  .view { height: 100%; min-height: 0; }
  .banner { position: absolute; z-index: 10; top: 8px; right: 20px; display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 14px; background: var(--ink-3); border: 1px solid var(--tomato); border-radius: var(--r-m); color: var(--paper); }
</style>
