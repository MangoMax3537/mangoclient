<script lang="ts">
  import { onMount, untrack } from 'svelte';
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
  import PanoramaBackground from '../PanoramaBackground.svelte';
  import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
  import PaletteIcon from 'phosphor-svelte/lib/PaletteIcon';
  import type { Key } from '../../lib/i18n';
  import { fmtDuration, fmtRelative } from '../../lib/format';
  import { actions, app, errorText, getApi, isBusy, lang, openBackgroundPicker, openIconPicker, t, toast } from '../../lib/stores.svelte';
  import type { Loader, LoaderVersion, ProfileView, VersionManifest } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
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
  <div class="status" role="status" aria-live="polite">{#if saved}<span class="ok">{t('settings.saved')}</span>{/if}</div>

  <section>
    <div class="row col">
      <label for="ps-name">{t('new.name')}</label>
      <input id="ps-name" class="field" bind:value={name} maxlength="48" autocomplete="off" aria-invalid={nameErr} onblur={saveName} onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
      {#if nameErr}<div class="err" role="alert">{t('new.nameRequired')}</div>{/if}
    </div>

    <GroupTagsEditor {profile} onsaved={() => { saved = true; clearTimeout(savedTimer); savedTimer = setTimeout(() => (saved = false), 1600); }} />

    <div class="row">
      <div class="who"><InstanceIcon profile={profile} size={56} notch={10} />
        <div class="txt"><b>{t('psettings.icon')}</b><div class="hint">{t('psettings.iconHint')}</div></div></div>
      <button class="btn" type="button" onclick={() => openIconPicker(profile.id)}><PaletteIcon weight="bold" size={16} aria-hidden="true" />{t('psettings.iconChange')}</button>
    </div>

    <div class="row">
      <div class="who"><span class="bgthumb on-stage"><PanoramaBackground profileId={profile.id} mcVersion={profile.mcVersion} background={profile.background} tint={profile.color} /></span>
        <div class="txt"><b>{t('psettings.background')}</b><div class="hint">{t(`psettings.bg.${profile.background.kind}` as Key)}</div></div></div>
      <button class="btn" type="button" onclick={() => openBackgroundPicker(profile.id)}><ImageIcon weight="bold" size={16} aria-hidden="true" />{t('psettings.backgroundChange')}</button>
    </div>

    <div class="row">
      <div class="txt"><b>{t('psettings.version')}</b>
        <div class="hint mono">{profile.mcVersion} &middot; {profile.loader === 'vanilla' ? t('profile.vanilla') : `${profile.loader} ${profile.loaderVersion ?? ''}`}</div>
      </div>
      <button class="btn" type="button" disabled={locked} onclick={() => (changing = true)}><SwapIcon weight="bold" size={16} aria-hidden="true" />{t('psettings.change')}</button>
    </div>

    <div class="row col">
      <div class="txt"><b id="ps-ram-l">{t('new.ram')}</b><div class="hint">{t('ram.hint.instance')}</div></div>
      <RamControl id="ps-ram" value={profile.effectiveRamMb} source={profile.ramSource} hint={profile.ramHint} limits={app.ramLimits} defaultMb={app.defaultRamMb}
        onsave={(ram) => save({ ram })} />
    </div>

    <div class="row col">
      <label for="ps-args">{t('settings.javaArgs')}</label>
      <input id="ps-args" class="field mono" bind:value={args} oninput={onArgs} spellcheck="false" autocomplete="off" placeholder={app.config?.javaArgs ?? ''} />
      <div class="hint">{t('psettings.argsHint')}</div>
    </div>

    <div class="row">
      <div class="txt"><b id="ps-mc">{t('psettings.mango')}</b><div class="hint">{t('psettings.mangoHint', { state: app.config?.mangoConfig ? t('psettings.on') : t('psettings.off') })}</div></div>
      <Seg label={t('psettings.mango')} value={mc3} idPrefix="psmc"
        options={[{ value: 'default', label: t('psettings.default') }, { value: 'on', label: t('psettings.on') }, { value: 'off', label: t('psettings.off') }]}
        onchange={(v) => save({ mangoConfig: v === 'default' ? null : v === 'on' })} />
    </div>
  </section>

  <section class="danger" aria-labelledby="ps-danger">
    <h2 id="ps-danger" class="eyebrow">{t('psettings.danger')}</h2>
    <div class="row">
      <div class="txt"><b>{t('psettings.deleteTitle')}</b><div class="hint">{t('psettings.deleteHint')}</div></div>
      <div class="delcol">
        <button class="btn danger" type="button" disabled={locked} onclick={() => (confirmDelete = true)}><TrashIcon weight="bold" size={16} aria-hidden="true" />{t('profile.delete')}</button>
        {#if locked}<span class="hint">{t('psettings.lose.running')}</span>{/if}
      </div>
    </div>
  </section>
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
  .ps { max-width: 760px; display: grid; gap: 24px; }
  .status { height: 18px; font-size: 13px; }
  .ok { color: var(--leaf); }
  section { display: grid; border-top: 1px solid var(--line); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 12px 0; border-bottom: 1px solid var(--line-soft); }
  .row.col { flex-direction: column; align-items: stretch; gap: 8px; }
  label { font-weight: 500; }
  .danger { border-top-color: rgba(237, 102, 92, 0.4); }
  .danger h2 { padding: 14px 0 6px; color: var(--tomato); }
  .chg { display: grid; gap: 6px; }
  .chg label { margin-top: 8px; }
</style>
