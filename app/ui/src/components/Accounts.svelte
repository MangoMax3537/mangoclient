<script lang="ts">
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import WindowsLogoIcon from 'phosphor-svelte/lib/WindowsLogoIcon';
  import Tile from './Tile.svelte';
  import { actions, app, errorText, nav, t } from '../lib/stores.svelte';
  import { getApi } from '../lib/stores.svelte';

  let name = $state('');
  let error = $state('');
  let busy = $state(false);
  let root: HTMLDivElement | undefined = $state();

  const valid = (s: string) => /^[A-Za-z0-9_]{3,16}$/.test(s);

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (!valid(name)) { error = t('accounts.nameInvalid'); return; }
    busy = true; error = '';
    try { await actions.addOffline(name); name = ''; } catch (err) { error = errorText(err); }
    busy = false;
  }
  async function microsoft() {
    error = '';
    try { await getApi().accountSignInMicrosoft(); } catch (err) { error = errorText(err); }
  }
  function onDoc(e: MouseEvent) {
    if (root && !root.contains(e.target as Node)) nav.accounts = false;
  }
  function onKey(e: KeyboardEvent) { if (e.key === 'Escape') nav.accounts = false; }
  $effect(() => { root?.querySelector<HTMLElement>('button, input')?.focus(); });
</script>

<svelte:document onclick={onDoc} onkeydown={onKey} />
<div class="pop" bind:this={root} role="dialog" aria-label={t('accounts.title')}>
  <h2 class="eyebrow">{t('accounts.title')}</h2>
  <ul>
    {#each app.accounts as a (a.uuid)}
      {@const active = a.uuid === app.config?.selectedAccount}
      <li class:active>
        <button class="pick" type="button" onclick={() => actions.selectAccount(a.uuid)} aria-pressed={active} aria-label="{t('accounts.select')}: {a.name}">
          <Tile kind="account" id={a.uuid} src={a.headUrl} size={32} radius={4} />
          <span class="who"><b>{a.name}</b>
            <small>{a.type === 'microsoft' ? t('accounts.msa') : t('accounts.offline')}{#if a.expired}&nbsp;&middot;&nbsp;<span class="err">{t('accounts.expired')}</span>{/if}</small>
          </span>
          {#if active}<CheckIcon weight="bold" size={16} aria-hidden="true" />{/if}
        </button>
        <button class="btn ghost rm" type="button" aria-label="{t('accounts.remove')}: {a.name}" onclick={() => actions.removeAccount(a.uuid)}><TrashIcon weight="bold" size={14} aria-hidden="true" /></button>
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
  <button class="btn ms" type="button" onclick={microsoft}><WindowsLogoIcon weight="bold" size={16} aria-hidden="true" />{t('accounts.microsoft')}</button>
</div>

<style>
  .pop { position: absolute; left: 60px; bottom: 0; z-index: 30; width: 320px; display: grid; gap: 12px; padding: 14px; background: var(--ink-2); border: 1px solid var(--line); border-radius: var(--r-l); box-shadow: 0 16px 40px rgba(0, 0, 0, 0.6); text-align: left; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  li { display: flex; align-items: center; border-radius: var(--r-m); }
  li.active { background: rgba(255, 173, 50, 0.08); }
  .pick { flex: 1; width: auto; height: auto; display: flex; align-items: center; gap: 10px; padding: 6px 8px; text-align: left; color: var(--paper); justify-content: flex-start; }
  .who { display: grid; flex: 1; line-height: 1.25; }
  small { color: var(--paper-dim); font-size: 12.5px; }
  .rm { width: 32px; height: 32px; padding: 0; justify-content: center; }
  form { display: grid; gap: 6px; }
  .row { display: flex; gap: 8px; }
  .row .field { flex: 1; }
  .ms { justify-content: center; }
</style>
