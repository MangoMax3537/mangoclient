<script lang="ts">
  // Memory control: slider (256 MB steps) and a GB field kept in sync, markers for default / pack recommendation / pack cap / machine total,
  // the source of the value, reset buttons and live warnings. Saves are debounced and only sent when the value changed.
  import { onDestroy, untrack } from 'svelte';
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import ArrowCounterClockwiseIcon from 'phosphor-svelte/lib/ArrowCounterClockwiseIcon';
  import PackageIcon from 'phosphor-svelte/lib/PackageIcon';
  import { fmtRam } from '../lib/format';
  import { gbText, hintTarget, parseGb, RAM_STEP, RamSaver, ramMarkers, ramResets, ramSourceKey, ramWarnings, snapRam, type RamEnv } from '../lib/ram';
  import { t } from '../lib/stores.svelte';
  import type { RamHint, RamLimitsView, RamSource } from '../lib/types';

  interface Props {
    id: string;
    /** What a launch uses now (effectiveRamMb) or, for the global default, config.ram. */
    value: number;
    source?: RamSource | null;
    hint?: RamHint | null;
    limits: RamLimitsView;
    defaultMb: number;
    /** `instance`: resetting saves null. `global`: resetting saves the suggested value. */
    mode?: 'instance' | 'global';
    disabled?: boolean;
    onsave: (ram: number | null) => void;
  }
  let { id, value, source = null, hint = null, limits, defaultMb, mode = 'instance', disabled = false, onsave }: Props = $props();

  let draft = $state(untrack(() => value));
  let text = $state(untrack(() => gbText(value)));
  let bad = $state(false);
  const env = $derived<RamEnv>({ limits, defaultMb, hint });
  const saver = new RamSaver((v) => onsave(v), () => value);
  // follow the stored value while the user is not editing
  $effect(() => { const v = value; untrack(() => { if (!saver.dirty) { draft = v; text = gbText(v); bad = false; } }); });
  onDestroy(() => saver.flush());

  const markers = $derived(ramMarkers(env));
  const warnings = $derived(ramWarnings(draft, env));
  const resets = $derived(ramResets(draft, mode === 'global' ? (draft === defaultMb ? 'default' : 'user') : (source ?? 'user'), env));
  const pct = $derived(limits.maxMb > limits.minMb ? ((draft - limits.minMb) / (limits.maxMb - limits.minMb)) * 100 : 0);

  function set(mb: number) {
    draft = snapRam(mb, limits);
    text = gbText(draft);
    bad = false;
    saver.set(draft);
  }
  function onText(e: Event & { currentTarget: HTMLInputElement }) {
    text = e.currentTarget.value;
    const mb = parseGb(text);
    bad = mb === null;
    if (mb !== null && mb >= limits.minMb && mb <= limits.maxMb) { draft = snapRam(mb, limits); saver.set(draft); }
  }
  function commitText() {
    const mb = parseGb(text);
    if (mb === null) { text = gbText(draft); bad = false; return; }
    set(mb);
    saver.flush();
  }
  function resetDefault() { saver.cancel(); draft = mode === 'global' ? defaultMb : value; if (mode === 'global') { text = gbText(defaultMb); onsave(defaultMb); } else onsave(null); }
  function usePack() { const mb = hintTarget(hint); if (mb === null) return; saver.cancel(); draft = snapRam(mb, limits); text = gbText(draft); onsave(draft); }
  const markLeft = (p: number) => `calc(${p}% + ${(0.5 - p / 100) * 16}px)`;
</script>

<div class="ram" class:off={disabled}>
  <div class="top">
    <label for="{id}-gb" class="sr-only">{t('ram.gbLabel')}</label>
    {#if mode === 'instance' && source}<span class="src s-{source}" data-source={source}>{t(ramSourceKey(source))}</span>{/if}
    <output class="now mono" for="{id}-range {id}-gb" data-testid="ram-now">{fmtRam(draft)}</output>
  </div>

  <div class="trackrow">
    <div class="trackwrap">
      <input id="{id}-range" class="range" type="range" min={limits.minMb} max={limits.maxMb} step={RAM_STEP} value={draft} {disabled}
        aria-label={t('ram.sliderLabel')} aria-valuetext={fmtRam(draft)} style:--pct="{pct}%"
        oninput={(e) => set(Number(e.currentTarget.value))} />
      <div class="ticks" aria-hidden="true">
        {#each markers as m (m.kind)}<i class="tick k-{m.kind}" class:clamped={m.clamped} style:left={markLeft(m.pct)} data-kind={m.kind} data-pct={m.pct.toFixed(1)}></i>{/each}
      </div>
    </div>
    <div class="gb">
      <input id="{id}-gb" class="field mono" inputmode="decimal" autocomplete="off" value={text} {disabled} aria-invalid={bad}
        oninput={onText} onblur={commitText} onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); commitText(); } }} />
      <span class="unit mono" aria-hidden="true">GB</span>
    </div>
  </div>

  <ul class="legend">
    {#each markers as m (m.kind)}
      <li><i class="sw k-{m.kind}" aria-hidden="true"></i>{t(`ram.marker.${m.kind}`, { ram: fmtRam(m.mb) })}</li>
    {/each}
  </ul>

  <div class="btns">
    <button class="btn small" type="button" disabled={disabled || !resets.canDefault} onclick={resetDefault}>
      <ArrowCounterClockwiseIcon weight="bold" size={14} aria-hidden="true" />{mode === 'global' ? t('ram.useSuggestion', { ram: fmtRam(defaultMb) }) : t('ram.useDefault', { ram: fmtRam(defaultMb) })}
    </button>
    {#if mode === 'instance' && hint && resets.packMb !== null}
      <button class="btn small" type="button" disabled={disabled || !resets.canPack} onclick={usePack}>
        <PackageIcon weight="bold" size={14} aria-hidden="true" />{t('ram.usePack', { ram: fmtRam(resets.packMb) })}
      </button>
    {/if}
  </div>

  {#if bad}<div class="err" role="alert">{t('ram.gbInvalid')}</div>{/if}
  {#if warnings.length}
    <ul class="warns" role="status" aria-live="polite">
      {#each warnings as w (w.kind)}
        <li class="w-{w.kind}"><WarningIcon weight="bold" size={16} aria-hidden="true" /><span>{t(`ram.warn.${w.kind}`, { ram: fmtRam(w.mb), total: fmtRam(limits.totalMb) })}</span></li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .ram { display: grid; gap: 10px; }
  .ram.off { opacity: 0.6; }
  .top { display: flex; align-items: baseline; gap: 12px; }
  .now { font-size: 22px; font-weight: 500; color: var(--mango); font-family: var(--font-mono); }
  .src { font-size: 12px; font-weight: 600; padding: 2px 8px; border: 1px solid var(--line); border-radius: var(--r-s); color: var(--paper-dim); }
  .src.s-modpack { border-color: var(--leaf); color: var(--leaf); }
  .src.s-user { border-color: var(--mango); color: var(--mango); }
  .trackrow { display: flex; align-items: flex-start; gap: 16px; }
  .trackwrap { position: relative; flex: 1; min-width: 0; padding-top: 4px; }
  .range { width: 100%; margin: 0; accent-color: var(--mango); }
  .ticks { position: relative; height: 10px; margin-top: 2px; }
  .tick { position: absolute; top: 0; width: 2px; height: 10px; margin-left: -1px; background: var(--paper-mute); }
  .tick.clamped { background: none; border-left: 2px dashed currentColor; width: 0; }
  .k-default { background: var(--paper-dim); color: var(--paper-dim); }
  .k-recommended { background: var(--leaf); color: var(--leaf); }
  .k-min { background: var(--sky); color: var(--sky); }
  .k-cap { background: var(--tomato); color: var(--tomato); }
  .k-total { background: var(--paper-mute); color: var(--paper-mute); }
  .gb { display: flex; align-items: center; gap: 6px; }
  .gb .field { width: 72px; text-align: right; }
  .unit { color: var(--paper-mute); font-size: 13px; }
  .legend { display: flex; flex-wrap: wrap; gap: 4px 16px; list-style: none; margin: 0; padding: 0; font-size: 12.5px; color: var(--paper-dim); }
  .legend li { display: inline-flex; align-items: center; gap: 6px; }
  .sw { display: inline-block; width: 10px; height: 10px; border-radius: 2px; }
  .btns { display: flex; flex-wrap: wrap; gap: 8px; }
  .warns { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .warns li { display: flex; align-items: flex-start; gap: 8px; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--mango) 45%, transparent); border-radius: var(--r-m); background: var(--ink-1); font-size: 13.5px; line-height: 1.4; }
  .warns li :global(svg) { flex: none; margin-top: 1px; color: var(--mango); }
  .warns li.w-cap, .warns li.w-min { border-color: var(--tomato); }
  .warns li.w-cap :global(svg), .warns li.w-min :global(svg) { color: var(--tomato); }
</style>
