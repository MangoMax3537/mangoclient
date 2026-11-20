<script lang="ts">
  import Skeleton from './skeleton/Skeleton.svelte';
  import SkeletonRegion from './skeleton/SkeletonRegion.svelte';
  import LoadError from './LoadError.svelte';
  import { BusyGate, createLoadState } from '../lib/loadState.svelte';
  import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
  import BroomIcon from 'phosphor-svelte/lib/BroomIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import MinusIcon from 'phosphor-svelte/lib/MinusIcon';
  import UsersIcon from 'phosphor-svelte/lib/UsersIcon';
  import RamControl from './RamControl.svelte';
  import Brand from './Brand.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import Seg from './Seg.svelte';
  import Toggle from './Toggle.svelte';
  import SettingRow from './SettingRow.svelte';
  import SettingSection from './SettingSection.svelte';
  import SettingsSearch from './SettingsSearch.svelte';
  import { BACKUP_KEEP_MAX, BACKUP_KEEP_MIN, clampKeep, parseKeep } from '../lib/backups';
  import { fmtBytes, fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { entriesFor } from '../lib/settingsIndex';
  import { jumpTo } from '../lib/settingsJump';
  import { parseWindowSize } from '../lib/windowSize';
  import { actions, anyGameActive, app, applyTheme, errorText, getApi, lang, nav, setCycleBackgrounds, t, theme, toast, ui, updates, type ThemeMode } from '../lib/stores.svelte';
  import type { StorageKey, StorageUsage } from '../lib/types';

  const cfg = $derived(app.config);
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

  /* settings search: while a query is typed only the matching rows (and the sections that hold them) are shown */
  let matches = $state<Set<string> | null>(null);
  const show = (id: string) => matches === null || matches.has(id);
  const appEntries = entriesFor('app');
  const sec = (id: string) => matches === null || appEntries.some((e) => e.section === id && matches?.has(e.id));

  /* window size */
  let wText = $state(String(app.config?.width ?? 1280));
  let hText = $state(String(app.config?.height ?? 720));
  const size = $derived(parseWindowSize(wText, hText));
  function commitSize() {
    if (size && (size.width !== cfg?.width || size.height !== cfg?.height)) void actions.setConfig({ width: size.width, height: size.height });
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
    <SettingsSearch scope="app" bind:matches onpick={(e) => jumpTo(e)} />

    <div class="set-sections">
      {#if sec('look')}
        <SettingSection id="look" title={t('settings.group.appearance')}>
          {#if show('theme')}
            <SettingRow id="theme" label={t('settings.theme')} hint={t('settings.theme.hint')}>
              <Seg options={themeOpts} value={theme.mode} label={t('settings.theme')} idPrefix="theme" onchange={(v) => applyTheme(v as ThemeMode)} />
            </SettingRow>
          {/if}
          {#if show('cycleBg')}
            <SettingRow id="cycleBg" asLabel label={t('settings.cycleBg')} hint={t('settings.cycleBgHint')}>
              <Toggle checked={ui.cycleBackgrounds} onchange={(on) => setCycleBackgrounds(on)} />
            </SettingRow>
          {/if}
        </SettingSection>
      {/if}

      {#if sec('launcher')}
        <SettingSection id="launcher" title={t('settings.group.general')}>
          {#if show('language')}
            <SettingRow id="language" label={t('settings.language')} forId="lang">
              <select id="lang" class="field" value={cfg.language} onchange={(e) => actions.setConfig({ language: e.currentTarget.value as 'de' | 'en' })}>
                <option value="de">{t('settings.language.de')}</option>
                <option value="en">{t('settings.language.en')}</option>
              </select>
            </SettingRow>
          {/if}
          {#if show('keepOpen')}
            <SettingRow id="keepOpen" asLabel label={t('settings.keepOpen')} hint={t('settings.keepOpenHint')}>
              <Toggle checked={cfg.keepLauncherOpen} onchange={(on) => actions.setConfig({ keepLauncherOpen: on })} />
            </SettingRow>
          {/if}
          {#if show('telemetry')}
            <SettingRow id="telemetry" asLabel label={t('settings.telemetry')} hint={t('settings.telemetryHint')}>
              <Toggle checked={cfg.telemetry} onchange={(on) => actions.setConfig({ telemetry: on })} />
            </SettingRow>
          {/if}
        </SettingSection>
      {/if}

      {#if sec('accounts')}
        <SettingSection id="accounts" title={t('nav.accounts')}>
          {#if show('accounts')}
            <SettingRow id="accounts" label={t('nav.accounts')} hint={t('settings.accounts.hint')}>
              <button class="btn" type="button" onclick={(e) => { e.stopPropagation(); nav.accounts = true; }}><UsersIcon weight="bold" size={16} aria-hidden="true" />{t('settings.accounts.manage')}</button>
            </SettingRow>
          {/if}
        </SettingSection>
      {/if}

      {#if sec('game')}
        <SettingSection id="game" title={t('settings.group.game')}>
          {#if show('ram')}
            <SettingRow id="ram" stack label={t('settings.ram')} hint={t('settings.ramHint')}>
              <RamControl id="ram" mode="global" value={cfg.ram} limits={app.ramLimits} defaultMb={app.defaultRamMb} onsave={(ram) => ram !== null && actions.setConfig({ ram })} />
              {#snippet extra()}<span class="hint">{t('settings.ramTable')}</span><span class="hint">{t('settings.ramSuggestion', { total: fmtRam(app.totalRamMb), ram: fmtRam(app.defaultRamMb) })}</span>{/snippet}
            </SettingRow>
          {/if}
          {#if show('javaArgs')}
            <SettingRow id="javaArgs" stack label={t('settings.javaArgs')} hint={t('settings.javaArgsHint')} forId="jargs">
              <input id="jargs" class="field mono" bind:value={args} oninput={onArgs} spellcheck="false" autocomplete="off" />
            </SettingRow>
          {/if}
          {#if show('fullscreen')}
            <SettingRow id="fullscreen" asLabel label={t('settings.fullscreen')} hint={t('settings.fullscreenHint')}>
              <Toggle checked={cfg.fullscreen} onchange={(on) => actions.setConfig({ fullscreen: on })} />
            </SettingRow>
          {/if}
          {#if show('windowSize')}
            <SettingRow id="windowSize" label={t('settings.windowSize')} hint={t('settings.windowSizeHint')}>
              <div class="wsz">
                <div class="dims">
                  <input id="wsz-w" class="field mono" inputmode="numeric" autocomplete="off" aria-label={t('settings.width')} aria-invalid={!size} value={wText}
                    disabled={cfg.fullscreen} oninput={(e) => (wText = e.currentTarget.value)} onblur={commitSize} onkeydown={(e) => e.key === 'Enter' && commitSize()} />
                  <span class="x" aria-hidden="true">&times;</span>
                  <input id="wsz-h" class="field mono" inputmode="numeric" autocomplete="off" aria-label={t('settings.height')} aria-invalid={!size} value={hText}
                    disabled={cfg.fullscreen} oninput={(e) => (hText = e.currentTarget.value)} onblur={commitSize} onkeydown={(e) => e.key === 'Enter' && commitSize()} />
                </div>
                {#if !size}<div class="bad" role="alert">{t('settings.windowSizeInvalid')}</div>{/if}
              </div>
            </SettingRow>
          {/if}
        </SettingSection>
      {/if}

      {#if sec('backups')}
        <SettingSection id="backups" title={t('settings.group.backups')}>
          {#if show('autoSnapshot')}
            <SettingRow id="autoSnapshot" asLabel label={t('settings.autoSnapshot')} hint={t('settings.autoSnapshotHint')}>
              <Toggle checked={cfg.autoSnapshot} onchange={(on) => actions.setConfig({ autoSnapshot: on })} />
            </SettingRow>
          {/if}
          {#if show('backupKeep')}
            <SettingRow id="backupKeep" label={t('settings.backupKeep')} hint={t('settings.backupKeepHint')} forId="bkeep">
              <div class="step">
                <div class="ctl">
                  <button class="btn small" type="button" aria-label={t('settings.backupKeepLess')} disabled={cfg.backupKeep <= BACKUP_KEEP_MIN} onclick={() => stepKeep(-1)}><MinusIcon weight="bold" size={14} aria-hidden="true" /></button>
                  <input id="bkeep" class="field mono" inputmode="numeric" autocomplete="off" value={keepText} aria-invalid={keepBad} aria-describedby={keepBad ? 'bkeep-err' : undefined}
                    oninput={(e) => { keepText = e.currentTarget.value; commitKeep(false); }} onblur={() => commitKeep(true)} onkeydown={(e) => { if (e.key === 'ArrowUp') { e.preventDefault(); stepKeep(1); } else if (e.key === 'ArrowDown') { e.preventDefault(); stepKeep(-1); } }} />
                  <button class="btn small" type="button" aria-label={t('settings.backupKeepMore')} disabled={cfg.backupKeep >= BACKUP_KEEP_MAX} onclick={() => stepKeep(1)}><PlusIcon weight="bold" size={14} aria-hidden="true" /></button>
                </div>
                {#if keepBad}<div id="bkeep-err" class="bad" role="alert">{t('settings.backupKeepInvalid')}</div>{/if}
              </div>
            </SettingRow>
          {/if}
        </SettingSection>
      {/if}

      {#if sec('storage')}
        <SettingSection id="storage" title={t('storage.title')}>
          <div class="set-row stack" id="set-storage">
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
              <ul class="usage set-rows">
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
          </div>
        </SettingSection>
      {/if}

      {#if sec('updates')}
        <SettingSection id="updates" title={t('update.title')}>
          <div class="set-row" id="set-updates">
            <div class="txt"><b class="ver"><Brand /> <span class="mono">{app.version}</span></b>{#if us.state === 'checking' && updGate.show}<Skeleton variant="bar" lh={20} h={11} w={180} />{:else}<div class="hint" role="status" aria-live="polite">{usText}</div>{/if}</div>
            {#if us.state === 'ready'}
              <button class="btn accent" type="button" onclick={() => (updates.overlay = true)}>{t('update.install')}</button>
            {:else}
              <button class="btn" type="button" disabled={us.state === 'checking' || us.state === 'downloading' || us.state === 'disabled'} onclick={() => actions.updateCheck()}>
                <ArrowsClockwiseIcon weight="bold" size={16} aria-hidden="true" />{t('update.check')}
              </button>
            {/if}
          </div>
        </SettingSection>
      {/if}
    </div>

    {#if matches === null}<footer class="hint mono">{t('settings.version')} {app.version} &middot; {t('settings.dataDir')}: {app.rootDir}</footer>{/if}
  </div></div>
{/if}

{#if confirmKey}
  <ConfirmDialog danger title={t('storage.confirmTitle', { name: t(`storage.${confirmKey}` as Key) })} body={t(`storage.hint.${confirmKey}` as Key)} confirmLabel={t('storage.clear')} busy={clearing !== null}
    onconfirm={() => confirmKey && clearKey(confirmKey)} oncancel={() => (confirmKey = null)} />
{/if}

<style>
  .scroller { height: 100%; overflow: auto; }
  .settings { padding: var(--s-5) 32px 40px; max-width: 800px; display: grid; gap: 20px; align-content: start; }
  h1 { font-size: var(--fs-3xl); }
  .step { display: grid; gap: 4px; justify-items: end; }
  .ctl { display: flex; gap: 6px; align-items: center; }
  .ctl .field { width: 64px; text-align: center; }
  .wsz { display: grid; gap: 4px; justify-items: end; }
  .dims { display: flex; gap: 8px; align-items: center; }
  .dims .field { width: 84px; text-align: center; }
  .x { color: var(--paper-mute); }
  .bad { color: var(--tomato); font-size: var(--fs-sm); max-width: 280px; text-align: right; }
  footer { word-break: break-all; }
  .usage { list-style: none; margin: 0; padding: 0; display: grid; }
  .usage li { display: grid; grid-template-columns: minmax(0, 1fr) 36% auto; gap: 16px; align-items: center; padding: 8px 0; }
  .lbl { display: flex; justify-content: space-between; gap: 8px; }
  .dim { color: var(--paper-mute); }
  .bar { height: 8px; background: var(--ink-3); border-radius: 2px; overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--mango); transform-origin: left; }
  .total { margin: 0; }
  .totsk { margin: 0 0 4px; }
  b.ver { display: inline-flex; align-items: center; gap: 8px; }
</style>
