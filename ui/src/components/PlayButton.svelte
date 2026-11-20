<script lang="ts">
  // The Play button. Busy: the whole notched button is the track; an accent fill layer sweeps over its full area by percent (or an indeterminate
  // band), and the label is rendered twice, light on the track and dark on the fill, each clipped to its own part so no letter is ever half covered.
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import StopIcon from 'phosphor-svelte/lib/StopIcon';
  import { fillClip } from '../lib/playFill';

  interface Props {
    /** play = ready, busy = launching (progress or indeterminate), stop = game running */
    mode: 'play' | 'busy' | 'stop';
    label: string;
    disabled?: boolean;
    /** 0..1 while busy. null/undefined = indeterminate sweep. */
    progress?: number | null;
    onclick: () => void;
  }
  let { mode, label, disabled = false, progress = null, onclick }: Props = $props();
  const clip = $derived(fillClip(progress));
</script>

<button
  class="play notch" class:stop={mode === 'stop'} class:busy={mode === 'busy'} class:sweep={mode === 'busy' && clip.indeterminate}
  disabled={disabled || mode === 'busy'} aria-busy={mode === 'busy'} {onclick} type="button"
>
  {#if mode === 'busy'}
    <span class="lbl sizer" aria-hidden="true"><PlayIcon weight="bold" size={22} /><span>{label}</span></span>
    <span class="layer track" style:clip-path={clip.indeterminate ? undefined : clip.trackInset}><span class="lbl"><PlayIcon weight="bold" size={22} aria-hidden="true" /><span>{label}</span></span></span>
    <span class="layer fill" style:clip-path={clip.indeterminate ? undefined : clip.fillInset} aria-hidden="true"><span class="lbl"><PlayIcon weight="bold" size={22} /><span>{label}</span></span></span>
  {:else}
    <span class="lbl">
      {#if mode === 'stop'}<StopIcon weight="bold" size={22} aria-hidden="true" />{:else}<PlayIcon weight="bold" size={22} aria-hidden="true" />{/if}
      <span>{label}</span>
    </span>
  {/if}
</button>

<style>
  .play {
    --n: var(--notch-l);
    display: inline-grid; place-items: center; height: 60px; min-width: 188px; padding: 0; position: relative; overflow: hidden;
    background: var(--accent-fill); color: var(--on-fill); font: 800 22px/1 var(--font-display); letter-spacing: -0.01em;
    transition: transform var(--dur) var(--ease), background var(--dur) var(--ease);
  }
  .lbl { display: inline-flex; align-items: center; justify-content: center; gap: 12px; padding: 0 36px 0 26px; white-space: nowrap; }
  .play:hover:not(:disabled) { background: var(--accent-fill-hover); transform: translateY(-1px); }
  .play:active:not(:disabled) { transform: translateY(1px); }
  .play:disabled { background: var(--ink-4); color: var(--paper-mute); }
  /* running: the same shape, inverted, with Stop */
  .play.stop { background: var(--paper); color: var(--ink-0); }
  .play.stop:hover:not(:disabled) { background: var(--paper-dim); }
  /* busy: dim track, full-size layers stacked; both labels sit at the same spot */
  .play.busy:disabled { background: color-mix(in srgb, var(--accent-fill) 22%, var(--ink-3)); color: var(--paper); cursor: progress; transform: none; }
  .sizer { visibility: hidden; grid-area: 1 / 1; }
  .layer { position: absolute; inset: 0; display: grid; place-items: center; }
  .track { color: var(--paper); }
  .fill { background: var(--accent-fill); color: var(--on-fill); transition: clip-path var(--dur-slow) var(--ease); }
  /* indeterminate: a band (30% wide) sweeps across the whole button; the track label shows through where the band is not */
  .sweep .fill { animation: sweep 1.6s var(--ease) infinite; }
  @keyframes sweep { 0% { clip-path: inset(0 100% 0 0); } 50% { clip-path: inset(0 35% 0 35%); } 100% { clip-path: inset(0 0 0 100%); } }
  @media (prefers-reduced-motion: reduce) {
    .sweep .fill { animation: none !important; clip-path: inset(0 60% 0 0); }
    .fill { transition: none; }
  }
</style>
