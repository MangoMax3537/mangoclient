<script lang="ts">
  /** Data-driven stepped sparkline (SVG, no library). `values` are plain numbers, oldest first. */
  let { values, label, height = 64 }: { values: number[]; label: string; height?: number } = $props();
  const W = 100;
  const max = $derived(Math.max(1, ...values));
  const ys = $derived(values.map((v) => 4 + (1 - v / max) * 92));
  const step = $derived(values.length > 1 ? W / (values.length - 1) : W);
  // step-after path: hold the value until the next sample, then jump
  const line = $derived(values.length === 0 ? '' : ys.map((y, i) => (i === 0 ? `M0 ${y}` : `H${(i * step).toFixed(2)} V${y.toFixed(2)}`)).join(' ') + ` H${W}`);
  const area = $derived(line ? `${line} V100 H0 Z` : '');
  const last = $derived(ys.length ? ys[ys.length - 1] : 100);
</script>

<div class="spark" style:height="{height}px" role="img" aria-label={label}>
  <svg viewBox="0 0 {W} 100" preserveAspectRatio="none" aria-hidden="true">
    <line x1="0" x2={W} y1="99.5" y2="99.5" class="base" vector-effect="non-scaling-stroke" />
    {#if area}<path d={area} class="area" />{/if}
    {#if line}<path d={line} class="ln" vector-effect="non-scaling-stroke" />{/if}
  </svg>
  {#if values.length}<span class="sq warn end" style:top="calc({last}% - 4px)"></span>{/if}
</div>

<style>
  .spark { position: relative; width: 100%; }
  svg { width: 100%; height: 100%; display: block; overflow: visible; }
  .base { stroke: var(--line); stroke-width: 1; }
  .area { fill: var(--mango); opacity: 0.14; }
  .ln { fill: none; stroke: var(--mango); stroke-width: 2; stroke-linejoin: miter; }
  .end { position: absolute; right: -4px; }
</style>
