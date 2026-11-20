<script lang="ts">
  import Img from './Img.svelte';
  import { fallbackImage } from '../lib/fallbackImage';
  // Background picker for an instance: Auto (wallpaper by version rule), Wallpapers, Minecraft panorama, Screenshot, Upload, None. Live preview in a mini hero.
  import { untrack } from 'svelte';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import ArrowCounterClockwiseIcon from 'phosphor-svelte/lib/ArrowCounterClockwiseIcon';
  import FileImageIcon from 'phosphor-svelte/lib/FileImageIcon';
  import Modal from './Modal.svelte';
  import Seg from './Seg.svelte';
  import PanoramaBackground from './PanoramaBackground.svelte';
  import ImagePick from './ImagePick.svelte';
  import { fmtDateTime } from '../lib/format';
  import { autoWallpaper, uniqueWallpapers, wallpapers } from '../lib/wallpapers';
  import { BLUR_MAX, backgroundStyle, choiceFromBackground, previewBackground, resolveBackground, sameBackground, type BackgroundChoice } from '../lib/instanceIcon';
  import { imageDrop, type PickedImage } from '../lib/pickImage';
  import { actions, app, errorText, getApi, lang, loadPanorama, panoramas, t, toast } from '../lib/stores.svelte';
  import type { Screenshot } from '../lib/types';

  let { profileId, onclose }: { profileId: string; onclose: () => void } = $props();
  const profile = $derived(app.profiles.find((p) => p.id === profileId) ?? null);
  const startBg = untrack(() => app.profiles.find((p) => p.id === profileId)?.background ?? { kind: 'auto' as const });
  const start: BackgroundChoice = choiceFromBackground(startBg, wallpapers);
  let draft = $state<BackgroundChoice>(start);
  type Tab = 'auto' | 'wallpapers' | 'panorama' | 'screenshot' | 'upload' | 'none';
  let tab = $state<Tab>(start.kind === 'keep' ? 'upload' : start.kind === 'none' ? 'none' : start.kind === 'panorama' ? 'panorama' : start.kind === 'wallpaper' ? 'wallpapers' : 'auto');
  // Dim (0..1) and blur (px) overrides; null = not touched (the saved value or the default stays). `reset` = back to auto with default look.
  let dimOv = $state<number | null>(null);
  let blurOv = $state<number | null>(null);
  let reset = $state(false);
  let busy = $state(false);
  let over = $state(false);
  let bad = $state(false);
  let shots = $state<Screenshot[] | null>(null);

  $effect(() => { void loadPanorama(profileId); });
  $effect(() => {
    if (shots) return;
    let dead = false;
    getApi().screenshotsList({ profileId }).then((l) => { if (!dead) shots = l; }, () => { if (!dead) shots = []; });
    return () => { dead = true; };
  });

  const pano = $derived(panoramas[profileId]);
  const hasStyle = 'dim' in startBg || 'blur' in startBg;
  const changed = $derived(reset || !sameBackground(draft, start) || dimOv !== null || blurOv !== null);
  const shown = $derived(previewBackground(draft, profile?.background ?? { kind: 'auto' }));
  // dim/blur persist across kind changes (only a reset clears them): the saved values apply until a slider overrides them
  const savedDim = 'dim' in startBg && typeof startBg.dim === 'number' ? startBg.dim : null;
  const savedBlur = 'blur' in startBg && typeof startBg.blur === 'number' ? startBg.blur : null;
  const effDim = $derived(dimOv ?? (reset ? null : savedDim));
  const effBlur = $derived(blurOv ?? (reset ? null : savedBlur));
  // the defaults of the shown background (the wallpaper's own scrim, the panorama's blur) fill in what is not set
  const defaults = $derived(backgroundStyle({ kind: 'auto' }, resolveBackground(shown, { mcVersion: profile?.mcVersion ?? '', profileId, panorama: pano, wallpapers })));
  const dimNow = $derived(effDim ?? defaults.dim);
  const blurNow = $derived(effBlur ?? defaults.blur);
  const all = uniqueWallpapers(wallpapers);
  const auto = $derived(autoWallpaper(wallpapers, profile?.mcVersion ?? '', profileId));
  const autoText = $derived(!auto ? t('bg.auto.empty') : auto.rule === 'version'
    ? t('bg.auto.version', { v: profile?.mcVersion ?? '', name: auto.wallpaper.label }) : t('bg.auto.generic', { name: auto.wallpaper.label }));
  const panoText = $derived(!pano ? t('bg.auto.none') : pano.source === 'own' ? t('bg.auto.own', { v: pano.mcVersion }) : t('bg.auto.nearest', { v: pano.mcVersion }));

  const tabs = $derived([
    { value: 'auto', label: t('bg.tab.auto') },
    ...(all.length ? [{ value: 'wallpapers', label: t('bg.tab.wallpapers') }] : []),
    // a tab with nothing behind it is not rendered: no panorama extracted, no screenshots taken
    ...(pano || tab === 'panorama' || draft.kind === 'panorama' ? [{ value: 'panorama', label: t('bg.tab.panorama') }] : []),
    ...(shots?.length || tab === 'screenshot' || draft.kind === 'screenshot' ? [{ value: 'screenshot', label: t('bg.tab.screenshot') }] : []),
    { value: 'upload', label: t('bg.tab.upload') },
    { value: 'none', label: t('bg.tab.none') },
  ]);
  function setTab(v: string) {
    tab = v as Tab;
    reset = false;
    if (tab === 'auto') draft = { kind: 'auto' };
    else if (tab === 'panorama') draft = { kind: 'panorama' };
    else if (tab === 'none') draft = { kind: 'none' };
  }
  function onFile(i: PickedImage) { bad = false; draft = { kind: 'image', path: i.path, name: i.name, preview: i.preview }; tab = 'upload'; }

  async function apply() {
    busy = true;
    try {
      const d = draft;
      if (reset) await actions.clearBackground(profileId); // auto + default dim and blur
      if (d.kind === 'auto') { if (!reset) await actions.setBackgroundKind(profileId, 'auto'); }
      else if (d.kind === 'panorama') await actions.setBackgroundKind(profileId, 'panorama');
      else if (d.kind === 'none') await actions.setBackgroundKind(profileId, 'none');
      else if (d.kind === 'wallpaper') await actions.setBackgroundKind(profileId, 'wallpaper', d.id);
      else if (d.kind === 'screenshot') await actions.setBackgroundScreenshot(profileId, d.name);
      else if (d.kind === 'image') await actions.setBackgroundImage(profileId, d.path);
      if (dimOv !== null || blurOv !== null) {
        await actions.setBackgroundStyle(profileId, { ...(dimOv !== null && { dim: dimOv }), ...(blurOv !== null && { blur: blurOv }) });
      }
      onclose();
    } catch (e) { toast(errorText(e), 'error'); busy = false; }
  }
</script>

<Modal title={t('bg.title', { name: profile?.name ?? '' })} size="lg" {onclose} onenter={changed ? apply : undefined}>
  <div class="bp" use:imageDrop={{ over: (b) => (over = b), drop: onFile, reject: () => { bad = true; tab = 'upload'; } }}>
    <div class="mini on-stage" role="img" aria-label={t('bg.preview')}>
      <PanoramaBackground {profileId} mcVersion={profile?.mcVersion ?? ''} background={shown} dim={effDim} blur={effBlur} tint={profile?.color ?? null} />
      <span class="mt"><b class="display">{profile?.name}</b><span class="mono">{profile?.mcVersion}</span></span>
    </div>

    <div class="tabscroll"><Seg options={tabs} value={tab} label={t('bg.title', { name: '' })} role="tablist" idPrefix="bpick" fill onchange={setTab} /></div>

    <div class="panel" role="tabpanel" aria-labelledby="bpick-{tab}">
      {#if tab === 'auto'}
        <p><b>{autoText}</b></p>
        <p class="hint">{t('bg.auto.hint')}</p>
      {:else if tab === 'panorama'}
        <p><b>{panoText}</b></p>
        <p class="hint">{t('bg.panorama.hint')}</p>
      {:else if tab === 'screenshot'}
        {#if !shots}
          <div class="g" aria-hidden="true">{#each Array(6) as _, i (i)}<span class="skel sh"></span>{/each}</div>
        {:else if shots.length === 0}
          <p><b>{t('bg.shots.empty')}</b></p><p class="hint">{t('bg.shots.emptyHint')}</p>
        {:else}
          <div class="g" role="radiogroup" aria-label={t('bg.tab.screenshot')}>
            {#each shots as s (s.name)}
              {@const on = draft.kind === 'screenshot' && draft.name === s.name}
              <button type="button" role="radio" aria-checked={on} class="th" class:on onclick={() => (draft = { kind: 'screenshot', name: s.name, url: s.url })}>
                <Img src={s.url} lazy fallback={fallbackImage('world', s.name, 300)} />
                <span class="cap mono">{fmtDateTime(s.taken, lang())}</span>
                {#if on}<span class="chk" aria-hidden="true"><CheckIcon weight="bold" size={12} /></span>{/if}
              </button>
            {/each}
          </div>
        {/if}
      {:else if tab === 'upload'}
        <ImagePick onpick={onFile} {over} {bad} hint={t('bg.upload.hint')} />
        {#if draft.kind === 'image' && !draft.preview}<p class="chip"><FileImageIcon weight="bold" size={16} aria-hidden="true" />{draft.name}</p>{/if}
      {:else if tab === 'wallpapers'}
        <div class="g" role="radiogroup" aria-label={t('bg.tab.wallpapers')}>
          {#each all as w (w.id)}
            {@const on = draft.kind === 'wallpaper' && draft.id === w.id}
            <button type="button" role="radio" aria-checked={on} class="th" class:on onclick={() => { reset = false; draft = { kind: 'wallpaper', id: w.id, label: w.label, url: w.url }; }}>
              <Img src={w.url} lazy fallback={fallbackImage('world', w.id, 300)} />
              <span class="cap">{w.label}</span>
              {#if on}<span class="chk" aria-hidden="true"><CheckIcon weight="bold" size={12} /></span>{/if}
            </button>
          {/each}
        </div>
      {:else}
        <p><b>{t('bg.none.title')}</b></p><p class="hint">{t('bg.none.hint')}</p>
      {/if}
    </div>

    {#if tab !== 'none'}
      <div class="look" role="group" aria-label={t('bg.look')}>
        <label class="sl" for="bg-dim"><span>{t('bg.dim')}</span>
          <input id="bg-dim" type="range" min="0" max="1" step="0.05" value={dimNow} aria-valuetext="{Math.round(dimNow * 100)} %" oninput={(e) => { dimOv = Number(e.currentTarget.value); }} />
          <output class="mono">{Math.round(dimNow * 100)} %</output></label>
        <label class="sl" for="bg-blur"><span>{t('bg.blur')}</span>
          <input id="bg-blur" type="range" min="0" max={BLUR_MAX} step="1" value={blurNow} aria-valuetext="{Math.round(blurNow)} px" oninput={(e) => { blurOv = Number(e.currentTarget.value); }} />
          <output class="mono">{Math.round(blurNow)} px</output></label>
      </div>
    {/if}
  </div>

  {#snippet footer()}
    <button class="btn ghost reset" type="button" disabled={draft.kind === 'auto' && !hasStyle && dimOv === null && blurOv === null && !changed} onclick={() => { draft = { kind: 'auto' }; tab = 'auto'; dimOv = null; blurOv = null; reset = true; }}><ArrowCounterClockwiseIcon weight="bold" size={14} aria-hidden="true" />{t('bg.reset')}</button>
    <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={!changed || busy} onclick={apply}>{t('bg.apply')}</button>
  {/snippet}
</Modal>

<style>
  .bp { display: grid; gap: 14px; }
  .mini { position: relative; height: 148px; border-radius: var(--r-m); overflow: hidden; border: 1px solid var(--line); display: flex; align-items: flex-end; }
  .mt { position: relative; display: grid; gap: 2px; padding: 12px 16px 12px 20px; color: var(--paper); }
  .mt b { font-size: var(--fs-2xl); }
  .mt .mono { font-size: var(--fs-sm); color: var(--paper-dim); }
  .tabscroll { overflow-x: auto; }
  .panel { min-height: 190px; display: grid; align-content: start; gap: 8px; }
  .panel p { margin: 0; }
  .g { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; max-height: 250px; overflow: auto; padding: 4px; margin: -4px; }
  .th { position: relative; display: block; aspect-ratio: 16 / 9; overflow: hidden; border-radius: var(--r-m); border: 2px solid transparent; background: var(--ink-3); }
  .th:hover { border-color: var(--line); }
  .th.on { border-color: var(--accent-fill); }
  .th:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
  .cap { position: absolute; left: 0; right: 0; bottom: 0; padding: 14px 8px 4px; font-size: var(--fs-xs); text-align: left; color: #f4f1ea; background: linear-gradient(0deg, rgba(11, 12, 14, 0.8), transparent); }
  .chk { position: absolute; right: 6px; top: 6px; width: 18px; height: 18px; display: grid; place-items: center; background: var(--accent-fill); color: var(--on-fill); border-radius: var(--r-s); }
  .sh { aspect-ratio: 16 / 9; border-radius: var(--r-m); }
  .chip { display: inline-flex; align-items: center; gap: 6px; color: var(--paper-dim); font-size: var(--fs-sm); }
  .reset { margin-right: auto; }
  .look { display: grid; gap: 8px; padding-top: 12px; border-top: 1px solid var(--line); }
  .sl { display: grid; grid-template-columns: 96px 1fr 52px; align-items: center; gap: 12px; font-size: var(--fs-sm); }
  .sl output { text-align: right; color: var(--paper-dim); }
</style>
