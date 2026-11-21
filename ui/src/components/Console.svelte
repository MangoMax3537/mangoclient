<script lang="ts">
  import { tick } from 'svelte';
  import TerminalIcon from 'phosphor-svelte/lib/TerminalIcon';
  import CaretDownIcon from 'phosphor-svelte/lib/CaretDownIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import { writeClipboard } from '../lib/clipboard';
  import { clearConsole, consoles, t, tp } from '../lib/stores.svelte';

  let { profileId }: { profileId: string } = $props();
  let open = $state(true);
  let box: HTMLDivElement | undefined = $state();
  const lines = $derived(consoles[profileId] ?? []);

  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  /** ALL lines (not only the visible ones) as plain text. */
  async function copyAll() {
    if (!lines.length) return;
    if (!(await writeClipboard(lines.map((l) => l.line).join('\n')))) return;
    copied = true; clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = false), 1500);
  }
  $effect(() => () => clearTimeout(copiedTimer));

  $effect(() => {
    void lines.length;
    if (open && box) {
      const el = box;
      const stick = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
      if (stick) tick().then(() => (el.scrollTop = el.scrollHeight));
    }
  });
</script>

<section class="console" aria-label={t('console.title')}>
  <header>
    <button class="toggle" type="button" aria-expanded={open} onclick={() => (open = !open)} aria-controls="console-body">
      <TerminalIcon weight="bold" size={16} aria-hidden="true" />
      <span>{t('console.title')}</span>
      <span class="count mono">{tp('console.lines', lines.length)}</span>
      <span class="caret" class:open><CaretDownIcon weight="bold" size={14} aria-hidden="true" /></span>
      <span class="sr-only">{open ? t('console.hide') : t('console.show')}</span>
    </button>
    <span class="sr-only" role="status" aria-live="polite">{copied ? t('console.copied') : ''}</span>
    {#if open}
      <button class="btn ghost small" type="button" disabled={!lines.length} onclick={copyAll}>
        {#if copied}<CheckIcon weight="bold" size={14} aria-hidden="true" />{t('console.copied')}{:else}<CopyIcon weight="bold" size={14} aria-hidden="true" />{t('console.copyAll')}{/if}
      </button>
    {/if}
    {#if open && lines.length}
      <button class="btn ghost small" type="button" onclick={() => clearConsole(profileId)}>
        <TrashIcon weight="bold" size={14} aria-hidden="true" />{t('console.clear')}
      </button>
    {/if}
  </header>
  {#if open}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div id="console-body" class="body mono" bind:this={box} role="log" aria-live="off" tabindex="0">
      {#each lines as l (l.id)}
        <div class="ln {l.level}">{l.line}</div>
      {:else}
        <div class="ln empty">{t('console.empty')}</div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .console { background: var(--ink-1); border: 1px solid var(--line-soft); overflow: hidden; }
  header { display: flex; align-items: center; justify-content: space-between; padding-right: 6px; }
  .toggle { display: flex; align-items: center; gap: 8px; padding: 10px 14px; flex: 1; text-align: left; font-weight: 600; }
  .count { color: var(--paper-mute); font-size: var(--fs-xs); font-weight: 400; }
  .caret { margin-left: auto; display: inline-flex; transition: transform var(--dur) var(--ease); transform: rotate(-90deg); }
  .caret.open { transform: none; }
  .small { height: 28px; font-size: var(--fs-sm); }
  .body { max-height: 220px; overflow: auto; padding: 8px 14px 12px; font-size: 12.5px; line-height: 1.55; border-top: 1px solid var(--line-soft); user-select: text; }
  .ln { white-space: pre-wrap; word-break: break-word; color: var(--paper-dim); }
  .ln.warn { color: var(--mango); }
  .ln.error { color: var(--tomato); }
  .ln.empty { color: var(--paper-mute); }
</style>
