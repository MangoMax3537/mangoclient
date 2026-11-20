<script lang="ts">
  import { onMount } from 'svelte';
  import Rail from './components/Rail.svelte';
  import Home from './components/Home.svelte';
  import Library from './components/Library.svelte';
  import Settings from './components/Settings.svelte';
  import NewProfile from './components/NewProfile.svelte';
  import { app, errorText, init, nav, t } from './lib/stores.svelte';
  import { inTauri } from './lib/api';

  onMount(() => { init(); });
  $effect(() => { document.documentElement.lang = app.config?.language ?? 'de'; });
</script>

<div class="shell">
  <div class="strip" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>{t('app.name')}</span>
    {#if !inTauri()}<span class="dev mono" data-tauri-drag-region>mock</span>{/if}
    <span class="ver mono" data-tauri-drag-region>{app.version}</span>
  </div>
  <div class="body">
    <Rail />
    <main>
      {#if !app.ready}
        <div class="loading" role="status"><span class="sr-only">{t('common.loading')}</span></div>
      {:else}
        {#if app.error}
          <div class="banner" role="alert">{errorText(app.error)}
            <button class="btn ghost" type="button" onclick={() => (app.error = null)}>{t('common.dismiss')}</button>
          </div>
        {/if}
        {#if nav.view === 'home'}<Home />{:else if nav.view === 'library'}<Library />{:else}<Settings />{/if}
      {/if}
    </main>
  </div>
  {#if nav.newProfile}<NewProfile />{/if}
</div>

<style>
  .shell { display: flex; flex-direction: column; height: 100%; min-width: 960px; min-height: 600px; background: var(--ink-0); }
  .strip { height: 32px; flex: none; display: flex; align-items: center; gap: 10px; padding: 0 14px; background: var(--ink-0); border-bottom: 1px solid var(--line-soft); font-size: 12px; color: var(--paper-dim); }
  .brand { font: 800 13px var(--font-display); color: var(--mango); letter-spacing: 0.02em; }
  .dev { padding: 0 6px; border: 1px solid var(--line); border-radius: 3px; }
  .ver { margin-left: auto; color: var(--paper-mute); }
  .body { flex: 1; display: flex; min-height: 0; }
  main { flex: 1; min-width: 0; position: relative; background: var(--ink-0); }
  .banner { position: absolute; z-index: 10; top: 8px; right: 20px; display: flex; align-items: center; gap: 8px; padding: 6px 6px 6px 14px; background: var(--ink-3); border: 1px solid var(--tomato); border-radius: var(--r-m); color: var(--paper); }
  .loading { height: 100%; }
</style>
