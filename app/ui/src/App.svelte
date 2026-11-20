<script lang="ts">
  import { onMount } from 'svelte';
  import Rail from './components/Rail.svelte';
  import Home from './components/Home.svelte';
  import Library from './components/Library.svelte';
  import Settings from './components/Settings.svelte';
  import NewProfile from './components/NewProfile.svelte';
  import ProfilePage from './components/ProfilePage.svelte';
  import Discover from './components/Discover.svelte';
  import Servers from './components/Servers.svelte';
  import Stats from './components/Stats.svelte';
  import Onboarding from './components/Onboarding.svelte';
  import Toasts from './components/Toasts.svelte';
  import UpdatePill from './components/UpdatePill.svelte';
  import ReloginDialog from './components/ReloginDialog.svelte';
  import UpdateOverlay from './components/UpdateOverlay.svelte';
  import { actions, app, auth, errorText, init, nav, t, updates } from './lib/stores.svelte';
  import { inTauri } from './lib/api';

  onMount(() => { init().then(() => { if (updates.status.state === 'idle') actions.updateCheck(); }); });
  $effect(() => { document.documentElement.lang = app.config?.language ?? 'de'; });
</script>

<div class="shell">
  <div class="strip" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>{t('app.name')}</span>
    {#if !inTauri()}<span class="dev mono" data-tauri-drag-region>mock</span>{/if}
    <span class="grow" data-tauri-drag-region></span>
    <UpdatePill />
    <span class="ver mono" data-tauri-drag-region>{app.version}</span>
  </div>
  <div class="body">
    <Rail />
    <main>
      {#if !app.ready}
        <div class="loading" role="status"><span class="sr-only">{t('common.loading')}</span>
          <div class="skel hero-sk" aria-hidden="true"></div>
          <div class="cards-sk" aria-hidden="true">{#each Array(4) as _, i (i)}<div class="skel notch" style:--n="14px"></div>{/each}</div>
        </div>
      {:else}
        {#if app.error}
          <div class="banner" role="alert">{errorText(app.error)}
            <button class="btn ghost" type="button" onclick={() => (app.error = null)}>{t('common.dismiss')}</button>
          </div>
        {/if}
        {#if nav.view === 'home'}<Home />
        {:else if nav.view === 'library'}<Library />
        {:else if nav.view === 'profile' && nav.profileId}<ProfilePage id={nav.profileId} />
        {:else if nav.view === 'discover'}<Discover />
        {:else if nav.view === 'servers'}<Servers />
        {:else if nav.view === 'stats'}<Stats />
        {:else}<Settings />{/if}
      {/if}
    </main>
  </div>
  {#if nav.newProfile}<NewProfile />{/if}
  {#if updates.overlay}<UpdateOverlay />{/if}
  {#if app.ready && app.config && !app.config.firstRunDone}<Onboarding />{/if}
  {#if auth.relogin}<ReloginDialog />{/if}
  <Toasts />
</div>

<style>
  .shell { position: relative; display: flex; flex-direction: column; height: 100%; min-width: 960px; min-height: 600px; background: var(--ink-0); }
  .strip { height: 32px; flex: none; display: flex; align-items: center; gap: 10px; padding: 0 14px; background: var(--ink-0); border-bottom: 1px solid var(--line-soft); font-size: 12px; color: var(--paper-dim); }
  .brand { font: 800 13px var(--font-display); color: var(--mango); letter-spacing: 0.02em; }
  .dev { padding: 0 6px; border: 1px solid var(--line); border-radius: 3px; }
  .grow { flex: 1; }
  .ver { color: var(--paper-mute); }
  .body { flex: 1; display: flex; min-height: 0; }
  main { flex: 1; min-width: 0; position: relative; background: var(--ink-0); }
  .banner { position: absolute; z-index: 10; top: 8px; right: 20px; display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 14px; background: var(--ink-3); border: 1px solid var(--tomato); border-radius: var(--r-m); color: var(--paper); }
  .loading { height: 100%; display: grid; grid-template-rows: 46vh auto; gap: var(--s-5); align-content: start; }
  .hero-sk { opacity: 0.6; }
  .cards-sk { display: flex; gap: var(--s-3); padding: 0 32px 0 44px; }
  .cards-sk div { width: 212px; height: 200px; }
</style>
