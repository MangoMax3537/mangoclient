<script lang="ts">
  import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
  import CubeIcon from 'phosphor-svelte/lib/CubeIcon';
  import CoffeeIcon from 'phosphor-svelte/lib/CoffeeIcon';
  import MemoryIcon from 'phosphor-svelte/lib/MemoryIcon';
  import ClockIcon from 'phosphor-svelte/lib/ClockIcon';
  import PlayButton from './PlayButton.svelte';
  import Seg from './Seg.svelte';
  import Overview from './profile/Overview.svelte';
  import Content from './profile/Content.svelte';
  import Media from './profile/Media.svelte';
  import Logs from './profile/Logs.svelte';
  import ProfileSettings from './profile/ProfileSettings.svelte';
  import { heroFallback } from '../lib/fallbackImage';
  import { fmtDuration, fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { actions, app, isBusy, nav, t, type ProfileTab } from '../lib/stores.svelte';

  let { id }: { id: string } = $props();
  const p = $derived(app.profiles.find((x) => x.id === id) ?? null);
  const busy = $derived(isBusy(id));
  const TABS: ProfileTab[] = ['overview', 'content', 'media', 'logs', 'settings'];
  const opts = $derived(TABS.map((k) => ({ value: k, label: t(`profile.tab.${k}` as Key) })));
  const ram = $derived(p ? (p.ram ?? app.config?.ram ?? 0) : 0);
  const loaderLabel = $derived(p ? (p.loader === 'vanilla' ? t('profile.vanilla') : `${p.loader} ${p.loaderVersion ?? ''}`.trim()) : '');

  $effect(() => { if (!p && app.ready) { nav.view = 'library'; } });
  function toggle() {
    if (!p) return;
    if (busy) actions.stop(p.id);
    else { actions.selectProfile(p.id); actions.play(p.id); }
  }
</script>

{#if p}
  <div class="page">
    <section class="hero" class:compact={nav.profileTab !== 'overview'} style:--cover="url({p.coverUrl ?? heroFallback(p.id)})" style:--tint={p.color ?? 'var(--mango)'}>
      <div class="art px" aria-hidden="true"></div>
      <div class="scrim" aria-hidden="true"></div>
      <div class="inner">
        <button class="btn ghost back" type="button" onclick={() => (nav.view = 'library')}><ArrowLeftIcon weight="bold" size={14} aria-hidden="true" />{t('nav.library')}</button>
        <div class="row">
          <div class="id">
            <h1>{p.name}</h1>
            <ul class="chips" aria-label={p.name}>
              <li class="chip mono"><CubeIcon weight="bold" size={14} aria-hidden="true" />{p.mcVersion}</li>
              <li class="chip">{loaderLabel}</li>
              <li class="chip mono"><CoffeeIcon weight="bold" size={14} aria-hidden="true" />{t('chip.java')} {t('chip.auto')}</li>
              <li class="chip mono"><MemoryIcon weight="bold" size={14} aria-hidden="true" />{ram ? fmtRam(ram) : t('chip.default')}</li>
              <li class="chip mono"><ClockIcon weight="bold" size={14} aria-hidden="true" />{fmtDuration(p.playTimeMs)}</li>
            </ul>
          </div>
          <PlayButton mode={busy ? 'stop' : 'play'} label={busy ? t('home.stop') : t('home.play')} onclick={toggle} disabled={!app.accounts.length && !busy} />
        </div>
      </div>
    </section>

    <div class="tabs"><Seg options={opts} value={nav.profileTab} label={p.name} role="tablist" idPrefix="ptab" onchange={(v) => (nav.profileTab = v as ProfileTab)} /></div>

    <div class="panel" role="tabpanel" aria-labelledby="ptab-{nav.profileTab}">
      {#key p.id}
        {#if nav.profileTab === 'overview'}<Overview profile={p} />
        {:else if nav.profileTab === 'content'}<Content profile={p} />
        {:else if nav.profileTab === 'media'}<Media profile={p} />
        {:else if nav.profileTab === 'logs'}<Logs profile={p} />
        {:else}<ProfileSettings profile={p} />{/if}
      {/key}
    </div>
  </div>
{/if}

<style>
  .page { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 16px 20px 0; gap: 12px; }
  .hero { position: relative; flex: none; min-height: 168px; border-radius: var(--r-l); overflow: hidden; border: 1px solid var(--line); display: flex; }
  .art { position: absolute; inset: 0; background: var(--cover) center / cover no-repeat; }
  .scrim { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(11, 12, 14, 0.94) 0%, rgba(11, 12, 14, 0.72) 50%, rgba(11, 12, 14, 0.2) 100%); }
  .scrim::after { content: ''; position: absolute; left: 0; top: 0; bottom: 0; width: 4px; background: var(--tint); }
  .inner { position: relative; width: 100%; display: flex; flex-direction: column; justify-content: space-between; padding: 10px 24px 18px 28px; gap: 12px; }
  .back { align-self: flex-start; height: 28px; margin-left: -10px; }
  .row { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
  .id { display: grid; gap: 10px; min-width: 0; }
  h1 { font-size: clamp(28px, 4vw, 42px); line-height: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; list-style: none; margin: 0; padding: 0; }
  .hero.compact { min-height: 0; }
  .hero.compact .back, .hero.compact .chips { display: none; }
  .hero.compact .inner { padding: 14px 20px 14px 28px; }
  .hero.compact h1 { font-size: 26px; }
  .tabs { flex: none; }
  .panel { flex: 1; min-height: 0; overflow: auto; padding: 4px 0 20px; }
</style>
