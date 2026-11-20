<script lang="ts">
  // Global activity indicator in the title strip: running installs and launches, with a popover that lists them.
  import PulseIcon from 'phosphor-svelte/lib/PulseIcon';
  import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
  import DownloadSimpleIcon from 'phosphor-svelte/lib/DownloadSimpleIcon';
  import { activityItems } from '../lib/activity';
  import { app, installs, launches, openProfile, runs, startInstall, t, tp } from '../lib/stores.svelte';

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();
  const items = $derived(activityItems({
    profiles: app.profiles, launches, installs, runs,
    label: (k, vars) => t(k, vars),
  }));
  const busy = $derived(items.filter((i) => i.kind !== 'running'));

  function go(it: (typeof items)[number]) {
    open = false;
    if (it.runId !== undefined) startInstall({ kind: 'run', runId: it.runId });
    else if (it.profileId) openProfile(it.profileId, it.kind === 'install' ? 'mods' : 'overview');
  }
  function onwin(e: MouseEvent) { if (open && root && !root.contains(e.target as Node)) open = false; }
  function onkey(e: KeyboardEvent) { if (open && e.key === 'Escape') { open = false; (root?.querySelector('.pill') as HTMLElement | null)?.focus(); } }
</script>

<svelte:window onclick={onwin} onkeydown={onkey} />

{#if items.length}
  <div class="act" bind:this={root}>
    <button class="pill" class:busy={busy.length > 0} type="button" aria-haspopup="true" aria-expanded={open}
      aria-label={t('activity.label', { n: items.length })} onclick={() => (open = !open)}>
      <span class:pulse={busy.length > 0} class="ic"><PulseIcon weight="bold" size={14} aria-hidden="true" /></span>
      <span class="txt">{busy.length ? tp('activity.running', busy.length) : tp('activity.playing', items.length)}</span>
    </button>
    {#if open}
      <div class="pop" role="region" aria-label={t('activity.title')}>
        <h2 class="eyebrow">{t('activity.title')}</h2>
        <ul>
          {#each items as it (it.key)}
            <li>
              <button type="button" class="it" onclick={() => go(it)}>
                <span class="k" aria-hidden="true">{#if it.kind === 'install'}<DownloadSimpleIcon weight="bold" size={16} />{:else}<PlayIcon weight="bold" size={16} />{/if}</span>
                <span class="t"><b>{it.title}</b><span class="hint">{it.detail}</span></span>
                {#if it.fraction !== null}<span class="bar step-end" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(it.fraction * 100)} aria-label={it.title}><span style:transform="scaleX({it.fraction})"></span></span>{/if}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </div>
{/if}

<style>
  .act { position: relative; }
  .pill { display: inline-flex; align-items: center; gap: 6px; height: 22px; padding: 0 10px; border: 1px solid var(--line); color: var(--paper-dim); font-size: var(--fs-xs); font-weight: 600; }
  .pill:hover { color: var(--paper); border-color: var(--paper-mute); }
  .pill.busy { color: var(--mango); border-color: color-mix(in srgb, var(--mango) 55%, transparent); }
  .ic { display: inline-flex; }
  .pulse { animation: pulse 1.4s ease-in-out infinite; }
  .pop { position: absolute; z-index: 60; right: 0; top: 30px; width: 340px; padding: 13px; }
  .pop h2 { margin-bottom: 6px; }
  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; }
  .it { display: grid; grid-template-columns: 20px minmax(0, 1fr); gap: 10px; align-items: center; width: 100%; padding: 8px; text-align: left; }
  .it:hover { background: var(--ink-4); }
  .k { color: var(--paper-dim); display: inline-flex; }
  .t { display: grid; min-width: 0; }
  .t b { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .t .hint { font-size: var(--fs-xs); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bar { grid-column: 2; height: 4px; background: var(--ink-4); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--accent-fill); transform-origin: left; transition: transform var(--dur) var(--ease); }
  @keyframes pulse { 50% { opacity: 0.35; } }
</style>
