<script lang="ts">
  import { brandIcon } from '../lib/fallbackImage';

  /** The profile's real cover, or null for the clean token surface with the MangoClient icon as watermark. */
  let { cover = null, tint = null }: { cover?: string | null; tint?: string | null } = $props();
  let failed = $state(false);
  $effect(() => { void cover; failed = false; });
  const real = $derived(!!cover && !failed);
</script>

<div class="stage" style:--tint={tint ?? 'var(--accent-fill)'} aria-hidden="true">
  {#if real}
    <img class="art" src={cover} alt="" draggable="false" onerror={() => (failed = true)} />
  {:else}
    <div class="surface"></div>
    <img class="mark px" src={brandIcon(512)} alt="" draggable="false" />
  {/if}
  <div class="scrim" class:plain={!real}></div>
  <div class="bar"></div>
</div>

<style>
  .stage { position: absolute; inset: 0; overflow: hidden; background: var(--hero-bg); }
  .art { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; transform-origin: 70% 40%; animation: kb 36s ease-out infinite alternate; }
  .surface { position: absolute; inset: 0; background: radial-gradient(90% 120% at 82% 38%, color-mix(in srgb, var(--tint) 20%, transparent), transparent 62%), var(--hero-bg); }
  .mark { position: absolute; right: 6%; top: 50%; height: 104%; width: auto; aspect-ratio: 1; opacity: 0.09; transform: translateY(-50%); animation: drift 40s ease-out infinite alternate; }
  .scrim { position: absolute; inset: 0; background: linear-gradient(90deg, rgba(var(--scrim), 0.94) 0%, rgba(var(--scrim), 0.72) 42%, rgba(var(--scrim), 0.12) 100%), linear-gradient(0deg, rgba(var(--scrim), 0.65), transparent 55%); }
  .scrim.plain { background: linear-gradient(90deg, rgba(var(--scrim), 0.35), transparent 70%); }
  .bar { position: absolute; left: 0; top: 0; bottom: 0; width: 4px; background: var(--tint); }
  @keyframes kb { to { transform: scale(1.08) translate3d(-1%, -1%, 0); } }
  @keyframes drift { to { transform: translateY(-50%) scale(1.04); } }
</style>
