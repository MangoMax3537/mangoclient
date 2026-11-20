<script lang="ts">
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import StopIcon from 'phosphor-svelte/lib/StopIcon';

  interface Props {
    mode: 'play' | 'busy' | 'stop';
    label: string;
    disabled?: boolean;
    onclick: () => void;
  }
  let { mode, label, disabled = false, onclick }: Props = $props();
</script>

<button class="play" class:stop={mode === 'stop'} class:busy={mode === 'busy'} {disabled} {onclick} type="button">
  {#if mode === 'stop'}<StopIcon weight="bold" size={20} aria-hidden="true" />{:else}<PlayIcon weight="bold" size={20} aria-hidden="true" />{/if}
  <span>{label}</span>
</button>

<style>
  .play {
    --n: 12px;
    display: inline-flex; align-items: center; gap: 10px; height: 52px; padding: 0 30px 0 24px; min-width: 164px;
    background: var(--mango); color: var(--mango-ink); font: 800 17px/1 var(--font-display); letter-spacing: 0.01em;
    clip-path: polygon(var(--n) 0, 100% 0, 100% calc(100% - var(--n)), calc(100% - var(--n)) 100%, 0 100%, 0 var(--n));
    transition: transform var(--dur) var(--ease), background var(--dur) var(--ease);
  }
  .play:hover:not(:disabled) { background: #ffbd57; transform: translateY(-1px); }
  .play:active:not(:disabled) { transform: translateY(1px); }
  .play:disabled { background: var(--ink-4); color: var(--paper-mute); }
  .play.stop { background: var(--paper); }
  .play.busy { background: #b97a1f; color: #fff; }
  .play:focus-visible { outline: 2px solid var(--paper); outline-offset: -4px; border-radius: 0; }
</style>
