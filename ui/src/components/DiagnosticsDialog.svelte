<script lang="ts">
  // "Export diagnostics": a zip for bug reports. Says what is in it and that paths, user names and tokens are scrubbed.
  import ShieldCheckIcon from 'phosphor-svelte/lib/ShieldCheckIcon';
  import Modal from './Modal.svelte';
  import BusyBar from './BusyBar.svelte';
  import SaveTarget from './SaveTarget.svelte';
  import FileResult from './FileResult.svelte';
  import { baseName, defaultDiagnosticsName, exportBlocker, exportErrorKey, withExtension } from '../lib/portability';
  import { toAppError } from '../lib/api';
  import { fmtBytes } from '../lib/format';
  import { app, errorText, getApi, lang, t } from '../lib/stores.svelte';
  import type { FileExport } from '../lib/types';

  let { profileId, onclose }: { profileId: string; onclose: () => void } = $props();
  const profile = $derived(app.profiles.find((p) => p.id === profileId) ?? null);
  let outPath = $state('');
  let running = $state(false);
  let result = $state<FileExport | null>(null);
  let error = $state('');

  const blocker = $derived(exportBlocker({ includeConfig: false, outPath }));
  const why = $derived(blocker === 'noPath' ? 'export.blocked.noPath' : blocker === 'badPath' ? 'export.blocked.badPath' : null);
  const defName = $derived(defaultDiagnosticsName(profile?.name ?? 'instance'));

  async function run() {
    if (blocker || running) return;
    running = true; error = '';
    try { result = await getApi().diagnosticsExport({ profileId, outPath: withExtension(outPath.trim(), '.zip') }); }
    catch (e) { const k = exportErrorKey(toAppError(e).code); error = k ? t(k) : errorText(e); }
    running = false;
  }
</script>

{#if profile}
  <Modal title={t('diag.title', { name: profile.name })} {onclose} size="lg" locked={running} onenter={result ? undefined : run}>
    <p class="lead">{t('diag.lead')}</p>
    {#if result}
      <FileResult {result} title={t('diag.doneTitle')} line={t('diag.done', { name: baseName(result.path), size: fmtBytes(result.sizeBytes, lang()) })} />
    {:else}
      <section class="in" aria-labelledby="dg-in">
        <h3 id="dg-in" class="eyebrow">{t('diag.contains')}</h3>
        <ul><li>{t('diag.c1')}</li><li>{t('diag.c2')}</li><li>{t('diag.c3')}</li><li>{t('diag.c4')}</li></ul>
      </section>
      <p class="scrub"><ShieldCheckIcon weight="bold" size={18} aria-hidden="true" /><span>{t('diag.scrubbed')}</span></p>
      <SaveTarget bind:path={outPath} defaultName={defName} title={t('diag.button')} label={t('export.saveTo')} placeholder={t('export.pathPlaceholder', { file: defName })}
        disabled={running} id="dg-path" invalidKey={outPath.trim() ? why : null} />
      {#if running}<div class="busy"><BusyBar label={t('diag.running')} /></div>{/if}
      {#if error}<p class="err box" role="alert">{error}</p>{/if}
    {/if}
    {#snippet footer()}
      {#if result}
        <button class="btn accent" type="button" onclick={onclose}>{t('common.done')}</button>
      {:else}
        <span id="dg-footer-why" class="why hint grow" role="status">{#if running}{t('diag.running')}{:else if why}{t(why)}{/if}</span>
        <button class="btn ghost" type="button" disabled={running} onclick={onclose}>{t('common.cancel')}</button>
        <button class="btn accent" type="button" disabled={!!blocker || running} aria-describedby="dg-footer-why" onclick={run}>{t('diag.run')}</button>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .grow { margin-right: auto; min-width: 0; }
  .lead { margin: 0 0 12px; color: var(--paper-dim); }
  .in { padding: 10px 12px; background: var(--ink-1); border: 1px solid var(--line-soft); }
  .in h3 { margin-bottom: 6px; }
  .in ul { margin: 0; padding-left: 18px; display: grid; gap: 4px; font-size: var(--fs-sm); color: var(--paper-dim); }
  .scrub { display: flex; gap: 10px; align-items: flex-start; margin: 12px 0 14px; padding: 10px 12px; border: 1px solid color-mix(in srgb, var(--leaf) 45%, transparent); background: color-mix(in srgb, var(--leaf) 9%, transparent); font-size: var(--fs-sm); }
  .scrub > :global(svg) { flex: none; color: var(--leaf); margin-top: 1px; }
  .busy { margin-top: 12px; }
  .box { margin: 12px 0 0; padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--tomato) 45%, transparent); background: color-mix(in srgb, var(--tomato) 10%, transparent); color: var(--paper); }
</style>
