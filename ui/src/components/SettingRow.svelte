<script lang="ts">
  // One settings row: text on the left, the control on the right (or underneath with `stack`). The row carries its search id, so the
  // settings search can scroll to it and flash it. Rows are separated by ONE hairline, drawn by `.set-rows > .set-row + .set-row` in app.css.
  import type { Snippet } from 'svelte';
  import InfoLine from './InfoLine.svelte';
  import type { Fact } from '../lib/infoLine';
  interface Props {
    id: string;
    label: string;
    hint?: string;
    /** Facts shown as the hint line with icons (InfoLine) instead of a joined string. */
    hintFacts?: readonly Fact[];
    /** Control below the text instead of beside it. */
    stack?: boolean;
    /** Make the whole row a <label>, so a click on its text operates the control (toggles). */
    asLabel?: boolean;
    /** `for` of the label (when the row is not a wrapping label). */
    forId?: string;
    children?: Snippet;
    extra?: Snippet;
  }
  let { id, label, hint = '', hintFacts, stack = false, asLabel = false, forId, children, extra }: Props = $props();
</script>

{#snippet text()}
  <span class="txt">
    {#if forId && !asLabel}<label for={forId}>{label}</label>{:else}<span class="lbl">{label}</span>{/if}
    {#if hintFacts?.length}<span class="hint mono"><InfoLine facts={hintFacts} wrap /></span>{:else if hint}<span class="hint">{hint}</span>{/if}
  </span>
{/snippet}

{#if asLabel}
  <label class="set-row" class:stack id="set-{id}">
    {@render text()}
    {#if children}{@render children()}{/if}
    {#if extra}{@render extra()}{/if}
  </label>
{:else}
  <div class="set-row" class:stack id="set-{id}">
    {@render text()}
    {#if children}{@render children()}{/if}
    {#if extra}{@render extra()}{/if}
  </div>
{/if}
