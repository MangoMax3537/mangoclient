<script lang="ts">
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import StopIcon from 'phosphor-svelte/lib/StopIcon';

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
  const pct = $derived(progress == null ? null : Math.max(0, Math.min(1, progress)) * 100);
</script>

<button
  class="play notch" class:stop={mode === 'stop'} class:busy={mode === 'busy'} class:sweep={mode === 'busy' && pct === null}
  disabled={disabled || mode === 'busy'} aria-busy={mode === 'busy'} {onclick} type="button"
>
  <span class="lbl">
    {#if mode === 'stop'}<StopIcon weight="bold" size={22} aria-hidden="true" />{:else}<PlayIcon weight="bold" size={22} aria-hidden="true" />{/if}
    <span>{label}</span>
  </span>
  {#if mode === 'busy'}
    <span class="fill" style:clip-path={pct === null ? undefined : `inset(0 ${100 - pct}% 0 0)`} aria-hidden="true">
      <span class="lbl over"><PlayIcon weight="bold" size={22} /><span>{label}</span></span>
    </span>
  {/if}
</button>

<style>
  .play {
    --n: var(--notch-l);
    display: inline-grid; align-items: center; height: 60px; min-width: 188px; padding: 0; position: relative;
    background: var(--accent-fill); color: var(--on-fill); font: 800 22px/1 var(--font-display); letter-spacing: -0.01em;
    transition: transform var(--dur) var(--ease), background var(--dur) var(--ease);
  }
  .play > * { grid-area: 1 / 1; }
  .lbl { display: inline-flex; align-items: center; gap: 12px; padding: 0 36px 0 26px; white-space: nowrap; }
  .play:hover:not(:disabled) { background: var(--accent-fill-hover); transform: translateY(-1px); }
  .play:active:not(:disabled) { transform: translateY(1px); }
  .play:disabled { background: var(--ink-4); color: var(--paper-mute); }
  /* running: the same shape, inverted, with Stop */
  .play.stop { background: var(--paper); color: var(--ink-0); }
  .play.stop:hover:not(:disabled) { background: var(--paper-dim); }
  /* busy: dark base, accent fill grows left to right with its own copy of the label in dark ink */
  .play.busy:disabled { background: color-mix(in srgb, var(--accent-fill) 22%, var(--ink-3)); color: var(--paper); cursor: progress; }
  .fill { background: var(--accent-fill); color: var(--on-fill); display: grid; align-items: center; transition: clip-path var(--dur-slow) var(--ease); }
  .fill .over { grid-area: 1 / 1; }
  .sweep .fill { animation: sweep 1.6s var(--ease) infinite; }
  @keyframes sweep { 0% { clip-path: inset(0 100% 0 0); } 50% { clip-path: inset(0 0 0 0); } 100% { clip-path: inset(0 0 0 100%); } }
</style>
