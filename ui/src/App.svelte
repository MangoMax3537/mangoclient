<script lang="ts">
  import { onMount } from 'svelte';
  import TitleBar from './components/TitleBar.svelte';
  import ResizeBorders from './components/ResizeBorders.svelte';
  import ConfirmDialog from './components/ConfirmDialog.svelte';
  import Rail from './components/Rail.svelte';
  import AppBackground from './components/AppBackground.svelte';
  import { engineProfile } from './lib/bgMode';
  import Home from './components/Home.svelte';
  import Library from './components/Library.svelte';
  import ProfilePage from './components/ProfilePage.svelte';
  import Toasts from './components/Toasts.svelte';
  import ContextMenu from './components/ContextMenu.svelte';
  import Lazy from './components/Lazy.svelte';
  import { BusyGate } from './lib/loadState.svelte';
  import DropHint from './components/DropHint.svelte';
  import { dialogs, screens, skeletons } from './lib/lazy';
  import { pf, watchDrops } from './lib/portability.svelte';
  import { actions, app, auth, errorText, flow, init, initTheme, nav, platform, selectedProfile, servers, t, toast, ui, updates, lang } from './lib/stores.svelte';
  import { inTauri, kind } from './lib/api';
  import { installShortcuts } from './lib/shortcutActions';
  import { installNativeMenuGuard } from './lib/contextMenu.svelte';
  import { fmtDuration, fmtRelative } from './lib/format';
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
    const noNative = installNativeMenuGuard();
    const stop = watchDrops({ tauri: inTauri(), mock: kind === 'mock' });
    return () => { off(); noNative(); stop(); };
  });
  $effect(() => { document.documentElement.dataset.shell = platform.mode; });
  const stopName = $derived(app.profiles.find((p) => p.id === ui.confirmStop)?.name ?? '');
  async function confirmStop() {
    const id = ui.confirmStop;
    ui.confirmStop = null;
    if (id) await actions.stop(id);
  }
  const delP = $derived(app.profiles.find((p) => p.id === ui.confirmDelete) ?? null);
  let deleting = $state(false);
  async function confirmDelete() {
    const p = delP;
    if (!p) return;
    deleting = true;
    try { await actions.deleteProfile(p.id); toast(t('psettings.deleted', { name: p.name }), 'ok'); ui.confirmDelete = null; }
    catch (e) { toast(errorText(e), 'error'); }
    deleting = false;
  }
  $effect(() => { if (ui.confirmDelete && !delP) ui.confirmDelete = null; });
  const bgProfile = $derived(engineProfile(nav.view, nav.profileId, app.profiles, selectedProfile()));
  const bgMode = $derived(nav.view === 'home' && bgProfile ? 'hero' : 'full');
  $effect(() => { document.documentElement.lang = app.config?.language ?? 'de'; });
</script>

{#snippet skel()}<Lazy load={skeletons.view} props={{ view: nav.view }} />{/snippet}

<div class="shell">
  <TitleBar />
  <div class="body">
    <Rail />
    <main>
      <!-- always mounted, in every state (booting, no instance, onboarding, error): the bundled wallpaper is its floor -->
      <AppBackground profileId={bgProfile?.id ?? ''} mcVersion={bgProfile?.mcVersion ?? ''} background={bgProfile?.background ?? { kind: 'auto' }} tint={bgProfile?.color ?? null} mode={bgMode} />
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
  {#if servers.join}<Lazy load={dialogs.serverJoin} props={{ server: servers.join, onclose: () => (servers.join = null) }} />{/if}
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
  {#if ui.rename}<Lazy load={dialogs.renameDialog} props={{ id: ui.rename }} />{/if}
  {#if delP}
    <ConfirmDialog danger busy={deleting} title={t('psettings.confirmTitle', { name: delP.name })} body={t('psettings.confirmBody')}
      facts={[
        { label: t('psettings.lose.playtime'), value: fmtDuration(delP.playTimeMs) },
        { label: t('psettings.lose.mods'), value: String(delP.modCount) },
        { label: t('psettings.lose.last'), value: fmtRelative(delP.lastPlayed, lang()) ?? t('home.never') },
      ]} warn={t('psettings.lose.warn')} confirmLabel={t('profile.delete')} onconfirm={confirmDelete} oncancel={() => (ui.confirmDelete = null)} />
  {/if}
  {#if pf.dragging}<DropHint />{/if}
  <ContextMenu />
  <ResizeBorders />
  <Toasts />
</div>

<style>
  .shell { position: relative; display: flex; flex-direction: column; height: 100%; min-width: 960px; min-height: 600px; background: var(--ink-0); }
  .body { flex: 1; display: flex; min-height: 0; }
  main { flex: 1; min-width: 0; position: relative; background: var(--ink-0); isolation: isolate; }
  .view { position: relative; z-index: 1; height: 100%; min-height: 0; }
  .banner { position: absolute; z-index: 10; top: 8px; right: 20px; display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 14px; background: var(--ink-3); border: 1px solid var(--tomato); color: var(--paper); }
</style>
