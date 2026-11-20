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
  <div class="track"><div class="fill" style:transform="scaleX({frac})"></div></div>
  <ol>
    {#each LAUNCH_STAGES as s, i (s)}
      <li class:done={i < idx} class:cur={i === idx}>
        <span class="dot"></span>{t(`launch.${s}` as Key)}
      </li>
    {/each}
  </ol>
  <div class="meta mono" aria-live="polite">{pct}%{#if sub}&nbsp;&middot;&nbsp;{sub}{/if}</div>
</div>

<style>
  .lp { display: grid; gap: 10px; width: min(560px, 100%); }
  .track { height: 6px; background: rgba(244, 241, 234, 0.14); overflow: hidden; border-radius: 1px; }
  .fill { height: 100%; background: var(--mango); transform-origin: left; transition: transform 200ms var(--ease); }
  ol { display: flex; gap: 4px 16px; flex-wrap: wrap; list-style: none; margin: 0; padding: 0; font-size: 13px; color: var(--paper-mute); }
  li { display: inline-flex; align-items: center; gap: 6px; }
  .dot { width: 7px; height: 7px; background: var(--ink-4); border: 1px solid var(--paper-mute); }
  li.done { color: var(--paper-dim); }
  li.done .dot { background: var(--paper-dim); }
  li.cur { color: var(--paper); font-weight: 600; }
  li.cur .dot { background: var(--mango); border-color: var(--mango); }
  .meta { font-size: 13px; color: var(--paper-dim); min-height: 1.4em; }
</style>
