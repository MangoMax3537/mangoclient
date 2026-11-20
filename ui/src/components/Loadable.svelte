<script lang="ts" generics="T">
  // Renders a LoadState: nothing for the first ~150 ms, then `skeleton` (at least ~250 ms), then the content / `empty` / a LoadError with
  // Retry. `aria-busy` sits on the region while loading.
  import type { Snippet } from 'svelte';
  import type { LoadState } from '../lib/loadState.svelte';
  import LoadError from './LoadError.svelte';

  interface Props {
    state: LoadState<T>;
    skeleton: Snippet;
    children: Snippet<[T]>;
    empty?: Snippet;
    class?: string;
    compactError?: boolean;
  }
  let { state: s, skeleton, children, empty, class: cls = '', compactError = false }: Props = $props();
</script>

<div class="ld {cls}" aria-busy={s.phase === 'loading'}>
  {#if s.phase === 'loading'}
    {#if s.showSkeleton}{@render skeleton()}{/if}
  {:else if s.phase === 'error'}
    <LoadError error={s.error} onretry={() => s.retry()} compact={compactError} />
  {:else if s.phase === 'empty'}
    {#if empty}{@render empty()}{:else if s.data !== null}{@render children(s.data)}{/if}
  {:else if s.phase === 'ready' && s.data !== null}
    {@render children(s.data)}
  {/if}
</div>

<style>
  .ld { display: contents; }
  .ld.fill { display: block; height: 100%; }
</style>
