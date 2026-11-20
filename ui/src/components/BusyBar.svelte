<script lang="ts">
  /** Indeterminate progress for work without byte counts (zipping, restoring). Animates transform only; static under reduced motion. */
  let { label }: { label: string } = $props();
</script>

<div class="busy" role="status" aria-live="polite">
  <span class="txt">{label}</span>
  <span class="bar" aria-hidden="true"><span></span></span>
</div>

<style>
  .busy { display: grid; gap: 6px; }
  .txt { font-size: var(--fs-sm); color: var(--paper-dim); }
  .bar { display: block; height: 6px; background: var(--ink-4); border-radius: 2px; overflow: hidden; }
  .bar span { display: block; width: 40%; height: 100%; background: var(--accent-fill); animation: slide 1.1s var(--ease) infinite; }
  @keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(250%); } }
  @media (prefers-reduced-motion: reduce) { .bar span { animation: none; width: 100%; opacity: 0.6; } }
</style>
