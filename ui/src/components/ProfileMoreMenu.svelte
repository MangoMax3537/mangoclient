<script lang="ts">
  // The "..." button of the profile page header: the shared instance menu (without "Open") plus the page-only items (analyze the latest crash,
  // export diagnostics). The same items open on right-click anywhere on the header (ProfilePage).
  import DotsThreeIcon from 'phosphor-svelte/lib/DotsThreeIcon';
  import { cm, openMenuFor } from '../lib/contextMenu.svelte';
  import { pageMenu } from '../lib/instanceMenu';
  import { t } from '../lib/stores.svelte';
  import type { ProfileView } from '../lib/types';

  let { profile }: { profile: ProfileView } = $props();
</script>

<button class="btn trig" type="button" aria-haspopup="menu" aria-expanded={cm.open && cm.anchor?.dataset.menuFor === profile.id} data-menu-for={profile.id}
  aria-label={t('library.menu', { name: profile.name })} onclick={(e) => openMenuFor(e.currentTarget, pageMenu(profile), t('library.menu', { name: profile.name }))}>
  <DotsThreeIcon weight="bold" size={18} aria-hidden="true" />
</button>

<style>
  .trig { width: 34px; height: 30px; padding: 0; justify-content: center; background: rgba(var(--scrim), 0.55); border-color: var(--hero-line); color: #f4f1ea; }
</style>
