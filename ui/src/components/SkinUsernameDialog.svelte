<script lang="ts">
  // "Use another player's skin": copies the public skin of a Minecraft name onto the account (`skin_use_username`). The name is validated
  // like the service (3..16 of A-Za-z0-9_). The current skin can be kept in the library first, so nothing is lost; after the copy the
  // dialog shows the result (front and back) before it closes.
  import Modal from './Modal.svelte';
  import Seg from './Seg.svelte';
  import SkinFlat from './SkinFlat.svelte';
  import Toggle from './Toggle.svelte';
  import { checkUsername } from '../lib/skinFlows';
  import { errorText, getApi, t } from '../lib/stores.svelte';
  import { toAppError } from '../lib/api';
  import type { SkinInfo, SkinVariantName } from '../lib/types';

  interface Props { uuid: string; current: SkinInfo | null; accountName: string; onclose: () => void; ondone: (info: SkinInfo) => void; onrelogin?: (retry: () => void) => void }
  let { uuid, current, accountName, onclose, ondone, onrelogin }: Props = $props();

  let name = $state('');
  let model = $state<'source' | SkinVariantName>('source');
  let keep = $state(true);
  let busy = $state(false);
  let error = $state('');
  let result = $state<SkinInfo | null>(null);
  const valid = $derived(checkUsername(name.trim()));
  const showInvalid = $derived(name.length > 0 && !valid);

  async function apply() {
    if (!valid || busy || result) return;
    busy = true; error = '';
    try {
      if (keep && current) await getApi().skinLibrarySaveCurrent({ uuid, name: accountName.slice(0, 40) });
      const info = await getApi().skinUseUsername(model === 'source' ? { uuid, name: name.trim() } : { uuid, name: name.trim(), variant: model });
      result = info;
      ondone(info);
    } catch (e) {
      if (toAppError(e).needsRelogin && onrelogin) { onrelogin(() => void apply()); onclose(); return; }
      error = errorText(e);
    }
    busy = false;
  }
</script>

<Modal title={t('skins.user.title')} {onclose} onenter={result ? onclose : apply}>
  <div class="body">
    {#if !result}
      <p class="hint">{t('skins.user.hint')}</p>
      <label class="lbl" for="sk-user">{t('skins.user.name')}</label>
      <input id="sk-user" class="field mono" bind:value={name} maxlength="16" autocomplete="off" spellcheck="false" aria-invalid={showInvalid} aria-describedby={showInvalid ? 'sk-user-err' : undefined} />
      {#if showInvalid}<div id="sk-user-err" class="err" role="alert">{t('accounts.nameInvalid')}</div>{/if}
      <span class="lbl">{t('skins.model')}</span>
      <Seg label={t('skins.model')} value={model} idPrefix="skumodel" options={[{ value: 'source', label: t('skins.user.sameModel') }, { value: 'classic', label: t('skins.classic') }, { value: 'slim', label: t('skins.slim') }]} onchange={(v) => (model = v as typeof model)} />
      {#if current}
        <label class="keep"><Toggle checked={keep} onchange={(on) => (keep = on)} /><span><span class="lbl">{t('skins.user.keep')}</span><br /><span class="hint">{t('skins.user.keepHint')}</span></span></label>
      {/if}
      {#if error}<div class="err" role="alert">{error}</div>{/if}
    {:else}
      <p class="ok" role="status">{t('skins.user.done', { name: name.trim() })}</p>
      <div class="res">
        <div class="fig"><SkinFlat url={result.skinUrl} slim={result.slim} side="front" scale={6} label={t('skins.preview.front')} /></div>
        <div class="fig"><SkinFlat url={result.skinUrl} slim={result.slim} side="back" scale={6} label={t('skins.preview.back')} /></div>
      </div>
    {/if}
  </div>
  {#snippet footer()}
    {#if result}
      <button class="btn accent" type="button" onclick={onclose}>{t('common.done')}</button>
    {:else}
      <button class="btn ghost" type="button" onclick={onclose}>{t('common.cancel')}</button>
      <button class="btn accent" type="button" disabled={!valid || busy} aria-busy={busy} onclick={apply}>{t('skins.user.apply')}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .body { display: grid; gap: 8px; }
  p { margin: 0; }
  .lbl { font-weight: 500; }
  .keep { display: flex; align-items: flex-start; gap: 12px; margin-top: 6px; cursor: default; }
  .res { display: flex; gap: 12px; }
  .fig { width: 84px; height: 168px; background: var(--ink-3); border-radius: var(--r-s); padding: 4px; }
  .ok { color: var(--leaf); }
</style>
