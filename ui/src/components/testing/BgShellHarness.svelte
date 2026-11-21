<script lang="ts">
  // Test harness: the engine mounted exactly like the app shell (same props derivation as App.svelte), views switched through `nav`.
  import AppBackground from '../AppBackground.svelte';
  import { engineProfile } from '../../lib/bgMode';
  import { app, nav, selectedProfile } from '../../lib/stores.svelte';
  const bgProfile = $derived(engineProfile(nav.view, nav.profileId, app.profiles, selectedProfile()));
  const bgMode = $derived(nav.view === 'home' && bgProfile ? 'hero' : 'full');
</script>

<main>
  <AppBackground profileId={bgProfile?.id ?? ''} mcVersion={bgProfile?.mcVersion ?? ''} background={bgProfile?.background ?? { kind: 'auto' }} tint={bgProfile?.color ?? null} mode={bgMode} />
  <div class="view" data-view={nav.view}>{nav.view}</div>
</main>
