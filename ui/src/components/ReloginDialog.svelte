<script lang="ts">
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import Modal from './Modal.svelte';
  import LoginError from './LoginError.svelte';
  import { actions, auth, selectedAccount, t } from '../lib/stores.svelte';

  const acc = $derived(selectedAccount());
  const close = () => { auth.relogin = null; auth.error = null; };
</script>

<Modal title={t('relogin.title')} onclose={close} locked={auth.signingIn}>
  <div class="msg">
    <p>{acc?.type === 'microsoft' ? t('relogin.body', { name: acc.name }) : t('relogin.bodyGeneric')}</p>
    <p class="hint">{t('relogin.hint')}</p>
    <p class="hint" role="status">{#if auth.signingIn}{t('login.pendingHint')}{/if}</p>
    {#if auth.error}<LoginError error={auth.error} />{/if}
  </div>
  {#snippet footer()}
    <button class="btn ghost" type="button" disabled={auth.signingIn} onclick={close}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={auth.signingIn} aria-busy={auth.signingIn} onclick={() => actions.reloginAndRetry()}>
      {#if auth.signingIn}<span class="spin"><CircleNotchIcon weight="bold" size={16} aria-hidden="true" /></span>{t('login.pending')}{:else}{t('accounts.signInAgain')}{/if}
    </button>
  {/snippet}
</Modal>

<style>
  .msg { display: grid; gap: 8px; }
  p { margin: 0; color: var(--paper-dim); }
  .hint:empty { display: none; }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
