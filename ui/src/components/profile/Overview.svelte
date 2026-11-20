<script lang="ts">
  import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
  import PuzzlePieceIcon from 'phosphor-svelte/lib/PuzzlePieceIcon';
  import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
  import ScrollIcon from 'phosphor-svelte/lib/ScrollIcon';
  import ImagesIcon from 'phosphor-svelte/lib/ImagesIcon';
  import { fmtDate, fmtDuration, fmtRelative } from '../../lib/format';
  import { logLevel } from '../../lib/logs';
  import { actions, errorText, getApi, lang, nav, openDiscover, openProfile, t, toast } from '../../lib/stores.svelte';
  import type { LogEntry, ProfileView, Screenshot } from '../../lib/types';

  let { profile }: { profile: ProfileView } = $props();
  let shots = $state<Screenshot[] | null>(null);
  let excerpt = $state<{ entry: LogEntry; lines: string[] } | null | undefined>(undefined);

  $effect(() => {
    const id = profile.id;
    let dead = false;
    shots = null; excerpt = undefined;
    const api = getApi();
    api.screenshotsList({ profileId: id }).then((l) => { if (!dead) shots = l; }, () => { if (!dead) shots = []; });
    api.logsList({ profileId: id }).then(async (l) => {
      const e = l.find((x) => x.kind === 'game') ?? l[0];
      if (!e) { if (!dead) excerpt = null; return; }
      const r = await api.logsRead({ profileId: id, id: e.id });
      if (!dead) excerpt = { entry: e, lines: r.text.split('\n').slice(-12) };
    }).catch(() => { if (!dead) excerpt = null; });
    return () => { dead = true; };
  });

  async function reveal() {
    try { toast(await getApi().openFolder({ target: profile.id }), 'info'); } catch (e) { toast(errorText(e), 'error'); }
  }
  async function duplicate() {
    try {
      const p = await getApi().profileDuplicate({ id: profile.id });
      await actions.addProfile(p);
      openProfile(p.id);
    } catch (e) { toast(errorText(e), 'error'); }
  }
</script>

<div class="ov">
  <dl class="tiles">
    <div><dt>{t('home.playtime')}</dt><dd class="mono big">{fmtDuration(profile.playTimeMs)}</dd></div>
    <div><dt>{t('overview.created')}</dt><dd class="mono">{fmtDate(profile.created, lang())}</dd></div>
    <div><dt>{t('home.lastPlayed')}</dt><dd class="mono">{profile.lastPlayed ? fmtRelative(profile.lastPlayed, lang()) : t('home.never')}</dd></div>
    <div><dt>{t('home.mods')}</dt><dd class="mono">{profile.modCount}</dd></div>
  </dl>

  <section class="shots" aria-labelledby="ov-shots">
    <header><h2 id="ov-shots" class="eyebrow">{t('overview.screenshots')}</h2>
      {#if shots?.length}<button class="btn ghost" type="button" onclick={() => (nav.profileTab = 'media')}>{t('overview.all')}</button>{/if}
    </header>
    {#if shots === null}
      <div class="strip"><div class="sk"></div><div class="sk"></div><div class="sk"></div></div>
    {:else if shots.length === 0}
      <p class="hint">{t('media.empty')}</p>
    {:else}
      <ul class="strip">
        {#each shots.slice(0, 6) as s (s.name)}
          <li><button type="button" aria-label={s.name} onclick={() => (nav.profileTab = 'media')}><img src={s.url} alt="" loading="lazy" draggable="false" /></button></li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="log" aria-labelledby="ov-log">
    <header><h2 id="ov-log" class="eyebrow">{t('overview.lastLog')}{#if excerpt}<span class="mono dim">&nbsp;&middot;&nbsp;{excerpt.entry.name}</span>{/if}</h2>
      <button class="btn ghost" type="button" onclick={() => (nav.profileTab = 'logs')}>{t('overview.openLogs')}</button>
    </header>
    {#if excerpt === undefined}<div class="sk wide"></div>
    {:else if excerpt === null}<p class="hint">{t('logs.empty')}</p>
    {:else}
      <pre class="mono">{#each excerpt.lines as l, i (i)}<span class="ln {logLevel(l)}">{l}{'\n'}</span>{/each}</pre>
    {/if}
  </section>

  <section class="links" aria-labelledby="ov-links">
    <h2 id="ov-links" class="eyebrow">{t('overview.quick')}</h2>
    <div class="btns">
      <button class="btn" type="button" onclick={reveal}><FolderOpenIcon weight="bold" size={16} aria-hidden="true" />{t('overview.openFolder')}</button>
      <button class="btn" type="button" onclick={() => openDiscover(profile.id)}><PuzzlePieceIcon weight="bold" size={16} aria-hidden="true" />{t('content.browse')}</button>
      <button class="btn" type="button" onclick={() => (nav.profileTab = 'media')}><ImagesIcon weight="bold" size={16} aria-hidden="true" />{t('profile.tab.media')}</button>
      <button class="btn" type="button" onclick={() => (nav.profileTab = 'logs')}><ScrollIcon weight="bold" size={16} aria-hidden="true" />{t('profile.tab.logs')}</button>
      <button class="btn" type="button" onclick={duplicate}><CopyIcon weight="bold" size={16} aria-hidden="true" />{t('profile.duplicate')}</button>
    </div>
  </section>
</div>

<style>
  .ov { display: grid; gap: 22px; max-width: 980px; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 0; margin: 0; border: 1px solid var(--line); border-radius: var(--r-m); overflow: hidden; }
  .tiles > div { padding: 12px 16px; border-right: 1px solid var(--line-soft); background: var(--ink-1); }
  .tiles > div:last-child { border-right: 0; }
  dt { font-size: 12px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--paper-mute); font-weight: 600; }
  dd { margin: 4px 0 0; }
  .big { font-size: 22px; color: var(--mango); }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px; min-height: 36px; }
  .dim { color: var(--paper-mute); text-transform: none; letter-spacing: 0; font-weight: 400; }
  .strip { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 8px; list-style: none; margin: 0; padding: 0; }
  .strip button { display: block; width: 100%; aspect-ratio: 16 / 9; border-radius: var(--r-m); overflow: hidden; border: 1px solid var(--line); }
  .strip img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .strip button:hover { border-color: var(--mango); }
  .sk { aspect-ratio: 16 / 9; background: var(--ink-2); border-radius: var(--r-m); animation: shimmer 1.4s ease-in-out infinite; }
  .sk.wide { aspect-ratio: auto; height: 150px; }
  @keyframes shimmer { 50% { opacity: 0.5; } }
  pre { margin: 0; padding: 10px 14px; background: var(--ink-1); border: 1px solid var(--line); border-radius: var(--r-m); font-size: 12.5px; line-height: 1.55; overflow: auto; user-select: text; }
  .ln { color: var(--paper-dim); }
  .ln.warn { color: var(--mango); }
  .ln.error { color: var(--tomato); }
  .btns { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
</style>
