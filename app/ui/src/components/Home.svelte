<script lang="ts">
  import ClockIcon from 'phosphor-svelte/lib/ClockIcon';
  import CoffeeIcon from 'phosphor-svelte/lib/CoffeeIcon';
  import MemoryIcon from 'phosphor-svelte/lib/MemoryIcon';
  import CubeIcon from 'phosphor-svelte/lib/CubeIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import Tile from './Tile.svelte';
  import PlayButton from './PlayButton.svelte';
  import LaunchProgress from './LaunchProgress.svelte';
  import Console from './Console.svelte';
  import ProfileMenu from './ProfileMenu.svelte';
  import { heroFallback } from '../lib/fallbackImage';
  import { fmtDuration, fmtRam, fmtRelative } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import {
    actions, app, consoles, isBusy, isRunning, lang, launchInfo, nav, selectedProfile, t, tp,
  } from '../lib/stores.svelte';

  const p = $derived(selectedProfile());
  const info = $derived(p ? launchInfo(p.id) : null);
  const busy = $derived(p ? isBusy(p.id) : false);
  const running = $derived(p ? isRunning(p.id) : false);
  const hero = $derived(p?.coverUrl ?? heroFallback(p?.id));
  const ram = $derived(p ? (p.ram ?? app.config?.ram ?? 0) : 0);
  const loaderLabel = $derived(p ? (p.loader === 'vanilla' ? t('profile.vanilla') : `${p.loader} ${p.loaderVersion ?? ''}`.trim()) : '');
  const showConsole = $derived(p ? busy || (consoles[p.id]?.length ?? 0) > 0 : false);
  const others = $derived([...app.profiles].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)).slice(0, 6));

  function toggle() {
    if (!p) return;
    if (busy) actions.stop(p.id);
    else actions.play(p.id);
  }
</script>

<div class="home">
  {#if p && info}
    <section class="hero" style:--cover="url({hero})" style:--tint={p.color ?? 'var(--mango)'}>
      <div class="art px" aria-hidden="true"></div>
      <div class="scrim" aria-hidden="true"></div>
      <div class="content">
        <div class="eyebrow">
          {#if running}<span class="live">{t('home.running')}</span>{:else if busy}{t(`launch.${info.state}` as Key)}{:else}{t('home.lastPlayed')}{#if p.lastPlayed}&nbsp;&middot;&nbsp;{fmtRelative(p.lastPlayed, lang())}{:else}&nbsp;&middot;&nbsp;{t('home.never')}{/if}{/if}
        </div>
        <h1>{p.name}</h1>
        <ul class="chips" aria-label={p.name}>
          <li class="chip mono"><CubeIcon weight="bold" size={14} aria-hidden="true" />{p.mcVersion}</li>
          <li class="chip">{loaderLabel}</li>
          <li class="chip mono"><CoffeeIcon weight="bold" size={14} aria-hidden="true" />{t('chip.java')} {t('chip.auto')}</li>
          <li class="chip mono"><MemoryIcon weight="bold" size={14} aria-hidden="true" />{ram ? fmtRam(ram) : t('chip.default')}</li>
          <li class="chip mono"><ClockIcon weight="bold" size={14} aria-hidden="true" />{fmtDuration(p.playTimeMs)}</li>
          {#if p.modCount}<li class="chip">{tp('profile.mods', p.modCount)}</li>{/if}
        </ul>
        <div class="actions">
          <PlayButton mode={busy ? 'stop' : 'play'} label={busy ? t('home.stop') : t('home.play')} onclick={toggle} disabled={!app.accounts.length && !busy} />
          <ProfileMenu currentId={p.id} disabled={busy} />
        </div>
        {#if busy && info.state !== 'running'}
          <LaunchProgress profileId={p.id} />
        {/if}
      </div>
    </section>

    {#if showConsole}<Console profileId={p.id} />{/if}

    <section class="recent" aria-labelledby="recent-h">
      <header>
        <h2 id="recent-h" class="eyebrow">{t('home.continue')}</h2>
        <button class="btn ghost" type="button" onclick={() => (nav.view = 'library')}>{t('nav.library')}<ArrowRightIcon weight="bold" size={14} aria-hidden="true" /></button>
      </header>
      <table>
        <thead>
          <tr><th scope="col">{t('col.name')}</th><th scope="col">{t('col.version')}</th><th scope="col">{t('col.loader')}</th><th scope="col" class="r">{t('col.time')}</th><th scope="col"><span class="sr-only">{t('home.play')}</span></th></tr>
        </thead>
        <tbody>
          {#each others as o (o.id)}
            <tr class:sel={o.id === p.id}>
              <td>
                <button class="row-name" type="button" onclick={() => actions.selectProfile(o.id)}>
                  <Tile kind="profile" id={o.id} src={o.coverUrl} size={32} radius={4} />
                  <span>{o.name}</span>
                </button>
              </td>
              <td class="mono">{o.mcVersion}</td>
              <td>{o.loader === 'vanilla' ? t('profile.vanilla') : o.loader}</td>
              <td class="mono r">{fmtDuration(o.playTimeMs)}</td>
              <td class="r">
                <button class="btn ghost icon" type="button" disabled={isBusy(o.id) || !app.accounts.length} aria-label="{t('home.play')}: {o.name}"
                  onclick={async () => { await actions.selectProfile(o.id); actions.play(o.id); }}>
                  <PlayIcon weight="bold" size={16} aria-hidden="true" />
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {:else}
    <section class="empty">
      <h1>{t('home.noProfiles')}</h1>
      <p class="hint">{t('home.noProfilesHint')}</p>
      <button class="btn accent" type="button" onclick={() => (nav.newProfile = true)}><PlusIcon weight="bold" size={16} aria-hidden="true" />{t('library.new')}</button>
    </section>
  {/if}
</div>

<style>
  .home { display: grid; gap: 16px; padding: 16px 20px 24px; align-content: start; overflow: auto; height: 100%; }
  .hero { position: relative; min-height: 340px; border-radius: var(--r-l); overflow: hidden; display: flex; align-items: flex-end; border: 1px solid var(--line); }
  .art { position: absolute; inset: 0; background: var(--cover) center / cover no-repeat; transform: scale(1.02); }
  .scrim { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(11, 12, 14, 0.94) 0%, rgba(11, 12, 14, 0.7) 45%, rgba(11, 12, 14, 0.15) 100%), linear-gradient(0deg, rgba(11, 12, 14, 0.7), transparent 50%); }
  .scrim::after { content: ''; position: absolute; left: 0; top: 0; bottom: 0; width: 4px; background: var(--tint); }
  .content { position: relative; display: grid; gap: 14px; padding: 28px 32px 28px 36px; width: 100%; }
  h1 { font-size: clamp(34px, 5vw, 56px); line-height: 1; max-width: 18ch; }
  .live { color: var(--leaf); }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; list-style: none; margin: 0; padding: 0; }
  .actions { display: flex; gap: 12px; align-items: center; }
  .recent header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px; }
  table { width: 100%; border-collapse: collapse; }
  th { text-align: left; font-size: 12px; font-weight: 600; color: var(--paper-mute); padding: 6px 10px; border-bottom: 1px solid var(--line); }
  td { padding: 6px 10px; border-bottom: 1px solid var(--line-soft); font-size: 14px; }
  .r { text-align: right; }
  tr.sel td { background: rgba(255, 173, 50, 0.06); }
  tbody tr:hover td { background: var(--ink-2); }
  .row-name { display: flex; align-items: center; gap: 12px; font-weight: 500; }
  .icon { width: 32px; height: 32px; padding: 0; justify-content: center; }
  .empty { display: grid; gap: 12px; justify-items: start; padding: 64px 32px; }
</style>
