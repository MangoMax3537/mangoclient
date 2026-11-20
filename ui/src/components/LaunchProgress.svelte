<script lang="ts">
  import { LAUNCH_STAGES, launchInfo, stageProgress, t } from '../lib/stores.svelte';
  import type { Key } from '../lib/i18n';

  let { profileId }: { profileId: string } = $props();
  const info = $derived(launchInfo(profileId));
  const idx = $derived(LAUNCH_STAGES.indexOf(info.state as (typeof LAUNCH_STAGES)[number]));
  const frac = $derived(stageProgress(info));
  const pct = $derived(Math.round(frac * 100));
  const sub = $derived(
    info.state === 'downloading' && info.progress && info.progress.phase !== 'done'
      ? `${t(`phase.${info.progress.phase}` as Key)} ${info.progress.done}/${info.progress.total}`
      : '',
  );
</script>

<div class="lp" role="progressbar" aria-label={t('launch.progress')} aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct}>
  <div class="track"><div class="fill step-end" style:transform="scaleX({frac})"></div></div>
  <ol>
    {#each LAUNCH_STAGES as s, i (s)}
      <li class:done={i < idx} class:cur={i === idx}>
        <span class="sq"></span>{t(`launch.${s}` as Key)}
      </li>
    {/each}
  </ol>
  <div class="meta mono" aria-live="polite">{pct}%{#if sub}<span class="sep">/</span>{sub}{/if}</div>
</div>

<style>
  .lp { display: grid; gap: 10px; width: min(600px, 100%); }
  .track { height: 4px; background: var(--line); overflow: hidden; }
  .fill { height: 100%; background: var(--accent-fill); transform-origin: left; transition: transform var(--dur-slow) var(--ease); }
  ol { display: flex; gap: 4px 18px; flex-wrap: wrap; list-style: none; margin: 0; padding: 0; font-size: var(--fs-sm); color: var(--paper-mute); }
  li { display: inline-flex; align-items: center; gap: 7px; }
  li .sq { width: 7px; height: 7px; background: transparent; box-shadow: inset 0 0 0 1px var(--paper-mute); }
  li.done { color: var(--paper-dim); }
  li.done .sq { background: var(--paper-dim); box-shadow: none; }
  li.cur { color: var(--paper); font-weight: 600; }
  li.cur .sq { background: var(--mango); box-shadow: none; animation: blink 1.2s ease-in-out infinite; }
  .meta { font-size: var(--fs-sm); color: var(--paper-dim); min-height: 1.4em; }
  .sep { margin: 0 8px; opacity: 0.5; }
  @keyframes blink { 50% { opacity: 0.4; } }
</style>
