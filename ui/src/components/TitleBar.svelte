<script lang="ts">
  // The custom title bar. LEFT: Brand (click = Home) + Back / Forward, then the drag region. CENTER: the page or instance title.
  // RIGHT: update pill, activity, version and (Windows / Linux) Minimize / Maximize or Restore / Close. macOS keeps its native traffic
  // lights (left padding instead of buttons). In the plain browser there is no drag logic and no controls; the layout does not shift.
  import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import MinusIcon from 'phosphor-svelte/lib/MinusIcon';
  import SquareIcon from 'phosphor-svelte/lib/SquareIcon';
  import CopySimpleIcon from 'phosphor-svelte/lib/CopySimpleIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import Brand from './Brand.svelte';
  import Activity from './Activity.svelte';
  import UpdatePill from './UpdatePill.svelte';
  import { inHttp, inTauri } from '../lib/api';
  import { shortcutLabel } from '../lib/keybinds';
  import { stripMouseAction, titleBarModel, windowControls } from '../lib/titlebar';
  import { tip } from '../lib/tooltip';
  import type { Key } from '../lib/i18n';
  import { app, getApi, goView, hist, nav, navBack, navForward, platform, t, win } from '../lib/stores.svelte';

  const model = $derived(titleBarModel(platform.mode, win));
  const wc = () => windowControls(getApi());
  const MOD = platform.mac;
  const hint = (text: string, a: Parameters<typeof shortcutLabel>[0]): string => `${text} (${shortcutLabel(a, MOD)})`;

  const VIEW_KEY: Record<string, Key> = {
    home: 'nav.home', library: 'nav.library', discover: 'nav.discover', servers: 'nav.servers', skins: 'nav.skins', stats: 'nav.stats', settings: 'nav.settings',
  };
  const title = $derived(
    nav.view === 'profile' ? (app.profiles.find((p) => p.id === nav.profileId)?.name ?? t('nav.library')) : t(VIEW_KEY[nav.view] ?? 'nav.home'),
  );

  function down(e: MouseEvent) {
    const a = stripMouseAction(e, model.dragRegion);
    if (a === 'none') return;
    e.preventDefault();
    if (a === 'toggle') void wc().toggleMaximize(); else void wc().startDrag();
  }
</script>

<header class="strip" class:dim={model.dimmed} data-mode={model.mode} style:--inset="{model.leftInset}px" onmousedown={down} role="presentation">
  <div class="left">
    <button class="brandbtn" type="button" use:tip={{ text: t('title.home'), side: 'bottom' }} aria-label={t('title.home')} onclick={() => goView('home')}><Brand /></button>
    <div class="arrows" role="group" aria-label={t('title.history')}>
      <button class="ab" type="button" disabled={!hist.back} use:tip={{ text: hint(t('keys.back'), 'back'), side: 'bottom' }} aria-label={t('keys.back')} onclick={navBack}><ArrowLeftIcon weight="bold" size={16} aria-hidden="true" /></button>
      <button class="ab" type="button" disabled={!hist.forward} use:tip={{ text: hint(t('keys.forward'), 'forward'), side: 'bottom' }} aria-label={t('keys.forward')} onclick={navForward}><ArrowRightIcon weight="bold" size={16} aria-hidden="true" /></button>
    </div>
    {#if !inTauri() && model.mode === 'browser'}<span class="dev mono">{inHttp() ? 'dev server' : 'mock'}</span>{/if}
  </div>
  <div class="mid" aria-live="polite"><span class="ttl">{title}</span></div>
  <div class="right">
    <UpdatePill />
    <Activity />
    <span class="ver mono">{app.version}</span>
    {#if model.showControls}
      <div class="wc" role="group" aria-label={t('title.window')}>
        <button class="c" type="button" use:tip={{ text: t('title.minimize'), side: 'bottom' }} aria-label={t('title.minimize')} onclick={() => wc().minimize()}><MinusIcon weight="bold" size={14} aria-hidden="true" /></button>
        <button class="c" type="button" use:tip={{ text: model.maxIcon === 'restore' ? t('title.restore') : t('title.maximize'), side: 'bottom' }} aria-label={model.maxIcon === 'restore' ? t('title.restore') : t('title.maximize')} onclick={() => wc().toggleMaximize()}>
          {#if model.maxIcon === 'restore'}<CopySimpleIcon weight="bold" size={14} aria-hidden="true" />{:else}<SquareIcon weight="bold" size={13} aria-hidden="true" />{/if}
        </button>
        <button class="c close" type="button" use:tip={{ text: t('title.close'), side: 'bottom' }} aria-label={t('title.close')} onclick={() => wc().close()}><XIcon weight="bold" size={15} aria-hidden="true" /></button>
      </div>
    {/if}
  </div>
</header>

<style>
  .strip {
    position: relative; z-index: 20; height: 36px; flex: none; display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, auto) minmax(0, 1fr); align-items: center;
    padding: 0 0 0 calc(10px + var(--inset)); background: var(--ink-0); border-bottom: 1px solid var(--line-soft); font-size: 12px; color: var(--paper-dim);
    user-select: none; -webkit-user-select: none;
  }
  .strip[data-mode='browser'], .strip[data-mode='mac'] { padding-right: 14px; }
  .left { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .brandbtn { display: inline-flex; align-items: center; height: 28px; padding: 0 8px; border-radius: var(--r-s); transition: background var(--dur) var(--ease); }
  .brandbtn:hover { background: var(--ink-3); }
  .arrows { display: flex; gap: 2px; }
  .ab { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r-s); color: var(--paper-dim); transition: background var(--dur) var(--ease), color var(--dur) var(--ease); }
  .ab:hover:not(:disabled) { background: var(--ink-3); color: var(--paper); }
  .ab:disabled { opacity: 0.35; }
  .dev { padding: 0 6px; border: 1px solid var(--line); border-radius: 3px; }
  .mid { min-width: 0; max-width: 40vw; text-align: center; padding: 0 12px; }
  .ttl { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; color: var(--paper-dim); }
  .right { display: flex; align-items: center; justify-content: flex-end; gap: 10px; min-width: 0; height: 100%; }
  .ver { color: var(--paper-mute); }
  .wc { display: flex; height: 100%; margin-left: 4px; }
  .c { width: 46px; height: 100%; display: grid; place-items: center; color: var(--paper-dim); transition: background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease); }
  .c:hover { background: var(--ink-3); color: var(--paper); }
  .c:focus-visible { outline-offset: -3px; border-radius: 0; }
  .c.close:hover { background: #c42b1c; color: #fff; }
  .dim .c, .dim .ttl, .dim .ver { opacity: 0.55; }
  button { cursor: default; }
</style>
