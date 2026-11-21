<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import XIcon from 'phosphor-svelte/lib/XIcon';
  import { leaveDialog } from '../lib/motion';
  import { t } from '../lib/stores.svelte';

  interface Props {
    title: string;
    onclose: () => void;
    variant?: 'dialog' | 'drawer' | 'lightbox';
    /** Width of the plain dialog: md 480px, lg 640px. */
    size?: 'md' | 'lg';
    /** Enter confirms (unless a button, link or multi-line field has focus). Omit for dialogs without a primary action. */
    onenter?: () => void;
    /** Blocks Escape, backdrop click and the close button (takeover states). */
    locked?: boolean;
    children: Snippet;
    footer?: Snippet;
    actions?: Snippet;
  }
  let { title, onclose, variant = 'dialog', size = 'md', onenter, locked = false, children, footer, actions }: Props = $props();
  let dlg: HTMLDialogElement | undefined = $state();
  const uid = `m${Math.random().toString(36).slice(2, 8)}`;
  let opener: Element | null = null;

  onMount(() => {
    opener = document.activeElement;
    dlg?.showModal();
    return () => {
      if (dlg?.open) dlg.close();
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
    };
  });
  function cancel(e: Event) { e.preventDefault(); if (!locked) onclose(); }
  function key(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !onenter || e.defaultPrevented || e.isComposing) return;
    const el = e.target as HTMLElement | null;
    if (el?.closest('button, a, textarea, select, summary, [role="radio"], [role="option"]')) return;
    e.preventDefault(); onenter();
  }
  function backdrop(e: MouseEvent) { if (!locked && e.target === dlg) onclose(); }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog bind:this={dlg} class="nb {variant} {size}" aria-labelledby={uid} oncancel={cancel} onclick={backdrop} onkeydown={key} out:leaveDialog|global={{ drawer: variant === 'drawer' }}>
  <header>
    <h2 id={uid}>{title}</h2>
    <div class="acts">
      {#if actions}{@render actions()}{/if}
      {#if !locked}
        <button class="btn ghost x" type="button" aria-label={t('common.close')} onclick={onclose}><XIcon weight="bold" size={16} aria-hidden="true" /></button>
      {/if}
    </div>
  </header>
  <div class="body">{@render children()}</div>
  {#if footer}<footer>{@render footer()}</footer>{/if}
</dialog>

<style>
  /* a notched box with a hard 1px border (.nb): fill and border are background layers of the dialog (app.css) */
  dialog { position: fixed; --n: 16px; --fill: var(--ink-2); padding: 1px; color: var(--paper); max-height: min(86vh, 760px); overflow: hidden; }
  dialog[open] { display: flex; flex-direction: column; animation: pop var(--dur-slow) var(--ease); }
  dialog::backdrop { background: var(--backdrop); transition: opacity var(--dur) var(--ease); animation: fade var(--dur-slow) var(--ease); }
  dialog:global(.closing)::backdrop { opacity: 0; }
  dialog.dialog { width: min(480px, calc(100vw - 48px)); }
  dialog.dialog.lg { width: min(640px, calc(100vw - 48px)); }
  dialog.drawer { --n: 0px; width: min(560px, 100vw); height: 100vh; max-height: none; margin: 0 0 0 auto; animation: slide var(--dur-slow) var(--ease); }
  dialog.lightbox { --fill: var(--ink-1); width: min(1100px, calc(100vw - 40px)); max-height: calc(100vh - 40px); }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 14px 16px 12px 20px; border-bottom: 1px solid var(--line-soft); flex: none; }
  h2 { font-size: var(--fs-xl); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .acts { display: flex; align-items: center; gap: 4px; }
  .x { width: 34px; padding: 0; justify-content: center; }
  /* flex-basis auto (the content's height), shrinking to the dialog's max-height: a 0% basis collapsed the body to a sliver in
     WebKitGTK, which resolves it against the dialog's indefinite height (icon picker showed only header and footer) */
  .body { padding: 16px 20px; overflow: auto; min-height: 0; flex: 1 1 auto; user-select: text; }
  footer { display: flex; flex-wrap: wrap; align-items: center; justify-content: flex-end; gap: 8px; padding: 12px 20px; border-top: 1px solid var(--line-soft); flex: none; }
  @keyframes fade { from { opacity: 0; } }
  @keyframes pop { from { opacity: 0; transform: translateY(8px); } }
  @keyframes slide { from { transform: translateX(32px); opacity: 0; } }
</style>
