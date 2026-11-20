<script lang="ts">
  import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
  import BroomIcon from 'phosphor-svelte/lib/BroomIcon';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import { fmtBytes, fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { actions, anyGameActive, app, errorText, getApi, lang, t, toast, updates } from '../lib/stores.svelte';
  import type { StorageKey, StorageUsage } from '../lib/types';

  const cfg = $derived(app.config);
  const maxRam = $derived(Math.max(2048, app.totalRamMb || 8192));
  let args = $state(app.config?.javaArgs ?? '');
  let argTimer: ReturnType<typeof setTimeout> | undefined;
  let usage = $state<StorageUsage[] | null>(null);
  let clearing = $state<StorageKey | null>(null);
  let confirmKey = $state<StorageKey | null>(null);
  const totalBytes = $derived((usage ?? []).reduce((a, u) => a + u.bytes, 0));
  const gameActive = $derived(anyGameActive());
  $effect(() => { getApi().storageUsage().then((u) => (usage = u), (e) => toast(errorText(e), 'error')); });
  async function clearKey(key: StorageKey) {
    clearing = key;
    try { usage = await getApi().storageClear({ key }); toast(t('storage.cleared', { name: t(`storage.${key}` as Key) }), 'ok'); }
    catch (e) { toast(errorText(e), 'error'); }
    clearing = null; confirmKey = null;
  }
  const us = $derived(updates.status);
  const usText = $derived(
    us.state === 'checking' ? t('update.status.checking')
      : us.state === 'downloading' ? t('update.status.downloading', { n: Math.round(us.percent ?? 0), v: us.version ?? '' })
      : us.state === 'ready' ? t('update.status.ready', { v: us.version ?? '' })
      : us.state === 'current' ? t('update.status.current')
      : us.state === 'error' ? t(`update.error.${us.errorCode ?? 'other'}` as Key)
      : us.state === 'disabled' ? t(`update.disabled.${us.disabledReason ?? 'dev'}` as Key)
      : t('update.status.idle'),
  );
  function onArgs() {
    clearTimeout(argTimer);
    argTimer = setTimeout(() => actions.setConfig({ javaArgs: args }), 400);
  }
</script>

{#if cfg}
  <div class="settings">
    <h1>{t('settings.title')}</h1>

    <section aria-labelledby="s-launcher">
      <h2 id="s-launcher" class="eyebrow">{t('settings.group.general')}</h2>

      <div class="row">
        <div class="txt"><label for="lang">{t('settings.language')}</label></div>
        <select id="lang" class="field" value={cfg.language} onchange={(e) => actions.setConfig({ language: e.currentTarget.value as 'de' | 'en' })}>
          <option value="de">{t('settings.language.de')}</option>
          <option value="en">{t('settings.language.en')}</option>
        </select>
      </div>

      <div class="row">
        <div class="txt"><label for="keep">{t('settings.keepOpen')}</label><div class="hint">{t('settings.keepOpenHint')}</div></div>
        <span class="switch"><input id="keep" type="checkbox" checked={cfg.keepLauncherOpen} onchange={(e) => actions.setConfig({ keepLauncherOpen: e.currentTarget.checked })} /><span></span></span>
      </div>

      <div class="row">
        <div class="txt"><label for="tele">{t('settings.telemetry')}</label><div class="hint">{t('settings.telemetryHint')}</div></div>
        <span class="switch"><input id="tele" type="checkbox" checked={cfg.telemetry} onchange={(e) => actions.setConfig({ telemetry: e.currentTarget.checked })} /><span></span></span>
      </div>
    </section>

    <section aria-labelledby="s-game">
      <h2 id="s-game" class="eyebrow">{t('settings.group.game')}</h2>

      <div class="row col">
        <div class="txt"><label for="ram">{t('settings.ram')}</label></div>
        <div class="ram">
          <input id="ram" type="range" min="1024" max={maxRam} step="512" value={cfg.ram} aria-valuetext={fmtRam(cfg.ram)}
            oninput={(e) => actions.setConfig({ ram: Number(e.currentTarget.value) })} />
          <output class="mono"><b>{fmtRam(cfg.ram)}</b> <span class="hint">{t('settings.ramOf', { total: fmtRam(app.totalRamMb) })}</span></output>
        </div>
      </div>

      <div class="row col">
        <div class="txt"><label for="jargs">{t('settings.javaArgs')}</label><div class="hint">{t('settings.javaArgsHint')}</div></div>
        <input id="jargs" class="field mono" bind:value={args} oninput={onArgs} spellcheck="false" autocomplete="off" />
      </div>
    </section>

    <section aria-labelledby="s-storage">
      <h2 id="s-storage" class="eyebrow">{t('storage.title')}</h2>
      {#if !usage}
        <p class="hint">{t('common.loading')}</p>
      {:else}
        <p class="hint total mono">{t('storage.total', { size: fmtBytes(totalBytes, lang()) })}</p>
        <ul class="usage">
          {#each usage as u (u.key)}
            <li>
              <div class="lbl"><span>{t(`storage.${u.key}` as Key)}</span><span class="mono dim">{fmtBytes(u.bytes, lang())}</span></div>
              <div class="bar" role="meter" aria-label={t(`storage.${u.key}` as Key)} aria-valuemin="0" aria-valuemax={totalBytes} aria-valuenow={u.bytes}><span style:transform="scaleX({totalBytes ? u.bytes / totalBytes : 0})"></span></div>
              <button class="btn" type="button" disabled={!u.clearable || gameActive || u.bytes === 0 || clearing !== null} title={gameActive ? t('storage.gameRunning') : ''} onclick={() => (confirmKey = u.key)}>
                <BroomIcon weight="bold" size={14} aria-hidden="true" />{t('storage.clear')}
              </button>
            </li>
          {/each}
        </ul>
        {#if gameActive}<p class="hint">{t('storage.gameRunning')}</p>{/if}
      {/if}
    </section>

    <section aria-labelledby="s-updates">
      <h2 id="s-updates" class="eyebrow">{t('update.title')}</h2>
      <div class="row">
        <div class="txt"><b>{t('settings.version')} <span class="mono">{app.version}</span></b><div class="hint" role="status" aria-live="polite">{usText}</div></div>
        {#if us.state === 'ready'}
          <button class="btn accent" type="button" onclick={() => (updates.overlay = true)}>{t('update.install')}</button>
        {:else}
          <button class="btn" type="button" disabled={us.state === 'checking' || us.state === 'downloading' || us.state === 'disabled'} onclick={() => actions.updateCheck()}>
            <ArrowsClockwiseIcon weight="bold" size={16} aria-hidden="true" />{t('update.check')}
          </button>
        {/if}
      </div>
    </section>

    <footer class="hint mono">{t('settings.version')} {app.version} &middot; {t('settings.dataDir')}: {app.rootDir}</footer>
  </div>
{/if}

{#if confirmKey}
  <ConfirmDialog danger title={t('storage.confirmTitle', { name: t(`storage.${confirmKey}` as Key) })} body={t(`storage.hint.${confirmKey}` as Key)} confirmLabel={t('storage.clear')} busy={clearing !== null}
    onconfirm={() => confirmKey && clearKey(confirmKey)} oncancel={() => (confirmKey = null)} />
{/if}

<style>
  .settings { padding: 20px 28px 32px; max-width: 760px; display: grid; gap: 24px; height: 100%; overflow: auto; align-content: start; }
  h1 { font-size: 34px; }
  section { display: grid; border-top: 1px solid var(--line); }
  section h2 { padding: 14px 0 6px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 12px 0; border-bottom: 1px solid var(--line-soft); }
  .row.col { flex-direction: column; align-items: stretch; gap: 8px; }
  .txt label { font-weight: 500; }
  .ram { display: flex; align-items: center; gap: 16px; }
  .ram output { white-space: nowrap; font-size: 13px; }
  .ram b { color: var(--mango); font-weight: 500; }
  footer { word-break: break-all; }
  .usage { list-style: none; margin: 0; padding: 0; display: grid; }
  .usage li { display: grid; grid-template-columns: minmax(0, 1fr) 36% auto; gap: 16px; align-items: center; padding: 8px 0; border-bottom: 1px solid var(--line-soft); }
  .lbl { display: flex; justify-content: space-between; gap: 8px; }
  .dim { color: var(--paper-mute); }
  .bar { height: 8px; background: var(--ink-3); border-radius: 2px; overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--mango); transform-origin: left; }
  .total { margin: 6px 0 2px; }
</style>
