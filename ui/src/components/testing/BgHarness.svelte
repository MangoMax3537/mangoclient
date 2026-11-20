<script lang="ts">
  // Test harness: the background engine next to the Home view, like the app shell.
  import AppBackground from '../AppBackground.svelte';
  import Home from '../Home.svelte';
  import { app, selectedProfile } from '../../lib/stores.svelte';
  const p = $derived(selectedProfile());
</script>

<main>
  {#if p}<AppBackground profileId={p.id} mcVersion={p.mcVersion} background={{ kind: 'auto' }} mode="hero" />{/if}
  {#if app.ready}<div class="view"><Home /></div>{/if}
</main>
