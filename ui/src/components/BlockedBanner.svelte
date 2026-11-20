<script lang="ts">
  // "N mods still missing" notice on the instance page; opens the blocked-mods dialog.
  import ProhibitIcon from 'phosphor-svelte/lib/ProhibitIcon';
  import { openBlocked, t, tp } from '../lib/stores.svelte';
  import type { ProfileView } from '../lib/types';

  let { profile }: { profile: ProfileView } = $props();
</script>

{#if profile.pendingBlocked > 0}
  <div class="bb" role="status">
    <ProhibitIcon weight="bold" size={18} aria-hidden="true" />
    <div class="tx">
      <b>{tp('blocked.banner', profile.pendingBlocked)}</b>
      <span class="hint">{t('blocked.banner.hint')}</span>
    </div>
    <button class="btn accent" type="button" onclick={() => openBlocked(profile.id)}>{t('blocked.banner.button')}</button>
  </div>
{/if}

<style>
  .bb { display: flex; align-items: center; gap: 12px; padding: 10px 12px 10px 14px; border: 1px solid color-mix(in srgb, var(--mango) 40%, transparent); border-left-width: 3px; border-radius: var(--r-m); background: color-mix(in srgb, var(--mango) 9%, transparent); }
  .bb > :global(svg) { flex: none; color: var(--mango); }
  .tx { display: grid; flex: 1; min-width: 0; }
  .tx b { font-weight: 600; }
</style>
