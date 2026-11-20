<script lang="ts">
  // Icon picker for an instance (profile page, Library card menu) or for a new one (profileId = null, result via onchoose).
  import { untrack } from 'svelte';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ArrowCounterClockwiseIcon from 'phosphor-svelte/lib/ArrowCounterClockwiseIcon';
  import FileImageIcon from 'phosphor-svelte/lib/FileImageIcon';
  import Modal from './Modal.svelte';
  import Seg from './Seg.svelte';
  import BlockIcon from './BlockIcon.svelte';
  import Tile from './Tile.svelte';
  import ImagePick from './ImagePick.svelte';
  import { brandIcon } from '../lib/fallbackImage';
  import {
    blockForSeed, choiceFromIcon, findBlock, iconTint, pickerState, sameChoice, selectedBlockId, type IconChoice,
  } from '../lib/instanceIcon';
  import { imageDrop, type PickedImage } from '../lib/pickImage';
  import type { Key } from '../lib/i18n';
  import { actions, app, blocks, errorText, prepareBlocks, t, toast } from '../lib/stores.svelte';
  import type { BlockIcon as Block } from '../lib/types';

  interface Props {
    profileId: string | null;
    /** New-instance mode: seeds the auto preview (the backend picks from the real id after creation). */
    seed?: string;
    tint?: string | null;
    initial?: IconChoice;
    onchoose?: (c: IconChoice) => void;
    onclose: () => void;
  }
  let { profileId, seed = '', tint = null, initial, onchoose, onclose }: Props = $props();

  const profile = $derived(profileId ? app.profiles.find((p) => p.id === profileId) ?? null : null);
  const start: IconChoice = untrack(() => initial ?? (profile ? choiceFromIcon(profile.icon) : { kind: 'auto' }));
  let draft = $state<IconChoice>(start);
  let tab = $state<'gallery' | 'upload' | 'logo'>(start.kind === 'keep' ? 'logo' : 'gallery');
  let busy = $state(false);
  let over = $state(false);
  let bad = $state(false);

  const st = $derived(pickerState(blocks.status));
  const hasLogo = $derived(!!profile?.coverUrl && profile.icon?.kind === 'image');
  const tone = $derived(iconTint(profile) ?? tint ?? 'var(--leaf)');
  const autoBlock = $derived<Block | null>(profile ? findBlock(blocks.status, profile.autoBlock) : st.state === 'ready' ? blockForSeed(seed, st.blocks) : null);
  const changed = $derived(!sameChoice(draft, start));
  const selected = $derived(draft.kind === 'block' ? draft.block : draft.kind === 'auto' ? 'auto' : '');

  // what the preview shows for the current draft
  const view = $derived.by(() => {
    if (draft.kind === 'block' && st.state === 'ready') { const b = findBlock(blocks.status, draft.block); if (b) return { type: 'block' as const, b }; }
    if (draft.kind === 'auto' && autoBlock) return { type: 'block' as const, b: autoBlock };
    if (draft.kind === 'keep' && profile?.coverUrl) return { type: 'image' as const, src: profile.coverUrl };
    if (draft.kind === 'image') return draft.preview ? { type: 'image' as const, src: draft.preview } : { type: 'file' as const, name: draft.name };
    return { type: 'brand' as const };
  });
  const sumLabel = $derived(
    draft.kind === 'block' ? findBlock(blocks.status, draft.block)?.label ?? ''
      : draft.kind === 'auto' ? t('icons.auto') + (autoBlock ? `: ${autoBlock.label}` : '')
      : draft.kind === 'image' ? draft.name : t('icons.logo.current'));

  function pickBlock(id: string) { draft = id === 'auto' ? { kind: 'auto' } : { kind: 'block', block: id }; }
  function onFile(i: PickedImage) { bad = false; draft = { kind: 'image', path: i.path, name: i.name, preview: i.preview }; tab = 'upload'; }

  /* roving radio grid */
  let grid: HTMLDivElement | undefined = $state();
  const cellIds = $derived(['auto', ...(st.state === 'ready' ? st.blocks.map((b) => b.id) : [])]);
  function key(e: KeyboardEvent, i: number) {
    const cols = grid ? getComputedStyle(grid).gridTemplateColumns.split(' ').length : 1;
    const d = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowDown' ? cols : e.key === 'ArrowUp' ? -cols : 0;
    const to = e.key === 'Home' ? 0 : e.key === 'End' ? cellIds.length - 1 : i + d;
    if (to === i || (!d && e.key !== 'Home' && e.key !== 'End') || to < 0 || to >= cellIds.length) return;
    e.preventDefault();
    pickBlock(cellIds[to]);
    queueMicrotask(() => (grid?.querySelectorAll<HTMLElement>('[role=radio]')[to])?.focus());
  }

  async function apply() {
    if (!profileId) { onchoose?.(draft); onclose(); return; }
    busy = true;
    try {
      if (draft.kind === 'auto') await actions.clearIcon(profileId);
      else if (draft.kind === 'block') await actions.setIconBlock(profileId, draft.block);
      else if (draft.kind === 'image') await actions.setIconImage(profileId, draft.path);
      onclose();
    } catch (e) { toast(errorText(e), 'error'); busy = false; }
  }
  async function useLogo() {
    if (!profileId) return;
    busy = true;
    try { await actions.useModpackLogo(profileId); toast(t('icons.logo.restored'), 'ok'); onclose(); }
    catch (e) { toast(errorText(e), 'error'); busy = false; }
  }
  const tabs = $derived([
    { value: 'gallery', label: t('icons.tab.gallery') },
    { value: 'upload', label: t('icons.tab.upload') },
    { value: 'logo', label: t('icons.tab.logo'), disabledReason: hasLogo ? undefined : t('icons.logo.none') },
  ]);
</script>

<Modal title={profile ? t('icons.title', { name: profile.name }) : t('icons.titleNew')} size="lg" {onclose} onenter={changed ? apply : undefined}>
  <div class="pk" use:imageDrop={{ over: (b) => (over = b), drop: onFile, reject: () => { bad = true; tab = 'upload'; } }}>
    <div class="prev">
      <div class="big">
        {#if view.type === 'block'}<BlockIcon faces={view.b.faces} fill notch={20} tint={tone} />
        {:else if view.type === 'image'}<Tile kind="profile" id={profileId ?? seed} src={view.src} {tint} fill notch={20} />
        {:else if view.type === 'file'}<span class="file notch" style:--n="20px"><FileImageIcon weight="bold" size={44} aria-hidden="true" /></span>
        {:else}<Tile kind="profile" id={profileId ?? seed} src={null} tint={tone} fill notch={20} />{/if}
      </div>
      <p class="cap" aria-live="polite"><b>{sumLabel}</b></p>
      <button class="btn small" type="button" disabled={draft.kind === 'auto'} onclick={() => (draft = { kind: 'auto' })}><ArrowCounterClockwiseIcon weight="bold" size={14} aria-hidden="true" />{t('icons.reset')}</button>
    </div>

    <div class="main">
      <Seg options={tabs} value={tab} label={t('icons.title', { name: profile?.name ?? '' })} role="tablist" idPrefix="ipick" onchange={(v) => (tab = v as typeof tab)} />
      {#if !hasLogo && tab !== 'logo'}<p class="hint note">{t('icons.logo.none')}</p>{/if}

      {#if tab === 'gallery'}
        <div role="tabpanel" aria-labelledby="ipick-gallery" class="panel">
          {#if st.state === 'ready'}
            <div class="grid" role="radiogroup" aria-label={t('icons.tab.gallery')} bind:this={grid}>
              {#each cellIds as id, i (id)}
                {@const b = id === 'auto' ? autoBlock : st.blocks.find((x) => x.id === id)}
                {@const on = selected === id}
                <button type="button" role="radio" class="cell" class:on aria-checked={on} tabindex={on || (!selected && i === 0) ? 0 : -1}
                  onclick={() => pickBlock(id)} onkeydown={(e) => key(e, i)}>
                  <span class="th">
                    {#if b}<BlockIcon faces={b.faces} fill radius={10} tint={tone} />{:else}<Tile kind="profile" id={id} src={null} tint={tone} fill radius={10} />{/if}
                    {#if on}<span class="chk" aria-hidden="true"><CheckIcon weight="bold" size={12} /></span>{/if}
                  </span>
                  <span class="lbl">{id === 'auto' ? t('icons.auto') : b?.label}</span>
                  {#if id === 'auto' && autoBlock}<span class="sub">{autoBlock.label}</span>{/if}
                </button>
              {/each}
            </div>
            <p class="hint">{t('icons.autoHint')}</p>
          {:else if st.state === 'preparing'}
            <div class="state" role="status">
              <div class="skgrid" aria-hidden="true">{#each Array(14) as _, i (i)}<span class="skel sk"></span>{/each}</div>
              <div class="bar" aria-hidden="true"><span></span></div>
              <p><b>{t('icons.preparing')}</b></p>
              <p class="hint">{t('icons.privacy')}</p>
            </div>
          {:else if st.state === 'error'}
            <div class="state">
              <img class="px mk" src={brandIcon(128)} alt="" draggable="false" />
              <p class="err" role="alert">{t(st.key as Key)}</p>
              <button class="btn" type="button" onclick={() => prepareBlocks(true)}>{t('common.retry')}</button>
            </div>
          {:else}
            <div class="state">
              <img class="px mk" src={brandIcon(128)} alt="" draggable="false" />
              <p><b>{t('icons.missing')}</b></p>
              <p class="hint">{t('icons.privacy')}</p>
              <button class="btn accent" type="button" onclick={() => prepareBlocks()}>{t('icons.prepare')}</button>
            </div>
          {/if}
        </div>
      {:else if tab === 'upload'}
        <div role="tabpanel" aria-labelledby="ipick-upload" class="panel">
          <ImagePick onpick={onFile} {over} {bad} hint={t('icons.upload.hint')} />
        </div>
      {:else}
        <div role="tabpanel" aria-labelledby="ipick-logo" class="panel logo">
          {#if profile?.coverUrl}<span class="lg"><Tile kind="profile" id={profile.id} src={profile.coverUrl} {tint} size={112} notch={16} /></span>{/if}
          <div><p><b>{t('icons.logo.current')}</b></p><p class="hint">{t('icons.logo.hint')}</p>
            {#if profile?.iconSource === 'custom'}<button class="btn" type="button" disabled={busy} onclick={useLogo}>{t('icons.logo.use')}</button>{/if}</div>
        </div>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={!changed || busy || (draft.kind === 'block' && st.state !== 'ready')} onclick={apply}>{profileId ? t('icons.apply') : t('icons.use')}</button>
  {/snippet}
</Modal>

<style>
  .pk { display: grid; grid-template-columns: 168px minmax(0, 1fr); gap: 20px; min-height: 340px; }
  .prev { display: grid; grid-template-columns: minmax(0, 1fr); align-content: start; justify-items: center; gap: 10px; }
  .big { width: 168px; height: 168px; }
  .file { width: 100%; height: 100%; display: grid; place-items: center; background: var(--ink-3); color: var(--paper-dim); }
  .prev .btn { white-space: normal; height: auto; min-height: 28px; padding-block: 4px; text-align: center; }
  .cap { margin: 0; text-align: center; font-size: var(--fs-sm); max-width: 168px; overflow-wrap: anywhere; }
  .main { min-width: 0; display: grid; align-content: start; gap: 10px; }
  .note { margin: 0; }
  .panel { display: grid; gap: 10px; align-content: start; min-height: 260px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(84px, 1fr)); gap: 10px; padding: 4px; margin: -4px; }
  .cell { display: grid; gap: 4px; justify-items: stretch; text-align: center; padding: 6px 6px 8px; border: 1px solid transparent; border-radius: var(--r-m); }
  .cell:hover { background: var(--ink-3); }
  .cell.on { border-color: var(--accent-fill); background: var(--ink-3); }
  .cell:focus-visible { outline: 2px solid var(--focus); outline-offset: 1px; }
  .th { position: relative; display: block; aspect-ratio: 1; }
  .chk { position: absolute; right: 4px; top: 4px; width: 18px; height: 18px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); border-radius: var(--r-s); }
  .lbl { font-size: var(--fs-xs); font-weight: 600; line-height: 1.2; }
  .sub { font-size: var(--fs-xs); color: var(--paper-mute); line-height: 1.1; margin-top: -2px; }
  .state { display: grid; justify-items: center; align-content: center; gap: 10px; min-height: 260px; text-align: center; }
  .state p { margin: 0; max-width: 44ch; }
  .mk { width: 56px; height: 56px; opacity: 0.9; }
  .skgrid { width: 100%; display: grid; grid-template-columns: repeat(7, 1fr); gap: 10px; }
  .sk { aspect-ratio: 1; border-radius: var(--r-m); }
  .bar { width: 60%; height: 4px; background: var(--ink-4); overflow: hidden; }
  .bar span { display: block; width: 40%; height: 100%; background: var(--accent-fill); animation: slide 1.2s var(--ease) infinite; }
  .logo { grid-auto-flow: column; justify-content: start; align-items: center; gap: 16px; min-height: 0; padding-top: 8px; }
  .logo p { margin: 0 0 4px; }
  .lg { display: block; }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(250%); } }
  @media (prefers-reduced-motion: reduce) { .bar span { animation: none; width: 100%; opacity: 0.5; } }
  @media (max-width: 760px) { .pk { grid-template-columns: 1fr; } .prev { grid-template-columns: 96px 1fr; justify-items: start; align-items: center; } .big { width: 96px; height: 96px; } }
</style>
