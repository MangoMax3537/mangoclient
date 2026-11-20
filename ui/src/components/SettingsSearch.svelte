<script lang="ts">
  // Search field of the settings pages. Typing lists the matching settings under the field (section breadcrumb, the alias that matched),
  // filters the page to the matching rows, and Enter / click jumps to one: the page scrolls there, flashes it and focuses the control.
  import MagnifyingGlassIcon from 'phosphor-svelte/lib/MagnifyingGlassIcon';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import { entriesFor, sectionOf, type SettingEntry, type SettingsScope } from '../lib/settingsIndex';
  import { keyStep, searchSettings } from '../lib/settingsSearch';
  import { t } from '../lib/stores.svelte';

  interface Props {
    scope: SettingsScope;
    /** Ids of the matching entries while a query is typed; `null` = no filter (the page shows everything). */
    matches?: Set<string> | null;
    onpick: (entry: SettingEntry) => void;
  }
  let { scope, matches = $bindable(null), onpick }: Props = $props();

  let query = $state('');
  let active = $state(0);
  const uid = $derived(`ss-${scope}`);
  const entries = $derived(entriesFor(scope));
  const results = $derived(searchSettings(entries, query, {
    label: (e) => t(e.label), desc: (e) => (e.desc ? t(e.desc) : ''), section: (e) => t(sectionOf(e).label),
  }));
  const typing = $derived(query.trim().length > 0);
  $effect(() => { matches = typing ? new Set(results.map((r) => r.entry.id)) : null; });
  $effect(() => { void query; active = 0; });

  function clear() { query = ''; }
  function pick(i: number) {
    const hit = results[i];
    if (!hit) return;
    clear();
    onpick(hit.entry);
  }
  function key(e: KeyboardEvent) {
    const step = keyStep(e.key, active, results.length);
    if (e.key === 'Escape' && !typing) return; // let a dialog or menu handle Escape
    if (step.action === 'clear') { e.preventDefault(); e.stopPropagation(); clear(); return; }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Home' || e.key === 'End' || e.key === 'Enter') {
      if (!results.length) return;
      if ((e.key === 'Home' || e.key === 'End') && query.length > 0 && !e.ctrlKey) return; // Home/End keep moving the caret in the text
      e.preventDefault();
      active = step.active;
      if (step.action === 'pick') pick(step.active);
    }
  }
</script>

<div class="ss" role="search">
  <label class="box">
    <MagnifyingGlassIcon weight="bold" size={16} aria-hidden="true" />
    <span class="sr-only">{t('settings.search.label')}</span>
    <input class="field" type="text" data-search role="combobox" autocomplete="off" spellcheck="false" placeholder={t('settings.search.placeholder')}
      aria-expanded={typing && results.length > 0} aria-controls="{uid}-list" aria-activedescendant={typing && results.length ? `${uid}-${active}` : undefined}
      bind:value={query} onkeydown={key} />
    {#if typing}<button class="btn ghost ic" type="button" aria-label={t('settings.search.clear')} onclick={clear}><XIcon weight="bold" size={14} aria-hidden="true" /></button>{/if}
  </label>

  {#if typing}
    {#if results.length}
      <ul class="res" id="{uid}-list" role="listbox" aria-label={t('settings.search.label')}>
        {#each results as r, i (r.entry.id)}
          <li id="{uid}-{i}" role="option" aria-selected={i === active} class:on={i === active}>
            <button type="button" tabindex="-1" onclick={() => pick(i)} onmousemove={() => (active = i)}>
              <span class="l">{t(r.entry.label)}</span>
              <span class="c">{t(sectionOf(r.entry).label)}</span>
              {#if r.alias}<span class="m mono">{t('settings.search.matched', { alias: r.alias })}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
      <div class="sr-only" role="status" aria-live="polite">{results.length}</div>
    {:else}
      <p class="none" role="status">{t('settings.search.none', { q: query.trim() })}</p>
    {/if}
  {/if}
</div>

<style>
  .ss { display: grid; gap: 8px; }
  .box { position: relative; display: flex; align-items: center; }
  .box > :global(svg) { position: absolute; left: 12px; color: var(--paper-mute); pointer-events: none; }
  .field { width: 100%; padding-left: 36px; padding-right: 40px; }
  .ic { position: absolute; right: 4px; width: 30px; height: 30px; padding: 0; }
  .res { list-style: none; margin: 0; padding: 4px; display: grid; gap: 2px; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-2); }
  .res li.on button { background: var(--ink-4); }
  .res button { display: grid; grid-template-columns: minmax(0, 1fr) auto; grid-template-areas: 'l c' 'm m'; gap: 0 12px; width: 100%; text-align: left; padding: 7px 10px; border-radius: var(--r-s); }
  .res button:hover { background: var(--ink-3); }
  .l { grid-area: l; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .c { grid-area: c; font-size: var(--fs-xs); letter-spacing: 0.08em; text-transform: uppercase; color: var(--paper-dim); align-self: center; }
  .m { grid-area: m; font-size: var(--fs-xs); color: var(--paper-mute); }
  .none { margin: 0; color: var(--paper-dim); font-size: var(--fs-sm); padding: 4px 2px; overflow-wrap: anywhere; }
</style>
