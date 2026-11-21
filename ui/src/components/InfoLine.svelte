<script lang="ts">
  // THE info line: facts side by side, each with one small icon in front (no "/" separators). Used by the hero lines, instance and server cards.
  // The icon carries the meaning of the fact, so it has a title and a screen-reader label (`info.*`).
  import TagIcon from 'phosphor-svelte/lib/TagIcon';
  import PuzzlePieceIcon from 'phosphor-svelte/lib/PuzzlePieceIcon';
  import ClockIcon from 'phosphor-svelte/lib/ClockIcon';
  import UsersIcon from 'phosphor-svelte/lib/UsersIcon';
  import CellSignalFullIcon from 'phosphor-svelte/lib/CellSignalFullIcon';
  import type { Fact, FactKind } from '../lib/infoLine';
  import type { Key } from '../lib/i18n';
  import { t } from '../lib/stores.svelte';

  interface Props {
    facts: readonly Fact[];
    /** Icon size in px. */
    size?: number;
    /** Wrap onto more lines instead of cutting with an ellipsis. */
    wrap?: boolean;
  }
  let { facts, size = 13, wrap = false }: Props = $props();

  const ICONS = { version: TagIcon, loader: PuzzlePieceIcon, played: ClockIcon, players: UsersIcon, ping: CellSignalFullIcon } as const;
  const LABEL: Record<FactKind, Key> = { version: 'info.version', loader: 'info.loader', played: 'info.played', players: 'info.players', ping: 'info.ping' };
</script>

<span class="il" class:wrap data-info-line>
  {#each facts as f (f.kind)}
    {@const Icon = ICONS[f.kind]}
    <span class="it" class:dim={f.dim} data-fact={f.kind} title={t(LABEL[f.kind])}>
      <Icon weight="bold" {size} aria-hidden="true" />
      <span class="sr-only">{t(LABEL[f.kind])}: </span><span class="tx">{f.text}</span>
    </span>
  {/each}
</span>

<style>
  .il { display: flex; align-items: center; gap: 0 12px; min-width: 0; max-width: 100%; overflow: hidden; line-height: 1.4; color: inherit; }
  .il.wrap { flex-wrap: wrap; overflow: visible; }
  .it { display: inline-flex; align-items: center; gap: 5px; min-width: 0; white-space: nowrap; }
  .it :global(svg) { flex: none; color: var(--paper-mute); }
  .tx { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  /* the last fact takes the rest of the line and ends in an ellipsis rather than being cut */
  .il:not(.wrap) .it:last-child { flex: 0 1 auto; min-width: 0; }
  .dim { color: var(--paper-mute); }
</style>
