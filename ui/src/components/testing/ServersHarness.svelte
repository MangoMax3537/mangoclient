<script lang="ts">
  // Test harness: the Home or Servers view with the global join dialog, like the app shell.
  import Home from '../Home.svelte';
  import Servers from '../Servers.svelte';
  import ServerJoinDialog from '../ServerJoinDialog.svelte';
  import { app, nav, servers } from '../../lib/stores.svelte';
</script>

{#if app.ready}
  {#if nav.view === 'home'}<Home />{:else}<Servers />{/if}
  {#if servers.join}<ServerJoinDialog server={servers.join} onclose={() => (servers.join = null)} />{/if}
{/if}
