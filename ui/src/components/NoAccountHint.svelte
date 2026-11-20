<script lang="ts">
  // A disabled Play stays visible, so say why and offer the fix right there.
  import UserPlusIcon from 'phosphor-svelte/lib/UserPlusIcon';
  import { nav, t } from '../lib/stores.svelte';
</script>

<span class="nah" role="note">
  <span>{t('play.noAccount')}</span>
  <button class="btn small" type="button" onclick={(e) => { e.stopPropagation(); nav.accounts = true; }}><UserPlusIcon weight="bold" size={14} aria-hidden="true" />{t('play.addAccount')}</button>
</span>

<style>
  .nah { display: inline-flex; align-items: center; gap: var(--s-2); padding: 4px 6px 4px 12px; border-left: 3px solid var(--mango); font-size: var(--fs-sm); color: var(--hero-dim, var(--paper-dim)); background: rgba(0, 0, 0, 0.28); }
</style>
