<script lang="ts">
  import WindowsLogoIcon from 'phosphor-svelte/lib/WindowsLogoIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import LoginError from './LoginError.svelte';
  import { actions, auth, t } from '../lib/stores.svelte';
</script>

<div class="wrap">
  <button class="btn ms" type="button" disabled={auth.signingIn} aria-busy={auth.signingIn} onclick={() => actions.signInMicrosoft()}>
    {#if auth.signingIn}<span class="spin"><CircleNotchIcon weight="bold" size={16} aria-hidden="true" /></span>{t('login.pending')}
    {:else}<WindowsLogoIcon weight="bold" size={16} aria-hidden="true" />{t('accounts.microsoft')}{/if}
  </button>
  <p class="hint pend" role="status">{#if auth.signingIn}{t('login.pendingHint')}{/if}</p>
  {#if auth.error}<LoginError error={auth.error} />{/if}
</div>

<style>
  .wrap { display: grid; gap: 8px; }
  .ms { justify-content: center; }
  .pend { margin: 0; text-align: left; }
  .pend:empty { display: none; }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
