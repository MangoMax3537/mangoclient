<script lang="ts">
  import Skeleton from './skeleton/Skeleton.svelte';
  import SkeletonRegion from './skeleton/SkeletonRegion.svelte';
  import LoadError from './LoadError.svelte';
  import { BusyGate, createLoadState } from '../lib/loadState.svelte';
  import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
  import BroomIcon from 'phosphor-svelte/lib/BroomIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import MinusIcon from 'phosphor-svelte/lib/MinusIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import Seg from './Seg.svelte';
  import { inTauri } from '../lib/api';
  import { cleanDir, validateWatchDir } from '../lib/blocked';
  import { BACKUP_KEEP_MAX, BACKUP_KEEP_MIN, clampKeep, parseKeep } from '../lib/backups';
  import { pickFolder } from '../lib/pickImage';
  import { fmtBytes, fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { sourceOptions } from '../lib/install';
  import { actions, anyGameActive, app, applyTheme, errorText, getApi, lang, loadProviders, providers, t, theme, toast, updates, type ThemeMode } from '../lib/stores.svelte';
  import type { StorageKey, StorageUsage } from '../lib/types';

  const cfg = $derived(app.config);
  const maxRam = $derived(Math.max(2048, app.totalRamMb || 8192));
  let args = $state(app.config?.javaArgs ?? '');
  let argTimer: ReturnType<typeof setTimeout> | undefined;
  const usageLoad = createLoadState<StorageUsage[]>({ isEmpty: () => false });
  let usageOverride = $state<StorageUsage[] | null>(null);
  const usage = $derived(usageOverride ?? usageLoad.data);
  let clearing = $state<StorageKey | null>(null);
  let confirmKey = $state<StorageKey | null>(null);
  const totalBytes = $derived((usage ?? []).reduce((a, u) => a + u.bytes, 0));
  const gameActive = $derived(anyGameActive());
  const updGate = new BusyGate();
  $effect(() => { updGate.update(updates.status.state === 'checking'); return () => updGate.destroy(); });
  $effect(() => { usageLoad.run(() => getApi().storageUsage()); return () => usageLoad.cancel(); });
  async function clearKey(key: StorageKey) {
    clearing = key;
    try { usageOverride = await getApi().storageClear({ key }); toast(t('storage.cleared', { name: t(`storage.${key}` as Key) }), 'ok'); }
    catch (e) { toast(errorText(e), 'error'); }
    clearing = null; confirmKey = null;
  }
  const themeOpts = $derived((['system', 'dark', 'light'] as ThemeMode[]).map((m) => ({ value: m, label: t(`settings.theme.${m}` as Key) })));
  let cfKey = $state('');
  let cfErr = $state(false);
  const cf = $derived(sourceOptions(providers.status).find((o) => o.provider === 'curseforge'));
  $effect(() => { void loadProviders(); });
  async function saveKey(clear = false) {
    const k = clear ? '' : cfKey.trim();
    if (!clear && !/^[A-Za-z0-9$./_+=-]{16,128}$/.test(k)) { cfErr = true; return; }
    cfErr = false;
    await actions.setConfig({ curseforgeApiKey: clear ? null : k });
    cfKey = '';
    await loadProviders();
    toast(t(clear ? 'settings.cf.cleared' : 'settings.cf.saved'), 'ok');
  }
  let dirInput = $state('');
  let dirErr = $state<'invalid' | 'duplicate' | 'limit' | null>(null);
  const watchDirs = $derived(cfg?.blockedModsWatchDirs ?? []);
  async function addDir() {
    const err = validateWatchDir(watchDirs, dirInput);
    dirErr = err;
    if (err) return;
    await actions.setConfig({ blockedModsWatchDirs: [...watchDirs, cleanDir(dirInput)] });
    dirInput = '';
  }
  /** Tauri: the native folder dialog gives a real path; the browser keeps the text input. */
  async function browseDir() {
    let picked: string | null = null;
    try { picked = await pickFolder(t('settings.blocked.dialogTitle')); } catch (e) { toast(errorText(e), 'error'); return; }
    if (!picked) return;
    const err = validateWatchDir(watchDirs, picked);
    dirErr = err;
    if (err) return;
    await actions.setConfig({ blockedModsWatchDirs: [...watchDirs, cleanDir(picked)] });
  }
  const removeDir = (d: string) => actions.setConfig({ blockedModsWatchDirs: watchDirs.filter((x) => x !== d) });
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
  let keepText = $state(String(app.config?.backupKeep ?? 5));
  const keepBad = $derived(parseKeep(keepText) === null);
  /** Saves the field when it holds a valid whole number (1..50); an invalid one stays visible with its error and is never sent. */
  function commitKeep(final: boolean) {
    const n = parseKeep(keepText);
    if (n !== null && n !== cfg?.backupKeep) void actions.setConfig({ backupKeep: n });
    if (final && n !== null) keepText = String(n);
  }
  function stepKeep(d: number) {
    const n = clampKeep((parseKeep(keepText) ?? cfg?.backupKeep ?? 5) + d);
    keepText = String(n);
    commitKeep(true);
  }
  function onArgs() {
    clearTimeout(argTimer);
    argTimer = setTimeout(() => actions.setConfig({ javaArgs: args }), 400);
  }
</script>

{#if cfg}
  <div class="scroller"><div class="settings">
    <h1 class="display">{t('settings.title')}</h1>

    <section aria-labelledby="s-look">
      <h2 id="s-look" class="eyebrow">{t('settings.group.appearance')}</h2>
      <div class="row">
        <div class="txt"><span id="theme-l">{t('settings.theme')}</span><div class="hint">{t('settings.theme.hint')}</div></div>
        <Seg options={themeOpts} value={theme.mode} label={t('settings.theme')} idPrefix="theme" onchange={(v) => applyTheme(v as ThemeMode)} />
      </div>
    </section>

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

    <section aria-labelledby="s-src">
      <h2 id="s-src" class="eyebrow">{t('settings.group.sources')}</h2>
      <div class="row col">
        <div class="txt"><label for="cfkey">{t('settings.cf.key')}</label><div class="hint">{t('settings.cf.hint')}</div></div>
        <div class="keyrow">
          <input id="cfkey" class="field mono" type="password" bind:value={cfKey} placeholder={t('settings.cf.placeholder')} autocomplete="off" spellcheck="false"
            aria-invalid={cfErr} aria-describedby={cfErr ? 'cf-err' : undefined} onkeydown={(e) => e.key === 'Enter' && saveKey()} />
          <button class="btn accent" type="button" disabled={!cfKey.trim()} onclick={() => saveKey()}>{t('settings.cf.save')}</button>
          <button class="btn" type="button" disabled={cf?.reason === 'no_api_key'} onclick={() => saveKey(true)}>{t('settings.cf.clear')}</button>
        </div>
        {#if cfErr}<div id="cf-err" class="err" role="alert">{t('settings.cf.invalid')}</div>{/if}
        <div class="hint" role="status"><span class="sq" class:ok={cf && !cf.disabled} class:warn={cf?.disabled}></span> {cf?.disabled ? t(`settings.cf.status.${cf.reason ?? 'offline'}` as Key) : t('settings.cf.status.ok')}</div>
      </div>
    </section>

    <section aria-labelledby="s-blocked">
      <h2 id="s-blocked" class="eyebrow">{t('settings.group.blocked')}</h2>

      <div class="row">
        <div class="txt"><label for="bfall">{t('settings.blocked.fallback')}</label><div class="hint">{t('settings.blocked.fallbackHint')}</div></div>
        <span class="switch"><input id="bfall" type="checkbox" checked={cfg.blockedModsModrinthFallback} onchange={(e) => actions.setConfig({ blockedModsModrinthFallback: e.currentTarget.checked })} /><span></span></span>
      </div>

      <div class="row col">
        <div class="txt"><label for="bdir">{t('settings.blocked.dirs')}</label><div class="hint">{t('settings.blocked.dirsHint')}</div></div>
        {#if watchDirs.length}
          <ul class="dirs">
            {#each watchDirs as d (d)}
              <li><span class="mono" title={d}>{d}</span>
                <button class="btn ghost ic" type="button" aria-label={t('settings.blocked.remove', { dir: d })} onclick={() => removeDir(d)}><XIcon weight="bold" size={14} aria-hidden="true" /></button></li>
            {/each}
          </ul>
        {:else}
          <div class="hint warnline" role="status">{t('settings.blocked.noDirs')}</div>
        {/if}
        {#if inTauri()}
          <div class="keyrow">
            <button id="bdir" class="btn" type="button" onclick={browseDir}><PlusIcon weight="bold" size={14} aria-hidden="true" />{t('settings.blocked.browse')}</button>
          </div>
        {:else}
        <div class="keyrow">
          <input id="bdir" class="field mono" bind:value={dirInput} placeholder={t('settings.blocked.placeholder')} autocomplete="off" spellcheck="false"
            aria-invalid={dirErr !== null} aria-describedby={dirErr ? 'bdir-err' : undefined} oninput={() => (dirErr = null)} onkeydown={(e) => e.key === 'Enter' && addDir()} />
          <button class="btn" type="button" disabled={!dirInput.trim()} onclick={addDir}><PlusIcon weight="bold" size={14} aria-hidden="true" />{t('settings.blocked.add')}</button>
        </div>
        {/if}
        {#if dirErr}<div id="bdir-err" class="err" role="alert">{t(`settings.blocked.err.${dirErr}` as Key)}</div>{/if}
      </div>

      <div class="row">
        <div class="txt"><label for="bsub">{t('settings.blocked.sub')}</label><div class="hint">{t('settings.blocked.subHint')}</div></div>
        <span class="switch"><input id="bsub" type="checkbox" checked={cfg.blockedModsCheckSubfolders} onchange={(e) => actions.setConfig({ blockedModsCheckSubfolders: e.currentTarget.checked })} /><span></span></span>
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

    <section aria-labelledby="s-backups">
      <h2 id="s-backups" class="eyebrow">{t('settings.group.backups')}</h2>
      <div class="row">
        <div class="txt"><label for="asnap">{t('settings.autoSnapshot')}</label><div class="hint">{t('settings.autoSnapshotHint')}</div></div>
        <span class="switch"><input id="asnap" type="checkbox" checked={cfg.autoSnapshot} onchange={(e) => actions.setConfig({ autoSnapshot: e.currentTarget.checked })} /><span></span></span>
      </div>
      <div class="row">
        <div class="txt"><label for="bkeep">{t('settings.backupKeep')}</label><div class="hint">{t('settings.backupKeepHint')}</div></div>
        <div class="step">
          <div class="ctl">
            <button class="btn small" type="button" aria-label={t('settings.backupKeepLess')} disabled={cfg.backupKeep <= BACKUP_KEEP_MIN} onclick={() => stepKeep(-1)}><MinusIcon weight="bold" size={14} aria-hidden="true" /></button>
            <input id="bkeep" class="field mono" inputmode="numeric" autocomplete="off" value={keepText} aria-invalid={keepBad} aria-describedby={keepBad ? 'bkeep-err' : undefined}
              oninput={(e) => { keepText = e.currentTarget.value; commitKeep(false); }} onblur={() => commitKeep(true)} onkeydown={(e) => { if (e.key === 'ArrowUp') { e.preventDefault(); stepKeep(1); } else if (e.key === 'ArrowDown') { e.preventDefault(); stepKeep(-1); } }} />
            <button class="btn small" type="button" aria-label={t('settings.backupKeepMore')} disabled={cfg.backupKeep >= BACKUP_KEEP_MAX} onclick={() => stepKeep(1)}><PlusIcon weight="bold" size={14} aria-hidden="true" /></button>
          </div>
          {#if keepBad}<div id="bkeep-err" class="bad" role="alert">{t('settings.backupKeepInvalid')}</div>{/if}
        </div>
      </div>
    </section>

    <section aria-labelledby="s-storage">
      <h2 id="s-storage" class="eyebrow">{t('storage.title')}</h2>
      {#if usageLoad.phase === 'error' && !usage}
        <LoadError compact error={usageLoad.error} onretry={() => usageLoad.retry()} />
      {:else if !usage}
        {#if usageLoad.showSkeleton}
          <SkeletonRegion>
            <div class="totsk"><Skeleton variant="bar" lh={19} h={11} w={150} /></div>
            <ul class="usage">{#each Array(4) as _, i (i)}<li><Skeleton variant="bar" lh={22} h={13} w="64%" /><Skeleton variant="box" w="100%" h={8} radius={2} /><Skeleton variant="box" w={92} h={36} radius={8} /></li>{/each}</ul>
          </SkeletonRegion>
        {/if}
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
        <div class="txt"><b>{t('settings.version')} <span class="mono">{app.version}</span></b>{#if us.state === 'checking' && updGate.show}<Skeleton variant="bar" lh={20} h={11} w={180} />{:else}<div class="hint" role="status" aria-live="polite">{usText}</div>{/if}</div>
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
  </div></div>
{/if}

{#if confirmKey}
  <ConfirmDialog danger title={t('storage.confirmTitle', { name: t(`storage.${confirmKey}` as Key) })} body={t(`storage.hint.${confirmKey}` as Key)} confirmLabel={t('storage.clear')} busy={clearing !== null}
    onconfirm={() => confirmKey && clearKey(confirmKey)} oncancel={() => (confirmKey = null)} />
{/if}

<style>
  .scroller { height: 100%; overflow: auto; }
  .settings { padding: var(--s-5) 32px 40px; max-width: 800px; display: grid; gap: 24px; align-content: start; }
  h1 { font-size: var(--fs-3xl); }
  .keyrow { display: flex; gap: var(--s-2); }
  .step { display: grid; gap: 4px; justify-items: end; }
  .ctl { display: flex; gap: 6px; align-items: center; }
  .ctl .field { width: 64px; text-align: center; }
  .bad { color: var(--tomato); font-size: var(--fs-sm); width: 280px; text-align: right; }
  .dirs { list-style: none; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r-m); }
  .dirs li { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 2px 4px 2px 12px; border-bottom: 1px solid var(--line-soft); font-size: var(--fs-sm); }
  .dirs li:last-child { border-bottom: 0; }
  .dirs .mono { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .warnline { color: var(--mango); }
  .keyrow .field { flex: 1; }
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
  .totsk { margin: 6px 0 2px; }
</style>
