<script lang="ts">
  import { onMount } from 'svelte';
  import LoadError from './LoadError.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import ListRowSkeleton from './skeleton/ListRowSkeleton.svelte';
  import { BusyGate, createLoadState } from '../lib/loadState.svelte';
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import PaletteIcon from 'phosphor-svelte/lib/PaletteIcon';
  import BlockIcon from './BlockIcon.svelte';
  import Tile from './Tile.svelte';
  import IconPicker from './IconPicker.svelte';
  import { blockForSeed, choiceFromIcon, findBlock, iconTint, type IconChoice } from '../lib/instanceIcon';
  import { fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { actions, app, blocks, errorText, getApi, nav, startInstall, t } from '../lib/stores.svelte';
  import type { Loader, LoaderVersion, VersionManifest } from '../lib/types';

  const LOADERS: Loader[] = ['vanilla', 'fabric', 'quilt', 'forge', 'neoforge'];
  const COLORS = ['#ffad32', '#86b86b', '#ed665c', '#6fa8d6', '#d6c27a', '#e6a3b8'];
  const PAGE = 40;

  let dlg: HTMLDialogElement | undefined = $state();
  let step = $state(0);
  const prefill = nav.newProfilePrefill;
  let name = $state(prefill?.name ?? '');
  let color = $state(COLORS[0]);
  let touched = $state(false);
  let manifest = $state<VersionManifest | null>(null);
  const manifestLoad = createLoadState<VersionManifest>({ isEmpty: () => false });
  const lvGate = new BusyGate();
  $effect(() => { lvGate.update(loaderLoading); return () => lvGate.destroy(); });
  let filter = $state<'release' | 'snapshot' | 'all'>('release');
  let q = $state('');
  let limit = $state(PAGE);
  let mcVersion = $state('');
  let loader = $state<Loader>(prefill?.loader ?? 'vanilla');
  let loaderVersions = $state<LoaderVersion[]>([]);
  let loaderVersion = $state('');
  let loaderLoading = $state(false);
  let creating = $state(false);
  // icon: auto previews a block from a per-dialog seed; on create it is saved as that explicit block so what you saw is what you get
  const seed = crypto.randomUUID?.() ?? String(Math.random());
  let iconChoice = $state<IconChoice>(choiceFromIcon(null));
  let pickIcon = $state(false);
  const iconTone = $derived(iconTint({ color, loader }));
  const iconBlock = $derived.by(() => {
    if (blocks.status?.state !== 'ready') return null;
    return iconChoice.kind === 'block' ? findBlock(blocks.status, iconChoice.block) : iconChoice.kind === 'auto' ? blockForSeed(seed, blocks.status.blocks) : null;
  });
  let error = $state('');

  const steps = ['new.step.name', 'new.step.version', 'new.step.resources'] as const;
  const filtered = $derived.by(() => {
    const s = q.trim().toLowerCase();
    return (manifest?.versions ?? []).filter((v) =>
      (filter === 'all' || (filter === 'release' ? v.type === 'release' : v.type === 'snapshot')) && (!s || v.id.toLowerCase().includes(s)));
  });
  const nameOk = $derived(name.trim().length > 0);

  onMount(() => {
    dlg?.showModal();
    manifestLoad.run(() => getApi().versionsManifest());
    return () => manifestLoad.cancel();
  });

  $effect(() => {
    const m = manifestLoad.data;
    if (m && !manifest) {
      manifest = m;
      mcVersion = prefill?.mcVersion && m.versions.some((v) => v.id === prefill.mcVersion) ? prefill.mcVersion : m.latest.release;
    }
  });

  function close() { nav.newProfile = false; nav.newProfilePrefill = null; }

  let loadSeq = 0;
  async function loadLoader() {
    const seq = ++loadSeq;
    loaderVersions = []; loaderVersion = '';
    if (loader === 'vanilla' || !mcVersion) return;
    loaderLoading = true;
    try {
      const l = await getApi().loaderVersions({ loader, mcVersion });
      if (seq !== loadSeq) return;
      loaderVersions = l;
      loaderVersion = (l.find((x) => x.stable) ?? l[0])?.version ?? '';
    } catch (e) { if (seq === loadSeq) error = errorText(e); }
    if (seq === loadSeq) loaderLoading = false;
  }
  $effect(() => { void loader; void mcVersion; loadLoader(); });
  $effect(() => { void q; void filter; limit = PAGE; });

  function next() {
    if (step === 0) { touched = true; if (!nameOk) return; }
    if (step === 1 && (!mcVersion || (loader !== 'vanilla' && !loaderLoading && !loaderVersion))) return;
    step = Math.min(2, step + 1);
  }
  async function create() {
    creating = true; error = '';
    try {
      const p = await actions.createProfile(
        { name: name.trim(), mcVersion, loader, loaderVersion: loader === 'vanilla' ? null : loaderVersion },
        { color },
      );
      try {
        const b = iconChoice.kind === 'block' ? iconChoice.block : iconChoice.kind === 'auto' ? iconBlock?.id : null;
        if (iconChoice.kind === 'image') await actions.setIconImage(p.id, iconChoice.path);
        else if (b) await actions.setIconBlock(p.id, b);
      } catch { /* the instance exists; the icon can be changed on its page */ }
      close();
      // "Create new instance for this": continue with the dependency dialog for the new instance
      if (prefill?.then) startInstall({ kind: 'content', project: prefill.then, scopeProfileId: p.id });
    } catch (e) { error = errorText(e); creating = false; }
  }
</script>

<dialog bind:this={dlg} class="dlg" aria-labelledby="np-title" onclose={close} oncancel={close}>
  <header>
    <h2 id="np-title">{t('new.title')}</h2>
    <button class="btn ghost" type="button" aria-label={t('common.close')} onclick={close}><XIcon weight="bold" size={16} aria-hidden="true" /></button>
  </header>

  <ol class="steps" aria-label={t('new.title')}>
    {#each steps as s, i (s)}
      <li class:cur={i === step} class:done={i < step} aria-current={i === step ? 'step' : undefined}><span class="n mono">{i + 1}</span>{t(s as Key)}</li>
    {/each}
  </ol>

  <div class="body">
    {#if step === 0}
      <label class="lbl" for="np-name">{t('new.name')}</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="np-name" class="field" bind:value={name} placeholder={t('new.namePlaceholder')} maxlength="48" autofocus autocomplete="off"
        aria-invalid={touched && !nameOk} aria-describedby={touched && !nameOk ? 'np-name-err' : undefined}
        onkeydown={(e) => e.key === 'Enter' && next()} />
      {#if touched && !nameOk}<div id="np-name-err" class="err" role="alert">{t('new.nameRequired')}</div>{/if}
      <div class="lbl">{t('new.icon')}</div>
      <div class="irow">
        <span class="ibox">
          {#if iconBlock}<BlockIcon faces={iconBlock.faces} size={64} notch={11} tint={iconTone} />
          {:else if iconChoice.kind === 'image' && iconChoice.preview}<Tile kind="profile" id={seed} src={iconChoice.preview} size={64} notch={11} />
          {:else}<Tile kind="profile" id={seed} src={null} tint={iconTone} size={64} notch={11} />{/if}
        </span>
        <div class="itxt"><b>{iconChoice.kind === 'auto' ? t('icons.auto') + (iconBlock ? `: ${iconBlock.label}` : '') : iconChoice.kind === 'block' ? (iconBlock?.label ?? '') : iconChoice.kind === 'image' ? iconChoice.name : ''}</b>
          <span class="hint">{t('new.iconHint')}</span></div>
        <button class="btn" type="button" onclick={() => (pickIcon = true)}><PaletteIcon weight="bold" size={16} aria-hidden="true" />{t('icons.changeShort')}</button>
      </div>
      <div class="lbl">{t('new.color')}</div>
      <div class="swatches" role="radiogroup" aria-label={t('new.color')}>
        {#each COLORS as c (c)}
          <button type="button" role="radio" aria-checked={color === c} aria-label={c} class="sw" class:on={color === c} style:background={c} onclick={() => (color = c)}>
            {#if color === c}<CheckIcon weight="bold" size={14} color="#1a1100" aria-hidden="true" />{/if}
          </button>
        {/each}
      </div>
    {:else if step === 1}
      <div class="lbl">{t('new.loader')}</div>
      <div class="loaders" role="radiogroup" aria-label={t('new.loader')}>
        {#each LOADERS as l (l)}
          <button type="button" role="radio" aria-checked={loader === l} class="lo" class:on={loader === l} onclick={() => (loader = l)}>{l === 'vanilla' ? t('profile.vanilla') : l}</button>
        {/each}
      </div>
      <div class="vrow">
        <label class="search">
          <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
          <span class="sr-only">{t('new.search')}</span>
          <input class="field" type="search" bind:value={q} placeholder={t('new.search')} />
        </label>
        <div class="loaders" role="radiogroup" aria-label={t('new.releases')}>
          {#each [['release', 'new.releases'], ['snapshot', 'new.snapshots'], ['all', 'new.all']] as [f, k] (f)}
            <button type="button" role="radio" aria-checked={filter === f} class="lo" class:on={filter === f} onclick={() => (filter = f as typeof filter)}>{t(k as Key)}</button>
          {/each}
        </div>
      </div>
      {#if manifestLoad.phase === 'error' && !manifest}
        <LoadError compact error={manifestLoad.error} onretry={() => manifestLoad.retry()} />
      {:else if !manifest}
        {#if manifestLoad.showSkeleton}<div class="vlist"><ListRowSkeleton count={6} img={0} rowHeight={35} padding="0 12px" gap={0} bars={[{ w: '22%', h: 13 }]} /></div>{/if}
      {:else if filtered.length === 0}
        <p class="hint">{t('new.noVersions')}</p>
      {:else}
        <ul class="vlist" role="listbox" aria-label={t('new.step.version')}>
          {#each filtered.slice(0, limit) as v (v.id)}
            <li role="option" aria-selected={v.id === mcVersion}>
              <button type="button" class:on={v.id === mcVersion} onclick={() => (mcVersion = v.id)}>
                <span class="mono">{v.id}</span>
                <span class="tags">
                  {#if v.id === manifest.latest.release}<em>{t('new.latest')}</em>{/if}
                  {#if manifest.installed.includes(v.id)}<em class="inst">{t('new.installed')}</em>{/if}
                </span>
              </button>
            </li>
          {/each}
          {#if filtered.length > limit}
            <li><button type="button" class="more" onclick={() => (limit += PAGE)}>{t('new.showMore')}</button></li>
          {/if}
        </ul>
      {/if}
      {#if loader !== 'vanilla'}
        <label class="lbl" for="np-lv">{t('new.loaderVersion')}</label>
        {#if loaderLoading}
          {#if lvGate.show}<Skeleton variant="box" w="100%" h={36} radius={8} />{/if}
        {:else if !loaderVersions.length}
          <p class="err">{t('new.noLoaderVersion')}</p>
        {:else}
          <select id="np-lv" class="field" bind:value={loaderVersion}>
            {#each loaderVersions as lv (lv.version)}<option value={lv.version}>{lv.version}{lv.stable ? ` (${t('new.recommended')})` : ''}</option>{/each}
          </select>
        {/if}
      {/if}
    {:else}
      <p class="hint">{t('new.ramDefault', { ram: fmtRam(app.config?.ram ?? app.defaultRamMb) })}</p>
      <div class="lbl">{t('new.summary')}</div>
      <dl class="sum">
        <dt>{t('new.name')}</dt><dd>{name.trim()}</dd>
        <dt>{t('col.version')}</dt><dd class="mono">{mcVersion}</dd>
        <dt>{t('col.loader')}</dt><dd>{loader === 'vanilla' ? t('profile.vanilla') : `${loader} ${loaderVersion}`}</dd>
        <dt>{t('new.ram')}</dt><dd class="mono">{fmtRam(app.config?.ram ?? app.defaultRamMb)}</dd>
      </dl>
    {/if}
    {#if error}<div class="err" role="alert">{error}</div>{/if}
  </div>

  <footer>
    <button class="btn ghost" type="button" onclick={close}>{t('new.cancel')}</button>
    <span class="sp"></span>
    {#if step > 0}<button class="btn" type="button" onclick={() => (step -= 1)}><ArrowLeftIcon weight="bold" size={14} aria-hidden="true" />{t('new.back')}</button>{/if}
    {#if step < 2}
      <button class="btn accent" type="button" onclick={next}>{t('new.next')}<ArrowRightIcon weight="bold" size={14} aria-hidden="true" /></button>
    {:else}
      <button class="btn accent" type="button" disabled={creating} onclick={create}>{creating ? t('new.creating') : t('new.create')}</button>
    {/if}
  </footer>
</dialog>

{#if pickIcon}
  <IconPicker profileId={null} {seed} tint={iconTone} initial={iconChoice} onchoose={(c) => (iconChoice = c)} onclose={() => (pickIcon = false)} />
{/if}

<style>
  .dlg { width: min(640px, calc(100vw - 32px)); height: min(640px, calc(100vh - 48px)); padding: 0; background: var(--ink-2); color: var(--paper); border: 1px solid var(--line); border-radius: var(--r-l); box-shadow: 0 24px 64px rgba(0, 0, 0, 0.7); }
  .dlg[open] { display: flex; flex-direction: column; }
  .dlg::backdrop { background: rgba(5, 6, 7, 0.7); }
  header, .steps, footer { flex: none; }
  header { display: flex; justify-content: space-between; align-items: center; padding: 16px 20px 0; }
  h2 { font-size: 22px; }
  .steps { display: flex; gap: 20px; list-style: none; margin: 0; padding: 14px 20px; border-bottom: 1px solid var(--line); color: var(--paper-mute); font-size: 13px; }
  .steps li { display: flex; align-items: center; gap: 8px; }
  .n { width: 22px; height: 22px; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 4px; font-size: 12px; }
  .cur { color: var(--paper); font-weight: 600; }
  .cur .n { background: var(--mango); border-color: var(--mango); color: var(--mango-ink); }
  .done .n { border-color: var(--leaf); color: var(--leaf); }
  .body { display: grid; gap: 10px; padding: 18px 20px; overflow: auto; align-content: start; flex: 1 1 auto; min-height: 0; }
  .irow { display: flex; align-items: center; gap: 14px; }
  .ibox { flex: none; display: block; }
  .itxt { flex: 1; display: grid; gap: 2px; min-width: 0; }
  .lbl { font-size: 13px; font-weight: 600; color: var(--paper-dim); margin-top: 4px; }
  .swatches, .loaders { display: flex; gap: 6px; flex-wrap: wrap; }
  .sw { width: 30px; height: 30px; display: grid; place-items: center; border-radius: var(--r-s); border: 2px solid transparent; }
  .sw.on { border-color: var(--paper); }
  .lo { height: 30px; padding: 0 12px; border: 1px solid var(--line); border-radius: var(--r-s); color: var(--paper-dim); font-size: 13px; text-transform: capitalize; }
  .lo.on { background: var(--paper); color: var(--ink-0); border-color: var(--paper); font-weight: 600; }
  .vrow { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; }
  .search { position: relative; flex: 1; min-width: 180px; }
  .search :global(svg) { position: absolute; left: 12px; top: 10px; color: var(--paper-mute); pointer-events: none; }
  .search .field { width: 100%; padding-left: 36px; }
  .vlist { list-style: none; margin: 0; padding: 0; max-height: 200px; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-m); }
  .vlist button { display: flex; justify-content: space-between; width: 100%; padding: 7px 12px; text-align: left; border-bottom: 1px solid var(--line-soft); }
  .vlist button:hover { background: var(--ink-3); }
  .vlist button.on { background: rgba(255, 173, 50, 0.14); box-shadow: inset 3px 0 var(--mango); }
  .more { color: var(--mango); justify-content: center !important; }
  .tags { display: flex; gap: 6px; }
  em { font-style: normal; font-size: 12px; padding: 0 6px; border-radius: 3px; background: var(--ink-4); color: var(--paper-dim); }
  em.inst { color: var(--leaf); }
  .sum { display: grid; grid-template-columns: max-content 1fr; gap: 6px 18px; margin: 0; padding: 12px 14px; background: var(--ink-1); border: 1px solid var(--line); border-radius: var(--r-m); }
  .sum dt { color: var(--paper-mute); } .sum dd { margin: 0; }
  footer { display: flex; gap: 8px; padding: 14px 20px; border-top: 1px solid var(--line); }
  .sp { flex: 1; }
</style>
