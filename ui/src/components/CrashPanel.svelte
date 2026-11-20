<script lang="ts">
  // Crash analysis panel (docs/ipc.md Part 9, `crash_analyze`): summary, ranked cause cards with a checklist of things to try, related mods
  // (a link into the Content tab), the log excerpt and a report for support threads. Unknown results offer the log upload and diagnostics.
  import WarningIcon from 'phosphor-svelte/lib/WarningIcon';
  import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
  import UploadSimpleIcon from 'phosphor-svelte/lib/UploadSimpleIcon';
  import FirstAidKitIcon from 'phosphor-svelte/lib/FirstAidKitIcon';
  import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
  import CaretRightIcon from 'phosphor-svelte/lib/CaretRightIcon';
  import PuzzlePieceIcon from 'phosphor-svelte/lib/PuzzlePieceIcon';
  import Modal from './Modal.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import SkeletonRegion from './skeleton/SkeletonRegion.svelte';
  import { BusyGate } from '../lib/loadState.svelte';
  import EmptyState from './EmptyState.svelte';
  import { causeView, hasCauses, rankCauses, reportText } from '../lib/portability';
  import { hasKey, type Key } from '../lib/i18n';
  import { openDiagnostics, showModInContent } from '../lib/portability.svelte';
  import { app, errorText, getApi, lang, t, toast } from '../lib/stores.svelte';
  import type { CrashReport } from '../lib/types';

  let { profileId, logId, onclose }: { profileId: string; logId?: string; onclose: () => void } = $props();
  const profile = $derived(app.profiles.find((p) => p.id === profileId) ?? null);

  let report = $state<CrashReport | null>(null);
  let error = $state('');
  let showExcerpt = $state(false);
  let link = $state<string | null>(null);
  let uploading = $state(false);
  let checked = $state<Record<string, boolean>>({});
  let seq = 0;

  async function load() {
    const my = ++seq;
    report = null; error = ''; link = null;
    try {
      const r = await getApi().crashAnalyze(logId ? { profileId, logId } : { profileId });
      if (my === seq) report = r;
    } catch (e) { if (my === seq) error = errorText(e); }
  }
  $effect(() => { void profileId; void logId; void load(); });

  const causes = $derived(report ? rankCauses(report.causes) : []);
  const noLog = $derived(!!report && report.sourceLog === null);
  const source = $derived(report?.sourceLog ? report.sourceLog.replace(/^(crash|game|launcher):/, '') : '');
  const view = $derived(causes.map((c) => causeView(c, lang(), (k) => t(k), hasKey.bind(null, lang()))));

  async function copy() {
    if (!report || !profile) return;
    try { await navigator.clipboard.writeText(reportText(report, profile.name)); toast(t('crash.copied'), 'ok'); } catch { toast(t('err.unknown'), 'error'); }
  }
  async function upload() {
    if (!report?.sourceLog) return;
    uploading = true;
    try { link = (await getApi().logsUpload({ profileId, id: report.sourceLog })).url; } catch (e) { toast(errorText(e), 'error'); }
    uploading = false;
  }
  async function copyLink() {
    if (!link) return;
    try { await navigator.clipboard.writeText(link); toast(t('logs.linkCopied'), 'ok'); } catch { toast(t('err.unknown'), 'error'); }
  }
  // `profileId` is read from the parent's state, which is gone once the panel is closed: take it first.
  function diag() { const id = profileId; onclose(); openDiagnostics(id); }
  function showMod(m: string) { const id = profileId; onclose(); showModInContent(id, m); }
  const confKey = (c: string) => `crash.conf.${c}` as Key;
  const gate = new BusyGate();
  $effect(() => { gate.update(!report && !error); return () => gate.destroy(); });
</script>

<Modal title={t('crash.title')} {onclose} variant="drawer">
  {#if error}
    <p class="err box" role="alert">{error} <button class="btn small" type="button" onclick={load}>{t('common.retry')}</button></p>
  {:else if !report}
    {#if gate.show}
      <SkeletonRegion class="skl-crash"><Skeleton variant="bar" w="80%" h={24} radius={6} /><Skeleton variant="box" w="100%" h={96} radius={8} /><Skeleton variant="box" w="100%" h={96} radius={8} /></SkeletonRegion>
    {/if}
    <p class="hint">{t('crash.analyzing')}</p>
  {:else if noLog}
    <EmptyState title={t('crash.noLog.title')} hint={t('crash.noLog.hint')} />
  {:else}
    <div class="meta">
      {#if profile}<span class="chip">{profile.name}</span>{/if}
      {#if report.mcVersion}<span class="chip mono">{t('crash.meta.mc')} {report.mcVersion}</span>{/if}
      {#if report.javaVersion}<span class="chip mono">{t('crash.meta.java')} {report.javaVersion}</span>{/if}
      {#if source}<span class="chip mono" title={source}>{t('crash.meta.log')}: {source}</span>{/if}
    </div>

    {#if !hasCauses(report)}
      <EmptyState title={t('crash.empty.title')} hint={t('crash.empty.hint')}>
        <button class="btn" type="button" disabled={uploading} onclick={upload}><UploadSimpleIcon weight="bold" size={16} aria-hidden="true" />{uploading ? t('logs.uploading') : t('crash.upload')}</button>
        <button class="btn" type="button" onclick={diag}><FirstAidKitIcon weight="bold" size={16} aria-hidden="true" />{t('crash.exportDiag')}</button>
      </EmptyState>
      {#if link}
        <div class="res" role="status"><span>{t('logs.uploaded')}</span><input class="field mono" readonly value={link} aria-label={t('logs.link')} onfocus={(e) => e.currentTarget.select()} /><button class="btn" type="button" onclick={copyLink}><CopyIcon weight="bold" size={16} aria-hidden="true" />{t('logs.copyLink')}</button></div>
      {/if}
    {:else}
      <section aria-labelledby="cr-sum">
        <h3 id="cr-sum" class="eyebrow">{t('crash.summary')}</h3>
        <p class="sum">{view[0]?.fromLog !== null && view[0] ? t('crash.topCause', { title: view[0].title }) : report.summary}</p>
      </section>
      <section aria-labelledby="cr-causes">
        <h3 id="cr-causes" class="eyebrow">{t('crash.causes')}</h3>
        <ol class="causes">
          {#each causes as c, i (c.code + i)}
            {@const v = view[i]}
            <li class="cause {c.confidence}">
              <div class="ch">
                <span class="rank mono" aria-hidden="true">{i + 1}</span>
                <h4>{v.title}</h4>
                <span class="chip conf {c.confidence}"><span class="sq {c.confidence === 'high' ? 'bad' : c.confidence === 'medium' ? 'warn' : ''}"></span>{t(confKey(c.confidence))}</span>
              </div>
              <p class="detail">{v.detail}</p>
              {#if v.fromLog}<p class="quote mono"><span class="hint">{t('crash.fromLog')}</span> {v.fromLog}</p>{/if}
              {#if v.suggestions.length}
                <ul class="tips" aria-label={t('crash.suggestions')}>
                  {#each v.suggestions as s, k (k)}
                    <li><label><input type="checkbox" bind:checked={checked[`${c.code}:${k}`]} /><span class:done={checked[`${c.code}:${k}`]}>{s}</span></label></li>
                  {/each}
                </ul>
              {/if}
              {#if c.relatedMods.length}
                <div class="mods" aria-label={t('crash.related')}>
                  <span class="hint">{t('crash.related')}</span>
                  {#each c.relatedMods as m (m)}
                    <button class="chip modbtn" type="button" onclick={() => showMod(m)} title={t('crash.showMod', { name: m })} aria-label={t('crash.showMod', { name: m })}><PuzzlePieceIcon weight="bold" size={14} aria-hidden="true" />{m}</button>
                  {/each}
                </div>
              {/if}
            </li>
          {/each}
        </ol>
      </section>
    {/if}

    {#if report.excerpt.trim()}
      <section>
        <button class="tog" type="button" aria-expanded={showExcerpt} aria-controls="cr-ex" onclick={() => (showExcerpt = !showExcerpt)}>
          {#if showExcerpt}<CaretDownIcon weight="bold" size={14} aria-hidden="true" />{:else}<CaretRightIcon weight="bold" size={14} aria-hidden="true" />{/if}
          {showExcerpt ? t('crash.excerptHide') : t('crash.excerptShow')}
        </button>
        {#if showExcerpt}<!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrollable region must be reachable by keyboard) -->
        <pre id="cr-ex" class="ex mono" tabindex="0" aria-label={t('crash.excerpt')}>{report.excerpt}</pre>{/if}
      </section>
    {/if}
  {/if}

  {#snippet footer()}
    {#if report && !noLog}
      {#if hasCauses(report)}<button class="btn grow" type="button" onclick={diag}><FirstAidKitIcon weight="bold" size={16} aria-hidden="true" />{t('crash.exportDiag')}</button>{/if}
      <button class="btn accent" type="button" onclick={copy}><CopyIcon weight="bold" size={16} aria-hidden="true" />{t('crash.copy')}</button>
    {:else}
      <button class="btn" type="button" onclick={onclose}>{t('common.close')}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  section { margin-top: 18px; }
  section:first-of-type { margin-top: 14px; }
  .meta { display: flex; flex-wrap: wrap; gap: 6px; }
  .meta .chip { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: inline-block; line-height: 22px; }
  .sum { margin: 6px 0 0; font-size: var(--fs-lg); line-height: 1.4; }
  .causes { list-style: none; margin: 8px 0 0; padding: 0; display: grid; gap: 10px; }
  .cause { padding: 12px 14px; background: var(--ink-1); border: 1px solid var(--line); display: grid; gap: 8px; }
  .cause.high { border-color: var(--tomato); }
  .cause.medium { border-color: var(--mango); }
  .ch { display: flex; align-items: center; gap: 10px; }
  .rank { flex: none; width: 24px; height: 24px; display: grid; place-items: center; background: var(--ink-4); color: var(--paper); font-weight: 700; font-size: var(--fs-sm); }
  h4 { flex: 1; font: 700 var(--fs-lg) var(--font-display); min-width: 0; }
  .conf { flex: none; }
  .detail { margin: 0; color: var(--paper-dim); }
  .quote { margin: 0; padding: 6px 10px; background: var(--ink-2); font-size: var(--fs-sm); color: var(--paper-dim); word-break: break-word; }
  .tips { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  .tips label { display: flex; align-items: flex-start; gap: 10px; padding: 4px 0; }
  .tips input { margin-top: 3px; width: 16px; height: 16px; flex: none; }
  .tips .done { color: var(--paper-mute); text-decoration: line-through; }
  .mods { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .modbtn { color: var(--paper); transition: border-color var(--dur) var(--ease), background var(--dur) var(--ease); }
  .modbtn:hover { border-color: var(--accent-fill); background: var(--ink-3); }
  .tog { display: inline-flex; align-items: center; gap: 6px; font-weight: 600; color: var(--paper-dim); height: 32px; }
  .tog:hover { color: var(--paper); }
  .ex { margin: 6px 0 0; padding: 10px 12px; max-height: 260px; overflow: auto; background: var(--ink-1); border: 1px solid var(--line); font-size: 12.5px; white-space: pre-wrap; word-break: break-word; user-select: text; }
  .box { padding: 8px 12px; border: 1px solid color-mix(in srgb, var(--tomato) 45%, transparent); background: color-mix(in srgb, var(--tomato) 10%, transparent); color: var(--paper); }
  :global(.skl-crash) { display: grid; gap: 12px; }
  .res { display: flex; align-items: center; gap: 8px; margin-top: 12px; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--leaf) 55%, var(--line)); }
  .res .field { flex: 1; height: 32px; }
  .grow { margin-right: auto; }
</style>
