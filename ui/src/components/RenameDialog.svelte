<script lang="ts">
  // Small rename dialog: the current name is selected on open, Enter saves, Esc cancels, the problem (empty, too long, control characters) shows
  // inline before anything is sent. Saved through profile_update { name }.
  import { onMount } from 'svelte';
  import Modal from './Modal.svelte';
  import { checkName, problemKey } from '../lib/rename';
  import { actions, app, errorText, t, toast, ui } from '../lib/stores.svelte';

  let { id }: { id: string } = $props();
  const p = $derived(app.profiles.find((x) => x.id === id) ?? null);
  let value = $state(app.profiles.find((x) => x.id === id)?.name ?? '');
  let input: HTMLInputElement | undefined = $state();
  let busy = $state(false);
  let serverErr = $state('');
  const check = $derived(checkName(value));
  let touched = $state(false);
  const err = $derived(serverErr || (touched && !check.ok ? t(problemKey(check.problem)) : ''));
  const close = () => { ui.rename = null; };

  onMount(() => { queueMicrotask(() => { input?.focus(); input?.select(); }); });
  async function save() {
    touched = true; serverErr = '';
    if (!check.ok || !p || busy) return;
    if (check.name === p.name) { close(); return; }
    busy = true;
    try { await actions.updateProfile(p.id, { name: check.name }); toast(t('rename.done', { name: check.name }), 'ok', 2500); close(); }
    catch (e) { serverErr = errorText(e); busy = false; }
  }
</script>

<Modal title={t('rename.title')} onclose={close} onenter={() => void save()}>
  <label class="f">
    <span class="eyebrow">{t('rename.label')}</span>
    <input bind:this={input} class="field" type="text" bind:value maxlength="256" autocomplete="off" spellcheck="false" aria-invalid={err ? 'true' : undefined} aria-describedby={err ? 'rn-err' : undefined}
      oninput={() => { touched = true; serverErr = ''; }} />
  </label>
  {#if err}<div id="rn-err" class="err" role="alert">{err}</div>{/if}
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={close}>{t('common.cancel')}</button>
    <button class="btn accent" type="button" disabled={busy || (touched && !check.ok)} onclick={() => void save()}>{t('rename.save')}</button>
  {/snippet}
</Modal>

<style>
  .f { display: grid; gap: 6px; }
  .err { margin-top: 6px; }
</style>
