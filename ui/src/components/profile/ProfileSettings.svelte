<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte';
  import Skeleton from '../skeleton/Skeleton.svelte';
  import { BusyGate } from '../../lib/loadState.svelte';
  import SwapIcon from 'phosphor-svelte/lib/SwapIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import Modal from '../Modal.svelte';
  import ConfirmDialog from '../ConfirmDialog.svelte';
  import Seg from '../Seg.svelte';
  import InstanceIcon from '../InstanceIcon.svelte';
  import RamControl from '../RamControl.svelte';
  import GroupTagsEditor from '../GroupTagsEditor.svelte';
  import SettingRow from '../SettingRow.svelte';
  import SettingSection from '../SettingSection.svelte';
  import SettingsSearch from '../SettingsSearch.svelte';
  import { entriesFor } from '../../lib/settingsIndex';
  import { jumpTo } from '../../lib/settingsJump';
  import PanoramaBackground from '../PanoramaBackground.svelte';
  import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
  import PaletteIcon from 'phosphor-svelte/lib/PaletteIcon';
  import type { Key } from '../../lib/i18n';
  import { fmtDuration, fmtRelative } from '../../lib/format';
  import DownloadSimpleIcon from 'phosphor-svelte/lib/DownloadSimpleIcon';
  import { actions, app, errorText, getApi, isBusy, lang, openBackgroundPicker, openIconPicker, t, toast } from '../../lib/stores.svelte';
  import { toAppError } from '../../lib/api';
  import { WINDOW_PRESETS, checkInstanceSize, javaLabel, missingMajor, normalizeJavaPath, requiredJavaMajor, windowPatch } from '../../lib/javaHint';
  import type { JavaInfo, Loader, LoaderVersion, ProfileView, VersionManifest, WindowSettings } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
  let matches = $state<Set<string> | null>(null);
  const show = (id: string) => matches === null || matches.has(id);
  const instEntries = entriesFor('instance');
  const sec = (id: string) => matches === null || instEntries.some((e) => e.section === id && matches?.has(e.id));
  let name = $state(untrack(() => profile.name));
  let args = $state(untrack(() => profile.javaArgs));
  let saved = $state(false);
  let nameErr = $state(false);
  let confirmDelete = $state(false);
  let changing = $state(false);
  let deleting = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let savedTimer: ReturnType<typeof setTimeout> | undefined;

  const locked = $derived(isBusy(profile.id));
  const mc3 = $derived(profile.mangoConfig === null ? 'default' : profile.mangoConfig ? 'on' : 'off');

  async function save(patch: Partial<ProfileView>) {
    try {
      await actions.updateProfile(profile.id, patch);
      saved = true; clearTimeout(savedTimer);
      savedTimer = setTimeout(() => (saved = false), 1600);
    } catch (e) { toast(errorText(e), 'error'); }
  }
  function saveName() {
    const n = name.trim();
    if (!n) { nameErr = true; name = profile.name; return; }
    nameErr = false;
    if (n !== profile.name) save({ name: n });
  }
  function onArgs() { clearTimeout(timer); timer = setTimeout(() => save({ javaArgs: args }), 450); }
  async function remove() {
    deleting = true;
    try { await actions.deleteProfile(profile.id); toast(t('psettings.deleted', { name: profile.name }), 'ok'); }
    catch (e) { toast(errorText(e), 'error'); deleting = false; confirmDelete = false; }
  }

  /* ---- Java: a runtime from java_list, automatic (null) or a custom path validated by Rust ---- */
  const CUSTOM = '__custom';
  let javas = $state<JavaInfo[] | null>(null);
  let javaErr = $state('');
  let customText = $state(untrack(() => profile.javaPath ?? ''));
  let customMode = $state(false);
  let installing = $state<{ major: number; fraction: number } | null>(null);
  let unlistenJava: (() => void) | null = null;
  const needMajor = $derived(requiredJavaMajor(profile.mcVersion));
  const needInstall = $derived(javas !== null && missingMajor(javas, needMajor));
  /** A saved path that no listed runtime has is a custom path; the list may still be loading (then it is not judged yet). */
  const isCustom = $derived(customMode || (profile.javaPath !== null && javas !== null && !javas.some((j) => j.path === profile.javaPath)));
  const javaValue = $derived(isCustom ? CUSTOM : profile.javaPath ?? '');
  const loadJava = () => getApi().javaList().then((l) => (javas = l), () => { javas = []; });
  onMount(() => {
    void loadJava();
    void getApi().listen('java:progress', (e) => { if (installing && e.major === installing.major) installing = { major: e.major, fraction: e.total > 0 ? Math.min(1, e.done / e.total) : 0 }; }).then((u) => { unlistenJava = u; });
  });
  onDestroy(() => unlistenJava?.());
  async function saveJava(path: string | null) {
    javaErr = '';
    try {
      await actions.updateProfile(profile.id, { javaPath: path });
      saved = true; clearTimeout(savedTimer); savedTimer = setTimeout(() => (saved = false), 1600);
      void loadJava();
    } catch (e) {
      javaErr = toAppError(e).code.startsWith('java.') ? errorText(e) : errorText(e);
    }
  }
  function pickJava(v: string) {
    if (v === CUSTOM) { customMode = true; javaErr = ''; return; }
    customMode = false; customText = ''; javaErr = '';
    void saveJava(v === '' ? null : v);
  }
  function commitCustom() {
    const p = normalizeJavaPath(customText);
    if (p === profile.javaPath) return;
    if (p === null) { customMode = false; void saveJava(null); return; }
    void saveJava(p);
  }
  async function installJava() {
    if (needMajor === null || installing) return;
    installing = { major: needMajor, fraction: 0 };
    try {
      await getApi().javaInstall({ major: needMajor });
      await loadJava();
      toast(t('psettings.java.installed', { major: needMajor }), 'ok');
    } catch (e) { toast(errorText(e), 'error'); }
    installing = null;
  }

  /* ---- game window: default | custom size | fullscreen; saved debounced, validated like Rust ---- */
  const win0 = untrack(() => profile.window);
  let wMode = $state<WindowSettings['mode']>(win0.mode);
  let wW = $state(String(win0.width ?? app.config?.width ?? 1280));
  let wH = $state(String(win0.height ?? app.config?.height ?? 720));
  const wCheck = $derived(checkInstanceSize(wW, wH));
  let wTimer: ReturnType<typeof setTimeout> | undefined;
  function saveWindow(mode: WindowSettings['mode'], size?: { width: number; height: number }) {
    clearTimeout(wTimer);
    wTimer = setTimeout(() => save({ window: windowPatch(mode, size) }), mode === 'size' ? 450 : 0);
  }
  function setMode(m: string) {
    wMode = m as WindowSettings['mode'];
    if (wMode === 'size') { const c = checkInstanceSize(wW, wH); if (c.ok) saveWindow('size', c.value); }
    else saveWindow(wMode);
  }
  function onDims() { if (wMode === 'size' && wCheck.ok) saveWindow('size', wCheck.value); }
  function preset(w: number, h: number) { wW = String(w); wH = String(h); saveWindow('size', { width: w, height: h }); }
  onDestroy(() => { clearTimeout(wTimer); clearTimeout(timer); });

  /* ---- change version dialog ---- */
  let manifest = $state<VersionManifest | null>(null);
  let mcVersion = $state(untrack(() => profile.mcVersion));
  let loader = $state<Loader>(untrack(() => profile.loader));
  let loaderVersions = $state<LoaderVersion[]>([]);
  let loaderVersion = $state(untrack(() => profile.loaderVersion ?? ''));
  let applying = $state(false);
  const LOADERS: Loader[] = ['vanilla', 'fabric', 'quilt', 'forge', 'neoforge'];
  const versions = $derived((manifest?.versions ?? []).filter((v) => v.type === 'release' || (app.config?.showSnapshots && v.type === 'snapshot') || v.id === profile.mcVersion));
  const mGate = new BusyGate();
  $effect(() => { mGate.update(!manifest); return () => mGate.destroy(); });
  onMount(() => { getApi().versionsManifest().then((m) => (manifest = m), (e) => toast(errorText(e), 'error')); });
  let seq = 0;
  $effect(() => {
    const l = loader; const v = mcVersion;
    if (!changing) return;
    const my = ++seq;
    if (l === 'vanilla') { loaderVersions = []; loaderVersion = ''; return; }
    getApi().loaderVersions({ loader: l, mcVersion: v }).then((r) => {
      if (my !== seq) return;
      loaderVersions = r;
      if (!r.some((x) => x.version === loaderVersion)) loaderVersion = (r.find((x) => x.stable) ?? r[0])?.version ?? '';
    }, (e) => toast(errorText(e), 'error'));
  });
  async function apply() {
    applying = true;
    try {
      await actions.updateProfile(profile.id, { mcVersion, loader, loaderVersion: loader === 'vanilla' ? null : loaderVersion || null });
      changing = false;
      toast(t('psettings.versionChanged'), 'ok');
    } catch (e) { toast(errorText(e), 'error'); }
    applying = false;
  }
</script>

<div class="ps">
  <div class="top">
    <SettingsSearch scope="instance" bind:matches onpick={(e) => jumpTo(e)} />
    <div class="status" role="status" aria-live="polite">{#if saved}<span class="ok">{t('settings.saved')}</span>{/if}</div>
  </div>

  <div class="set-sections">
    {#if sec('general')}
      <SettingSection id="p-general" title={t('psettings.group.general')}>
        {#if show('name')}
          <SettingRow id="name" stack label={t('new.name')} forId="ps-name">
            <input id="ps-name" class="field" bind:value={name} maxlength="48" autocomplete="off" aria-invalid={nameErr} onblur={saveName} onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
            {#snippet extra()}{#if nameErr}<div class="err" role="alert">{t('new.nameRequired')}</div>{/if}{/snippet}
          </SettingRow>
        {/if}

        {#if show('grouptags')}
          <GroupTagsEditor {profile} onsaved={() => { saved = true; clearTimeout(savedTimer); savedTimer = setTimeout(() => (saved = false), 1600); }} />
        {/if}

        {#if show('icon')}
          <div class="set-row" id="set-icon">
            <div class="who"><InstanceIcon profile={profile} size={56} notch={10} />
              <div class="txt"><span class="lbl">{t('psettings.icon')}</span><span class="hint">{t('psettings.iconHint')}</span></div></div>
            <button class="btn" type="button" onclick={() => openIconPicker(profile.id)}><PaletteIcon weight="bold" size={16} aria-hidden="true" />{t('psettings.iconChange')}</button>
          </div>
        {/if}

        {#if show('background')}
          <div class="set-row" id="set-background">
            <div class="who"><span class="bgthumb on-stage"><PanoramaBackground profileId={profile.id} mcVersion={profile.mcVersion} background={profile.background} tint={profile.color} /></span>
              <div class="txt"><span class="lbl">{t('psettings.background')}</span><span class="hint">{t(`psettings.bg.${profile.background.kind}` as Key)}</span></div></div>
            <button class="btn" type="button" onclick={() => openBackgroundPicker(profile.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" />{t('psettings.backgroundChange')}</button>
          </div>
        {/if}
      </SettingSection>
    {/if}

    {#if sec('game')}
      <SettingSection id="p-game" title={t('settings.group.game')}>
        {#if show('version')}
          <SettingRow id="version" label={t('psettings.version')} hint={`${profile.mcVersion} · ${profile.loader === 'vanilla' ? t('profile.vanilla') : `${profile.loader} ${profile.loaderVersion ?? ''}`.trim()}`}>
            <button class="btn" type="button" disabled={locked} onclick={() => (changing = true)}><SwapIcon weight="bold" size={16} aria-hidden="true" />{t('psettings.change')}</button>
          </SettingRow>
        {/if}

        {#if show('ram')}
          <SettingRow id="ram" stack label={t('new.ram')} hint={t('ram.hint.instance')}>
            <RamControl id="ps-ram" value={profile.effectiveRamMb} source={profile.ramSource} hint={profile.ramHint} limits={app.ramLimits} defaultMb={app.defaultRamMb}
              onsave={(ram) => save({ ram })} />
          </SettingRow>
        {/if}

        {#if show('javaArgs')}
          <SettingRow id="javaArgs" stack label={t('settings.javaArgs')} hint={t('psettings.argsHint')} forId="ps-args">
            <input id="ps-args" class="field mono" bind:value={args} oninput={onArgs} spellcheck="false" autocomplete="off" placeholder={app.config?.javaArgs ?? ''} />
          </SettingRow>
        {/if}

        {#if show('java')}
          <SettingRow id="java" stack label={t('psettings.java')} hint={needMajor !== null ? t('psettings.java.needs', { mc: profile.mcVersion, major: needMajor }) : t('psettings.java.hint')} forId="ps-java">
            <select id="ps-java" class="field" value={javaValue} disabled={javas === null || locked} onchange={(e) => pickJava(e.currentTarget.value)}>
              <option value="">{t('psettings.java.auto')}</option>
              {#each javas ?? [] as j (j.path)}<option value={j.path}>{javaLabel(j)}</option>{/each}
              <option value={CUSTOM}>{t('psettings.java.custom')}</option>
            </select>
            {#snippet extra()}
              {#if isCustom}
                <div class="cust">
                  <input id="ps-java-path" class="field mono" bind:value={customText} spellcheck="false" autocomplete="off" aria-label={t('psettings.java.path')} aria-invalid={!!javaErr} aria-describedby={javaErr ? 'ps-java-err' : undefined}
                    placeholder={t('psettings.java.pathPlaceholder')} onblur={commitCustom} onkeydown={(e) => e.key === 'Enter' && commitCustom()} />
                </div>
              {/if}
              {#if javaErr}<div id="ps-java-err" class="err" role="alert">{javaErr}</div>{/if}
              {#if javaValue === '' && needInstall && needMajor !== null}
                <div class="jinst">
                  <span class="hint">{t('psettings.java.missing', { major: needMajor })}</span>
                  {#if installing}
                    <span class="jbar" role="progressbar" aria-label={t('psettings.java.installing', { major: installing.major })} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(installing.fraction * 100)}><span style:transform="scaleX({installing.fraction})"></span></span>
                    <span class="hint mono">{Math.round(installing.fraction * 100)}%</span>
                  {:else}
                    <button class="btn small" type="button" disabled={locked} onclick={installJava}><DownloadSimpleIcon weight="bold" size={14} aria-hidden="true" />{t('psettings.java.install', { major: needMajor })}</button>
                  {/if}
                </div>
              {/if}
            {/snippet}
          </SettingRow>
        {/if}

        {#if show('window')}
          <SettingRow id="window" stack label={t('psettings.window')} hint={t('psettings.window.hint')}>
            <Seg label={t('psettings.window')} value={wMode} idPrefix="pswin"
              options={[{ value: 'default', label: t('psettings.window.default') }, { value: 'size', label: t('psettings.window.size') }, { value: 'fullscreen', label: t('psettings.window.fullscreen') }]}
              onchange={setMode} />
            {#snippet extra()}
              {#if wMode === 'default'}
                <span class="hint">{t('psettings.window.defaultNow', { w: app.config?.fullscreen ? t('psettings.window.fullscreen') : `${app.config?.width ?? 1280} × ${app.config?.height ?? 720}` })}</span>
              {:else if wMode === 'size'}
                <div class="dims">
                  <input class="field mono" inputmode="numeric" autocomplete="off" aria-label={t('settings.width')} aria-invalid={!wCheck.width} bind:value={wW} oninput={onDims} />
                  <span class="x" aria-hidden="true">&times;</span>
                  <input class="field mono" inputmode="numeric" autocomplete="off" aria-label={t('settings.height')} aria-invalid={!wCheck.height} bind:value={wH} oninput={onDims} />
                </div>
                <div class="presets" role="group" aria-label={t('psettings.window.presets')}>
                  {#each WINDOW_PRESETS as [pw, ph] (pw)}
                    <button class="btn small mono" class:cur={wW === String(pw) && wH === String(ph)} type="button" onclick={() => preset(pw, ph)}>{pw}&times;{ph}</button>
                  {/each}
                </div>
                {#if !wCheck.ok}<div class="err" role="alert">{t('psettings.window.invalid')}</div>{/if}
              {/if}
              <span class="hint">{t('psettings.window.noMax')}</span>
            {/snippet}
          </SettingRow>
        {/if}

        {#if show('mango')}
          <SettingRow id="mango" label={t('psettings.mango')} hint={t('psettings.mangoHint', { state: app.config?.mangoConfig ? t('psettings.on') : t('psettings.off') })}>
            <Seg label={t('psettings.mango')} value={mc3} idPrefix="psmc"
              options={[{ value: 'default', label: t('psettings.default') }, { value: 'on', label: t('psettings.on') }, { value: 'off', label: t('psettings.off') }]}
              onchange={(v) => save({ mangoConfig: v === 'default' ? null : v === 'on' })} />
          </SettingRow>
        {/if}
      </SettingSection>
    {/if}

    {#if sec('danger')}
      <SettingSection id="p-danger" danger title={t('psettings.danger')}>
        {#if show('delete')}
          <div class="set-row" id="set-delete">
            <div class="txt"><span class="lbl">{t('psettings.deleteTitle')}</span><span class="hint">{t('psettings.deleteHint')}</span></div>
            <div class="delcol">
              <button class="btn danger" type="button" disabled={locked} onclick={() => (confirmDelete = true)}><TrashIcon weight="bold" size={16} aria-hidden="true" />{t('profile.delete')}</button>
              {#if locked}<span class="hint">{t('psettings.lose.running')}</span>{/if}
            </div>
          </div>
        {/if}
      </SettingSection>
    {/if}
  </div>
</div>

{#if confirmDelete}
  <ConfirmDialog danger busy={deleting} title={t('psettings.confirmTitle', { name: profile.name })} body={t('psettings.confirmBody')}
    facts={[
      { label: t('psettings.lose.playtime'), value: fmtDuration(profile.playTimeMs) },
      { label: t('psettings.lose.mods'), value: String(profile.modCount) },
      { label: t('psettings.lose.last'), value: fmtRelative(profile.lastPlayed, lang()) ?? t('home.never') },
    ]} warn={t('psettings.lose.warn')} confirmLabel={t('profile.delete')} onconfirm={remove} oncancel={() => (confirmDelete = false)} />
{/if}

{#if changing}
  <Modal title={t('psettings.change')} onclose={() => (changing = false)}>
    <div class="chg">
      <p class="hint">{t('psettings.changeHint')}</p>
      <label for="cv-loader">{t('new.loader')}</label>
      <select id="cv-loader" class="field" bind:value={loader}>{#each LOADERS as l (l)}<option value={l}>{l === 'vanilla' ? t('profile.vanilla') : l}</option>{/each}</select>
      <label for="cv-mc">{t('col.version')}</label>
      {#if !manifest && mGate.show}<Skeleton variant="box" w="100%" h={36} radius={8} />{:else}<select id="cv-mc" class="field mono" bind:value={mcVersion} disabled={!manifest}>{#each versions as v (v.id)}<option value={v.id}>{v.id}</option>{/each}</select>{/if}
      {#if loader !== 'vanilla'}
        <label for="cv-lv">{t('new.loaderVersion')}</label>
        <select id="cv-lv" class="field mono" bind:value={loaderVersion}>{#each loaderVersions as lv (lv.version)}<option value={lv.version}>{lv.version}{lv.stable ? ` (${t('new.recommended')})` : ''}</option>{/each}</select>
      {/if}
    </div>
    {#snippet footer()}
      <button class="btn ghost" type="button" onclick={() => (changing = false)}>{t('common.cancel')}</button>
      <button class="btn accent" type="button" disabled={applying || (loader !== 'vanilla' && !loaderVersion)} onclick={apply}>{t('psettings.apply')}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .who { display: flex; align-items: center; gap: 14px; min-width: 0; }
  .bgthumb { position: relative; flex: none; width: 100px; height: 56px; border-radius: var(--r-s); overflow: hidden; border: 1px solid var(--line); }
  .delcol { display: grid; gap: 4px; justify-items: end; text-align: right; max-width: 260px; }
  .ps { max-width: 760px; display: grid; gap: 14px; }
  .top { display: grid; gap: 4px; }
  .status { height: 18px; font-size: 13px; }
  .ok { color: var(--leaf); }
  .cust { display: grid; }
  .jinst { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .jbar { width: 160px; height: 6px; background: var(--ink-3); border-radius: 2px; overflow: hidden; }
  .jbar span { display: block; height: 100%; background: var(--mango); transform-origin: left; transition: transform var(--dur) var(--ease); }
  .dims { display: flex; gap: 8px; align-items: center; }
  .dims .field { width: 96px; text-align: center; }
  .x { color: var(--paper-mute); }
  .presets { display: flex; gap: 6px; flex-wrap: wrap; }
  .presets .cur { border-color: var(--accent-fill); }
  .chg { display: grid; gap: 6px; }
  .chg label { margin-top: 8px; font-weight: 500; }
</style>
