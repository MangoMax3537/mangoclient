<script lang="ts">
  // Test harness: the engine mounted exactly like the app shell (same props derivation as App.svelte), views switched through `nav`.
  import AppBackground from '../AppBackground.svelte';
  import { engineProfile } from '../../lib/bgMode';
  import { app, nav, selectedProfile, ui } from '../../lib/stores.svelte';
  const bgProfile = $derived(engineProfile(nav.view, nav.profileId, app.profiles, selectedProfile()));
  const bgMode = $derived(nav.view === 'home' && bgProfile ? 'hero' : 'full');
  const bgOn = $derived(app.ready && (nav.view === 'home' ? !!bgProfile || ui.backgroundEverywhere : ui.backgroundEverywhere));
</script>

<main>
  {#if bgOn}<AppBackground profileId={bgProfile?.id ?? ''} mcVersion={bgProfile?.mcVersion ?? ''} background={bgProfile?.background ?? { kind: 'auto' }} tint={bgProfile?.color ?? null} mode={bgMode} />{/if}
  <div class="view" data-view={nav.view}>{nav.view}</div>
</main>
