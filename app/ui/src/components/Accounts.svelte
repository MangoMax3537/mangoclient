<script lang="ts">
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
  import LockKeyIcon from 'phosphor-svelte/lib/LockKeyIcon';
  import SignInIcon from 'phosphor-svelte/lib/SignInIcon';
  import Tile from './Tile.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import MicrosoftButton from './MicrosoftButton.svelte';
  import PersistNotice from './PersistNotice.svelte';
  import { accountNeedsLogin, actions, app, auth, errorText, nav, t, toast } from '../lib/stores.svelte';
  import type { PublicAccount } from '../lib/types';

  let name = $state('');
  let error = $state('');
  let busy = $state(false);
  let pendingRemove = $state<PublicAccount | null>(null);
  let root: HTMLDivElement | undefined = $state();

  const valid = (s: string) => /^[A-Za-z0-9_]{3,16}$/.test(s);

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (!valid(name)) { error = t('accounts.nameInvalid'); return; }
    busy = true; error = '';
    try { await actions.addOffline(name); name = ''; } catch (err) { error = errorText(err); }
    busy = false;
  }
  async function refreshOne(a: PublicAccount) {
    if (await actions.refreshAccount(a.uuid)) toast(t('accounts.refreshed', { name: a.name }), 'ok');
  }
  async function confirmRemove() {
    const a = pendingRemove;
    pendingRemove = null;
    if (!a) return;
    await actions.removeAccount(a.uuid);
    toast(t('accounts.removed', { name: a.name }), 'info');
  }
  function onDoc(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) nav.accounts = false;
  }
  function onKey(e: KeyboardEvent) { if (e.key === 'Escape' && !pendingRemove) nav.accounts = false; }
  $effect(() => { root?.querySelector<HTMLElement>('button, input')?.focus(); });
</script>

<svelte:document onclick={onDoc} onkeydown={onKey} />
<div class="pop" bind:this={root} role="dialog" aria-label={t('accounts.title')}>
  <h2 class="eyebrow">{t('accounts.title')}</h2>
  <PersistNotice />
  <ul>
    {#each app.accounts as a (a.uuid)}
      {@const active = a.uuid === app.config?.selectedAccount}
      {@const relogin = accountNeedsLogin(a)}
      <li class:active>
        <div class="line">
          <button class="pick" type="button" onclick={() => actions.selectAccount(a.uuid)} aria-pressed={active} aria-label="{t('accounts.select')}: {a.name}">
            <Tile kind="account" id={a.uuid} src={a.headUrl} size={32} radius={4} />
            <span class="who"><b>{a.name}</b></span>
            {#if active}<CheckIcon weight="bold" size={16} aria-hidden="true" />{/if}
          </button>
          {#if a.type === 'microsoft' && !relogin}
            <button class="btn ghost ic" type="button" disabled={!!auth.refreshing[a.uuid]} aria-label="{t('accounts.refresh')}: {a.name}" title={t('accounts.refresh')} onclick={() => refreshOne(a)}>
              <span class:spin={!!auth.refreshing[a.uuid]}><ArrowsClockwiseIcon weight="bold" size={14} aria-hidden="true" /></span>
            </button>
          {/if}
          <button class="btn ghost ic" type="button" aria-label="{t('accounts.remove')}: {a.name}" title={t('accounts.remove')} onclick={() => (pendingRemove = a)}><TrashIcon weight="bold" size={14} aria-hidden="true" /></button>
        </div>
        <div class="badges">
          <span class="badge">{a.type === 'microsoft' ? t('accounts.msa') : t('accounts.offline')}</span>
          {#if active}<span class="badge use">{t('accounts.inUse')}</span>{/if}
          {#if a.credentialsLocked}
            <span class="badge warn" title={t('accounts.lockedHint')}><LockKeyIcon weight="bold" size={12} aria-hidden="true" />{t('accounts.locked')}<span class="sr-only">. {t('accounts.lockedHint')}</span></span>
          {/if}
          {#if relogin}
            <button class="badge act" type="button" disabled={auth.signingIn} title={t('accounts.expiredHint')} aria-label="{t('accounts.signInAgain')}: {a.name}. {t('accounts.expiredHint')}" onclick={() => actions.signInMicrosoft()}>
              <SignInIcon weight="bold" size={12} aria-hidden="true" />{t('accounts.signInAgain')}
            </button>
          {/if}
        </div>
      </li>
    {:else}
      <li class="hint">{t('accounts.empty')}</li>
    {/each}
  </ul>
  <form onsubmit={add}>
    <label class="eyebrow" for="offname">{t('accounts.addOffline')}</label>
    <div class="row">
      <input id="offname" class="field" bind:value={name} placeholder={t('accounts.offlineName')} maxlength="16" autocomplete="off" aria-invalid={!!error} aria-describedby={error ? 'offerr' : undefined} />
      <button class="btn" type="submit" disabled={busy}>{t('accounts.add')}</button>
    </div>
    {#if error}<div id="offerr" class="err" role="alert">{error}</div>{/if}
  </form>
  <MicrosoftButton />
  {#if pendingRemove}
    <ConfirmDialog title={t('accounts.removeTitle', { name: pendingRemove.name })} body={pendingRemove.type === 'microsoft' ? t('accounts.removeBodyMs') : t('accounts.removeBody')}
      confirmLabel={t('accounts.remove')} danger onconfirm={confirmRemove} oncancel={() => (pendingRemove = null)} />
  {/if}
</div>

<style>
  .pop { position: absolute; left: 60px; bottom: 0; z-index: 30; width: 360px; max-height: calc(100vh - 48px); overflow-y: auto; overflow-x: hidden; display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; box-sizing: border-box; padding: 14px; background: var(--ink-2); border: 1px solid var(--line); border-radius: var(--r-l); box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6); text-align: left; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: minmax(0, 1fr); gap: 6px; }
  li { display: grid; gap: 2px; padding-bottom: 6px; border-radius: var(--r-m); }
  li.active { background: rgba(255, 173, 50, 0.08); }
  .line { display: flex; align-items: center; }
  .pick { flex: 1; min-width: 0; width: auto; height: auto; display: flex; align-items: center; gap: 10px; padding: 6px 8px; text-align: left; color: var(--paper); justify-content: flex-start; }
  .who { display: grid; flex: 1; min-width: 0; line-height: 1.25; }
  .who b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ic { width: 32px; height: 32px; padding: 0; justify-content: center; flex: none; }
  .badges { display: flex; flex-wrap: wrap; gap: 6px; padding: 0 8px 0 54px; }
  .badge { display: inline-flex; align-items: center; gap: 4px; height: 22px; padding: 0 8px; border-radius: var(--r-s); border: 1px solid var(--line); background: var(--ink-3); color: var(--paper-dim); font-size: 12px; font-weight: 500; line-height: 1; }
  .badge.use { color: var(--mango); border-color: rgba(255, 173, 50, 0.4); }
  .badge.warn { color: var(--mango); border-color: rgba(255, 173, 50, 0.5); background: rgba(255, 173, 50, 0.08); }
  button.badge.act { cursor: pointer; color: var(--mango-ink); background: var(--mango); border-color: var(--mango); font-weight: 700; }
  button.badge.act:hover:not(:disabled) { background: #ffbd57; }
  button.badge.act:disabled { opacity: 0.5; }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  form { display: grid; gap: 6px; }
  .row { display: flex; gap: 8px; }
  .row .field { flex: 1; width: 0; }
  .row .btn { flex: none; }
</style>
