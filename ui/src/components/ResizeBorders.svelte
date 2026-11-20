<script lang="ts">
  // Eight invisible hit zones along the window border (4 edges 6 px, 4 corners 10 px) that start a native resize drag. The window is
  // frameless on Windows and Linux; rendered only there and not while maximized or fullscreen.
  import { CORNER_PX, EDGE_PX, RESIZE_ZONES, titleBarModel, windowControls, type ResizeDirection } from '../lib/titlebar';
  import { getApi, platform, win } from '../lib/stores.svelte';

  const on = $derived(titleBarModel(platform.mode, win).resizeZones);
  function down(e: MouseEvent, dir: ResizeDirection) {
    if (e.button !== 0) return;
    e.preventDefault(); e.stopPropagation();
    void windowControls(getApi()).startResize(dir);
  }
</script>

{#if on}
  {#each RESIZE_ZONES as z (z.dir)}
    <div class="rz {z.dir}" style:cursor={z.cursor} style:--e="{EDGE_PX}px" style:--c="{CORNER_PX}px" role="presentation" onmousedown={(e) => down(e, z.dir)}></div>
  {/each}
{/if}

<style>
  .rz { position: fixed; z-index: 1000; }
  .north { top: 0; left: var(--c); right: var(--c); height: var(--e); }
  .south { bottom: 0; left: var(--c); right: var(--c); height: var(--e); }
  .west { left: 0; top: var(--c); bottom: var(--c); width: var(--e); }
  .east { right: 0; top: var(--c); bottom: var(--c); width: var(--e); }
  .northwest { top: 0; left: 0; width: var(--c); height: var(--c); }
  .northeast { top: 0; right: 0; width: var(--c); height: var(--c); }
  .southwest { bottom: 0; left: 0; width: var(--c); height: var(--c); }
  .southeast { bottom: 0; right: 0; width: var(--c); height: var(--c); }
</style>
