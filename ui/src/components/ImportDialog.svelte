<script lang="ts">
  // Import plan dialog (docs/ipc.md Part 9): the plan of a .mrpack / CurseForge / Prism file, an editable name, blocked files and the import
  // itself with progress. All state lives in portability.svelte.ts, so an import keeps running when this dialog is hidden.
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import PackageIcon from 'phosphor-svelte/lib/PackageIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import BlockedPlanNote from './BlockedPlanNote.svelte';
  import { fmtBytes } from '../lib/format';
  import {
    baseName, importErrorKey, importRetryable, KIND_LABEL, nameValid, NAME_MAX,
  } from '../lib/portability';
  import { backToPlan, closeImport, confirmImport, pf, pickAndImport, renameImport } from '../lib/portability.svelte';
  import type { Key } from '../lib/i18n';
  import { errorText, installs, lang, t } from '../lib/stores.svelte';

  const st = $derived(pf.imp);
  const plan = $derived(st && (st.s === 'plan' || st.s === 'importing') ? st.plan : st?.s === 'error' ? st.plan ?? null : null);
  const file = $derived(st && st.s !== 'done' ? baseName(st.path) : '');
  const name = $derived(st && (st.s === 'plan' || st.s === 'importing') ? st.name : st?.s === 'error' ? st.name ?? '' : '');
  const importing = $derived(st?.s === 'importing');
  const ok = $derived(nameValid(name));
  const pct = $derived.by(() => { const e = installs['import']; return e && e.total ? Math.round((e.done / e.total) * 100) : 0; });
  const errCode = $derived(st?.s === 'error' ? st.code : '');
  const errText = $derived.by(() => { const k = importErrorKey(errCode); return k ? t(k) : errorText({ code: errCode }); });
</script>

{#if st && pf.impOpen}
  <Modal title={t('import.title')} onclose={closeImport} size="lg" onenter={st.s === 'plan' ? confirmImport : undefined}>
    <div class="top">
      <Tile kind="mod" id={st.s === 'done' ? '' : st.path} src={null} size={64} radius={10} />
      <div class="hd">
        {#if plan}<span class="chip kind {plan.kind}"><PackageIcon weight="bold" size={14} aria-hidden="true" />{t(KIND_LABEL[plan.kind])}</span>{:else}<span class="eyebrow">{t('import.source')}</span>{/if}
        <h3 class="mono" title={st.s === 'done' ? '' : st.path}>{file}</h3>
        {#if plan}<p class="hint">{t(`import.kindNote.${plan.kind}` as Key)}</p>{/if}
      </div>
    </div>

    {#if st.s === 'planning'}
      <div class="skl" aria-busy="true" aria-label={t('import.planning')} role="status"><span class="skel ln"></span><span class="skel ln"></span><span class="skel ln"></span><span class="sr-only">{t('import.planning')}</span></div>
    {:else if st.s === 'error' && !plan}
      <p class="err box" role="alert">{errText}</p>
    {:else if plan}
      <label class="lbl" for="imp-name">{t('import.name')}</label>
      <input id="imp-name" class="field" value={name} maxlength={NAME_MAX} autocomplete="off" aria-invalid={!ok} disabled={st.s !== 'plan'}
        oninput={(e) => renameImport(e.currentTarget.value)} />
      {#if !ok}<div class="err" role="alert">{t('new.nameRequired')}</div>{/if}

      <dl class="facts">
        <div><dt>{t('col.version')}</dt><dd class="mono">{plan.mcVersion}</dd></div>
        <div><dt>{t('col.loader')}</dt><dd><span class="chip mono">{plan.loader === 'vanilla' ? t('profile.vanilla') : `${plan.loader} ${plan.loaderVersion ?? ''}`.trim()}</span></dd></div>
        <div><dt>{t('flow.pack.files')}</dt><dd class="mono">{plan.fileCount}</dd></div>
        <div><dt>{t('flow.pack.size')}</dt><dd class="mono">{plan.sizeBytes !== null ? fmtBytes(plan.sizeBytes, lang()) : t('import.sizeUnknown')}</dd></div>
      </dl>

      <BlockedPlanNote files={plan.blockedFiles} id="imp-blocked" />

      {#if importing}
        <div class="prog" role="status">
          <span class="bar" role="progressbar" aria-label={t('import.progress', { name: name.trim() })} aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct}><span style:transform="scaleX({pct / 100})"></span></span>
          <span class="hint mono">{pct}%</span>
        </div>
        <p class="hint">{t('flow.run.background')}</p>
      {:else if st.s === 'error'}
        <p class="err box" role="alert">{errText}</p>
      {:else}
        <p class="hint creates">{t('import.creates')}</p>
      {/if}
    {/if}

    {#snippet footer()}
      {#if st.s === 'error' && importRetryable(errCode)}
        <button class="btn accent" type="button" onclick={() => { closeImport(); void pickAndImport(); }}>{t('import.chooseAgain')}</button>
      {:else if st.s === 'error' && st.during === 'import'}
        <button class="btn" type="button" onclick={backToPlan}>{t('import.back')}</button>
      {/if}
      <button class="btn ghost" type="button" onclick={closeImport}>{importing ? t('import.hide') : st.s === 'error' ? t('common.close') : t('import.cancel')}</button>
      {#if st.s === 'plan' || importing}
        <button class="btn accent" type="button" disabled={st.s !== 'plan' || !ok} onclick={confirmImport}>
          {#if importing}<span class="spin"><CircleNotchIcon weight="bold" size={14} aria-hidden="true" /></span>{t('import.importing')}{:else}{t('import.confirm')}{/if}
        </button>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .top { display: flex; gap: 14px; align-items: center; margin-bottom: 14px; }
  .hd { display: grid; gap: 4px; min-width: 0; justify-items: start; }
  h3 { font-size: var(--fs-lg); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hd p { margin: 0; }
  .kind { font-weight: 600; color: var(--paper); border-color: var(--accent-fill); }
  .kind.curseforge { border-color: var(--tomato); }
  .kind.prism { border-color: var(--sky); }
  .lbl { display: block; font-size: var(--fs-sm); font-weight: 600; color: var(--paper-dim); margin-bottom: 4px; }
  .field { width: 100%; }
  .facts { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin: 14px 0 0; border: 1px solid var(--line); border-radius: var(--r-m); }
  .facts div { display: flex; justify-content: space-between; align-items: center; gap: 12px; padding: 8px 12px; border-bottom: 1px solid var(--line-soft); min-height: 40px; }
  .facts div:nth-child(odd) { border-right: 1px solid var(--line-soft); }
  .facts div:nth-last-child(-n + 2) { border-bottom: 0; }
  dt { color: var(--paper-dim); }
  dd { margin: 0; }
  .creates { margin: 12px 0 0; }
  .box { margin: 14px 0 0; padding: 8px 12px; border-left: 3px solid var(--tomato); background: color-mix(in srgb, var(--tomato) 10%, transparent); color: var(--paper); }
  .skl { display: grid; gap: 10px; align-content: start; min-height: 220px; }
  .ln { height: 14px; width: 60%; }
  .prog { display: flex; align-items: center; gap: 12px; margin-top: 14px; }
  .bar { flex: 1; height: 6px; background: var(--ink-4); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent-fill); transform-origin: left; transition: transform var(--dur) var(--ease); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
