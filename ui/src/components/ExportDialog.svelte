<script lang="ts">
  // "Export as .mrpack" (docs/ipc.md Part 9): options, what the file will and will not contain, the target and the result with warnings.
  import PackageIcon from 'phosphor-svelte/lib/PackageIcon';
  import Modal from './Modal.svelte';
  import BusyBar from './BusyBar.svelte';
  import SaveTarget from './SaveTarget.svelte';
  import FileResult from './FileResult.svelte';
  import InstanceIcon from './InstanceIcon.svelte';
  import { defaultExportName, exportBlocker, exportErrorKey, withExtension } from '../lib/portability';
  import { toAppError } from '../lib/api';
  import { app, errorText, getApi, lang, t } from '../lib/stores.svelte';
  import { fmtBytes } from '../lib/format';
  import type { FileExport } from '../lib/types';

  let { profileId, onclose }: { profileId: string; onclose: () => void } = $props();
  const profile = $derived(app.profiles.find((p) => p.id === profileId) ?? null);
  let includeConfig = $state(false);
  let outPath = $state('');
  let running = $state(false);
  let result = $state<FileExport | null>(null);
  let error = $state('');

  const blocker = $derived(exportBlocker({ includeConfig, outPath }));
  const why = $derived(blocker === 'noPath' ? 'export.blocked.noPath' : blocker === 'badPath' ? 'export.blocked.badPath' : null);
  const defName = $derived(defaultExportName(profile?.name ?? 'instance'));

  async function run() {
    if (blocker || running || !profile) return;
    running = true; error = '';
    try {
      result = await getApi().instanceExport({ profileId, format: 'mrpack', includeConfig, outPath: withExtension(outPath.trim(), '.mrpack') });
    } catch (e) {
      const k = exportErrorKey(toAppError(e).code);
      error = k ? t(k) : errorText(e);
    }
    running = false;
  }
</script>

{#if profile}
  <Modal title={t('export.title', { name: profile.name })} {onclose} size="lg" locked={running} onenter={result ? undefined : run}>
    <div class="top">
      <InstanceIcon {profile} size={56} notch={10} />
      <p class="lead">{t('export.lead')}</p>
    </div>

    {#if result}
      <FileResult {result} title={t('export.doneTitle')} line={t('export.done', { name: profile.name, size: fmtBytes(result.sizeBytes, lang()) })} />
    {:else}
      <label class="opt">
        <input type="checkbox" bind:checked={includeConfig} disabled={running} />
        <span><b>{t('export.includeConfig')}</b><span class="hint">{t('export.includeConfigHint')}</span></span>
      </label>

      <section class="rules" aria-labelledby="ex-rules">
        <h3 id="ex-rules" class="eyebrow"><PackageIcon weight="bold" size={14} aria-hidden="true" />{t('export.rules')}</h3>
        <ul>
          <li>{t('export.rule.hosted')}</li>
          <li>{t('export.rule.local')}</li>
          <li>{t('export.rule.curseforge')}</li>
        </ul>
      </section>

      <SaveTarget bind:path={outPath} defaultName={defName} title={t('export.menu')} label={t('export.saveTo')} placeholder={t('export.pathPlaceholder', { file: defName })}
        disabled={running} id="ex-path" invalidKey={outPath.trim() ? why : null} />

      {#if running}<div class="busy"><BusyBar label={t('export.running', { name: profile.name })} /></div>{/if}
      {#if error}<p class="err box" role="alert">{error}</p>{/if}
    {/if}

    {#snippet footer()}
      {#if result}
        <button class="btn accent" type="button" onclick={onclose}>{t('common.done')}</button>
      {:else}
        <span id="ex-footer-why" class="why hint grow" role="status">{#if running}{t('export.running', { name: profile.name })}{:else if why}{t(why)}{/if}</span>
        <button class="btn ghost" type="button" disabled={running} onclick={onclose}>{t('common.cancel')}</button>
        <button class="btn accent" type="button" disabled={!!blocker || running} aria-describedby="ex-footer-why" onclick={run}>{t('export.run')}</button>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .grow { margin-right: auto; min-width: 0; }
  .top { display: flex; align-items: center; gap: 14px; margin-bottom: 14px; }
  .lead { margin: 0; color: var(--paper-dim); }
  .opt { display: flex; gap: 12px; align-items: flex-start; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--r-m); cursor: pointer; }
  .opt input { margin-top: 3px; width: 16px; height: 16px; flex: none; }
  .opt span { display: grid; gap: 2px; }
  .rules { margin: 14px 0; padding: 10px 12px; background: var(--ink-1); border: 1px solid var(--line-soft); border-radius: var(--r-m); }
  .rules h3 { display: flex; align-items: center; gap: 6px; margin-bottom: 6px; }
  .rules ul { margin: 0; padding-left: 18px; display: grid; gap: 4px; font-size: var(--fs-sm); color: var(--paper-dim); }
  .busy { margin-top: 12px; }
  .box { margin: 12px 0 0; padding: 8px 12px; border-left: 3px solid var(--tomato); background: color-mix(in srgb, var(--tomato) 10%, transparent); color: var(--paper); }
</style>
