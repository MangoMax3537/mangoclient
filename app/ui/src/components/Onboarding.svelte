<script lang="ts">
  import { onMount } from 'svelte';
  import ArrowRightIcon from 'phosphor-svelte/lib/ArrowRightIcon';
  import ArrowLeftIcon from 'phosphor-svelte/lib/ArrowLeftIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import Tile from './Tile.svelte';
  import MicrosoftButton from './MicrosoftButton.svelte';
  import PersistNotice from './PersistNotice.svelte';
  import { heroFallback } from '../lib/fallbackImage';
  import { fmtRam } from '../lib/format';
  import type { Key } from '../lib/i18n';
  import { actions, app, errorText, t } from '../lib/stores.svelte';

  const STEPS = ['welcome', 'language', 'account', 'ram', 'done'] as const;
  let step = $state(0);
  let name = $state('');
  let error = $state('');
  let busy = $state(false);
  let root: HTMLDivElement | undefined = $state();
  let ram = $state(app.config?.ram ?? 4096);

  const maxRam = $derived(Math.max(2048, Math.floor((app.totalRamMb || 8192) * 0.75 / 512) * 512));
  const recommended = $derived(Math.min(maxRam, Math.max(2048, Math.round((app.totalRamMb || 8192) / 4 / 512) * 512)));
  const cfg = $derived(app.config);

  onMount(() => { root?.querySelector<HTMLElement>('h1')?.focus(); });
  $effect(() => { void step; queueMicrotask(() => root?.querySelector<HTMLElement>('h1')?.focus()); });

  async function addOffline(e: SubmitEvent) {
    e.preventDefault();
    if (!/^[A-Za-z0-9_]{3,16}$/.test(name)) { error = t('accounts.nameInvalid'); return; }
    busy = true; error = '';
    try { await actions.addOffline(name); name = ''; } catch (err) { error = errorText(err); }
    busy = false;
  }
  async function finish() {
    await actions.setConfig({ ram, firstRunDone: true });
  }
  function next() { if (step === 3) actions.setConfig({ ram }); step = Math.min(STEPS.length - 1, step + 1); }
</script>

<div class="ob" bind:this={root} role="dialog" aria-modal="true" aria-labelledby="ob-h" style:--bg="url({heroFallback()})">
  <div class="bg px" aria-hidden="true"></div>
  <div class="card">
    <ol class="dots" aria-label={t('onb.progress', { n: step + 1, total: STEPS.length })}>
      {#each STEPS as s, i (s)}<li class:on={i === step} class:done={i < step} aria-current={i === step ? 'step' : undefined}><span class="sr-only">{t(`onb.step.${s}` as Key)}</span></li>{/each}
    </ol>

    {#if step === 0}
      <div class="eyebrow">{t('app.name')}</div>
      <h1 id="ob-h" tabindex="-1">{t('onb.welcome.title')}</h1>
      <p class="lead">{t('onb.welcome.body')}</p>
    {:else if step === 1}
      <h1 id="ob-h" tabindex="-1">{t('onb.language.title')}</h1>
      <div class="langs" role="radiogroup" aria-label={t('settings.language')}>
        {#each ['de', 'en'] as l (l)}
          <button type="button" role="radio" aria-checked={cfg?.language === l} class:on={cfg?.language === l} onclick={() => actions.setConfig({ language: l as 'de' | 'en' })}>
            <b>{t(`settings.language.${l}` as Key)}</b>
            {#if cfg?.language === l}<CheckIcon weight="bold" size={18} aria-hidden="true" />{/if}
          </button>
        {/each}
      </div>
    {:else if step === 2}
      <h1 id="ob-h" tabindex="-1">{t('onb.account.title')}</h1>
      <p class="lead">{t('onb.account.body')}</p>
      <MicrosoftButton />
      <PersistNotice />
      <form onsubmit={addOffline}>
        <label for="ob-name" class="eyebrow">{t('accounts.addOffline')}</label>
        <div class="row">
          <input id="ob-name" class="field" bind:value={name} maxlength="16" autocomplete="off" placeholder={t('accounts.offlineName')} aria-invalid={!!error} aria-describedby={error ? 'ob-err' : undefined} />
          <button class="btn" type="submit" disabled={busy}>{t('accounts.add')}</button>
        </div>
      </form>
      {#if error}<div id="ob-err" class="err" role="alert">{error}</div>{/if}
      {#if app.accounts.length}
        <ul class="accs" aria-label={t('accounts.title')}>
          {#each app.accounts as a (a.uuid)}<li><Tile kind="account" id={a.uuid} src={a.headUrl} size={28} radius={4} /><b>{a.name}</b><span class="hint">{a.type === 'microsoft' ? t('accounts.msa') : t('accounts.offline')}</span></li>{/each}
        </ul>
      {/if}
    {:else if step === 3}
      <h1 id="ob-h" tabindex="-1">{t('onb.ram.title')}</h1>
      <p class="lead">{t('onb.ram.body', { total: fmtRam(app.totalRamMb) })}</p>
      <div class="ram">
        <input id="ob-ram" type="range" min="1024" max={maxRam} step="512" bind:value={ram} aria-label={t('settings.ram')} aria-valuetext={fmtRam(ram)} />
        <output class="mono"><b>{fmtRam(ram)}</b></output>
      </div>
      <button class="btn ghost" type="button" onclick={() => (ram = recommended)}>{t('onb.ram.recommended', { ram: fmtRam(recommended) })}</button>
    {:else}
      <h1 id="ob-h" tabindex="-1">{t('onb.done.title')}</h1>
      <p class="lead">{t('onb.done.body')}</p>
      <dl class="sum">
        <dt>{t('settings.language')}</dt><dd>{t(`settings.language.${cfg?.language ?? 'de'}` as Key)}</dd>
        <dt>{t('accounts.title')}</dt><dd>{app.accounts.length ? app.accounts.map((a) => a.name).join(', ') : t('onb.done.noAccount')}</dd>
        <dt>{t('settings.ram')}</dt><dd class="mono">{fmtRam(ram)}</dd>
      </dl>
    {/if}

    <div class="nav">
      {#if step > 0 && step < STEPS.length - 1}<button class="btn ghost" type="button" onclick={() => (step -= 1)}><ArrowLeftIcon weight="bold" size={14} aria-hidden="true" />{t('new.back')}</button>{:else}<span></span>{/if}
      {#if step < STEPS.length - 1}
        <button class="btn accent" type="button" onclick={next}>{step === 0 ? t('onb.start') : step === 2 && !app.accounts.length ? t('onb.skip') : t('new.next')}<ArrowRightIcon weight="bold" size={14} aria-hidden="true" /></button>
      {:else}
        <button class="btn accent" type="button" onclick={finish}>{t('onb.finish')}</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .ob { position: absolute; inset: 0; z-index: 60; display: grid; place-items: center; background: var(--ink-0); overflow: auto; }
  .bg { position: absolute; inset: 0; background: var(--bg) center / cover no-repeat; opacity: 0.28; }
  .bg::after { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, var(--ink-0) 0%, transparent 70%); }
  .card { position: relative; width: min(540px, calc(100% - 48px)); display: grid; gap: 14px; padding: 28px 32px; background: var(--ink-2); border: 1px solid var(--line); border-left: 4px solid var(--mango); border-radius: var(--r-l); box-shadow: 0 24px 64px rgba(0, 0, 0, 0.6); margin: 24px; justify-self: start; margin-left: clamp(24px, 12vw, 160px); }
  h1 { font-size: 34px; line-height: 1.05; }
  h1:focus { outline: none; }
  .lead { margin: 0; color: var(--paper-dim); font-size: 15px; }
  .dots { display: flex; gap: 6px; list-style: none; margin: 0; padding: 0; }
  .dots li { width: 28px; height: 4px; background: var(--ink-4); }
  .dots li.done { background: var(--paper-mute); }
  .dots li.on { background: var(--mango); }
  .langs { display: grid; gap: 8px; }
  .langs button { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-1); font-size: 16px; }
  .langs button:hover { background: var(--ink-3); }
  .langs button.on { border-color: var(--mango); color: var(--mango); }
  form { display: grid; gap: 6px; }
  .row { display: flex; gap: 8px; }
  .row .field { flex: 1; }
  .accs { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .accs li { display: flex; align-items: center; gap: 10px; }
  .ram { display: flex; align-items: center; gap: 16px; }
  .ram output { color: var(--mango); white-space: nowrap; }
  .sum { display: grid; grid-template-columns: auto 1fr; gap: 6px 16px; margin: 0; }
  dt { color: var(--paper-mute); }
  dd { margin: 0; }
  .nav { display: flex; justify-content: space-between; align-items: center; margin-top: 6px; }
</style>
