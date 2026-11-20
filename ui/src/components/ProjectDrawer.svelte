<script lang="ts">
  import DownloadSimpleIcon from 'phosphor-svelte/lib/DownloadSimpleIcon';
  import HeartIcon from 'phosphor-svelte/lib/HeartIcon';
  import CheckCircleIcon from 'phosphor-svelte/lib/CheckCircleIcon';
  import WarningCircleIcon from 'phosphor-svelte/lib/WarningCircleIcon';
  import Modal from './Modal.svelte';
  import Tile from './Tile.svelte';
  import Img from './Img.svelte';
  import LoadError from './LoadError.svelte';
  import DrawerSkeleton from './skeleton/DrawerSkeleton.svelte';
  import ListRowSkeleton from './skeleton/ListRowSkeleton.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { fallbackImage } from '../lib/fallbackImage';
  import { fmtCompact, fmtDate } from '../lib/format';
  import { markdownLite } from '../lib/markdown';
  import type { Key } from '../lib/i18n';
  import { providerLabel } from '../lib/install';
  import { errorText, getApi, lang, t, toast } from '../lib/stores.svelte';
  import type { ContentItem, ProfileView, ProjectDetail, ProjectHit, ProjectVersion } from '../lib/types';

  interface Props {
    hit: ProjectHit;
    scope: ProfileView | null;
    installed: ContentItem | null;
    busy: boolean;
    progress: number;
    oninstall: (versionId?: string) => void;
    onclose: () => void;
  }
  let { hit, scope, installed, busy, progress, oninstall, onclose }: Props = $props();
  const detailLoad = createLoadState<ProjectDetail>({ isEmpty: () => false });
  const versLoad = createLoadState<ProjectVersion[]>({ isEmpty: () => false });
  const detail = $derived(detailLoad.data);
  const versions = $derived(versLoad.data);
  let shot = $state(0);

  $effect(() => {
    const id = hit.projectId;
    shot = 0;
    const api = getApi();
    detailLoad.run(() => api.projectDetail({ provider: hit.provider, projectId: id }));
    versLoad.run(() => api.projectVersions({ provider: hit.provider, projectId: id, ...(scope && hit.type !== 'modpack' ? { profileId: scope.id } : {}) }));
    return () => { detailLoad.cancel(); versLoad.cancel(); };
  });

  const blocks = $derived(detail ? markdownLite(detail.body) : []);
  const canInstall = true;
</script>

<Modal variant="drawer" title={hit.title} {onclose}>
  <div class="top">
    <Tile kind="mod" id={hit.projectId} src={hit.iconUrl} size={72} radius={10} />
    <div class="meta">
      <div class="hint">{providerLabel(hit.provider)} &middot; {t('discover.by', { author: hit.author })}</div>
      <p class="d">{hit.description}</p>
      <div class="stats mono">
        <span><DownloadSimpleIcon weight="bold" size={14} aria-hidden="true" />{fmtCompact(hit.downloads, lang())}</span>
        <span><HeartIcon weight="bold" size={14} aria-hidden="true" />{fmtCompact(hit.follows, lang())}</span>
        <span>{t('drawer.updated', { date: fmtDate(hit.dateModified, lang()) })}</span>
      </div>
    </div>
  </div>
  <div class="cta">
    <button class="btn accent" type="button" disabled={busy || !canInstall || (!!installed && !installed.updateAvailable && hit.type !== 'modpack')} onclick={() => oninstall()}>
      {busy ? `${t('discover.btn.installing')} ${progress}%` : installed ? (installed.updateAvailable ? t('discover.btn.update') : t('discover.btn.installed')) : hit.type === 'modpack' ? t('drawer.installPack') : t('discover.btn.install')}
    </button>
    {#if hit.type !== 'modpack' && scope}<span class="hint">{t('discover.installingTo')} <b>{scope.name}</b></span>{:else if hit.type !== 'modpack'}<span class="hint">{t('drawer.pickNext')}</span>{/if}
  </div>

  {#if detail}
    {#if detail.gallery.length}
      <section aria-label={t('drawer.gallery')}>
        <div class="big"><Img src={detail.gallery[shot]} alt={t('drawer.galleryAlt', { n: shot + 1, title: hit.title })} fallback={fallbackImage('mod', hit.projectId, 512)} /></div>
        <ul class="thumbs">
          {#each detail.gallery as g, i (g + i)}
            <li><button type="button" class:on={i === shot} aria-label={t('drawer.galleryAlt', { n: i + 1, title: hit.title })} aria-pressed={i === shot} onclick={() => (shot = i)}><Img src={g} lazy fallback={fallbackImage('mod', hit.projectId, 128)} /></button></li>
          {/each}
        </ul>
      </section>
    {/if}
    <section class="body" aria-label={t('drawer.about')}>
      {#each blocks as b, i (i)}
        {#if b.kind === 'h'}
          <h3 class="h{b.level}">{#each b.inline as s, j (j)}{#if s.kind === 'bold'}<b>{s.text}</b>{:else if s.kind === 'italic'}<i>{s.text}</i>{:else if s.kind === 'code'}<code>{s.text}</code>{:else}{s.text}{/if}{/each}</h3>
        {:else if b.kind === 'p'}
          <p>{#each b.inline as s, j (j)}{#if s.kind === 'bold'}<b>{s.text}</b>{:else if s.kind === 'italic'}<i>{s.text}</i>{:else if s.kind === 'code'}<code>{s.text}</code>{:else}{s.text}{/if}{/each}</p>
        {:else if b.kind === 'ul'}
          <ul>{#each b.items as li, k (k)}<li>{#each li as s, j (j)}{#if s.kind === 'bold'}<b>{s.text}</b>{:else if s.kind === 'italic'}<i>{s.text}</i>{:else if s.kind === 'code'}<code>{s.text}</code>{:else}{s.text}{/if}{/each}</li>{/each}</ul>
        {:else}
          <pre class="mono">{b.text}</pre>
        {/if}
      {/each}
    </section>
    <dl class="facts">
      {#if detail.license}<div><dt>{t('drawer.license')}</dt><dd>{detail.license}</dd></div>{/if}
      {#if detail.sourceUrl}<div><dt>{t('drawer.source')}</dt><dd class="mono">{detail.sourceUrl}</dd></div>{/if}
      {#if detail.issuesUrl}<div><dt>{t('drawer.issues')}</dt><dd class="mono">{detail.issuesUrl}</dd></div>{/if}
    </dl>
  {:else if detailLoad.phase === 'error'}
    <LoadError compact error={detailLoad.error} onretry={() => detailLoad.retry()} />
  {:else if detailLoad.showSkeleton}
    <DrawerSkeleton />
  {/if}

  <section aria-labelledby="dv-h">
    <h3 id="dv-h" class="eyebrow">{t('drawer.versions')}</h3>
    {#if versLoad.phase === 'error'}
      <LoadError compact error={versLoad.error} onretry={() => versLoad.retry()} />
    {:else if versions === null}
      {#if versLoad.showSkeleton}<ListRowSkeleton count={4} img={18} padding="8px 0" gap={10} bars={[{ w: '30%', h: 13 }, { w: '62%', h: 10 }]} tail={92} />{/if}
    {:else if versions.length === 0}
      <p class="hint">{t('drawer.noVersions')}</p>
    {:else}
      <ul class="vers">
        {#each versions as v (v.versionId)}
          <li>
            <span class="ok" class:no={!v.compatible} title={v.compatible ? t('drawer.compatible') : t('drawer.incompatible')}>
              {#if v.compatible}<CheckCircleIcon weight="bold" size={18} aria-hidden="true" />{:else}<WarningCircleIcon weight="bold" size={18} aria-hidden="true" />{/if}
              <span class="sr-only">{v.compatible ? t('drawer.compatible') : t('drawer.incompatible')}</span>
            </span>
            <div class="vm">
              <b class="mono">{v.versionNumber}</b>
              <span class="tag {v.type}">{t(`drawer.vtype.${v.type}` as Key)}</span>
              <div class="hint mono">{v.gameVersions.slice(0, 3).join(', ')}{v.gameVersions.length > 3 ? ' ...' : ''} &middot; {v.loaders.join(', ')} &middot; {fmtDate(v.datePublished, lang())}</div>
            </div>
            <button class="btn" type="button" disabled={busy || !canInstall} onclick={() => oninstall(v.versionId)} aria-label="{t('drawer.installVersion')} {v.versionNumber}">{t('drawer.installVersion')}</button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</Modal>

<style>
  .top { display: flex; gap: 14px; }
  .meta { display: grid; gap: 4px; min-width: 0; }
  .d { margin: 0; color: var(--paper-dim); }
  .stats { display: flex; flex-wrap: wrap; gap: 12px; font-size: 13px; color: var(--paper-dim); }
  .stats span { display: inline-flex; align-items: center; gap: 5px; }
  .cta { display: flex; align-items: center; gap: 12px; margin: 14px 0; }
  section { margin-top: 16px; }
  .big { width: 100%; aspect-ratio: 16 / 9; border-radius: var(--r-m); border: 1px solid var(--line); background: var(--ink-1); overflow: hidden; }
  .thumbs { display: flex; gap: 6px; list-style: none; margin: 8px 0 0; padding: 0; overflow: auto; }
  .thumbs button { width: 84px; aspect-ratio: 16 / 9; border-radius: var(--r-s); overflow: hidden; border: 2px solid transparent; }
  .thumbs button.on { border-color: var(--mango); }
  .body :global(p) { margin: 0 0 10px; color: var(--paper-dim); }
  .body h3 { margin: 14px 0 6px; font-size: 17px; }
  .body h3.h1 { font-size: 20px; }
  .body ul { margin: 0 0 10px; padding-left: 20px; color: var(--paper-dim); }
  .body code { font-family: var(--font-mono); font-size: 12.5px; background: var(--ink-4); padding: 1px 5px; border-radius: 3px; }
  pre { margin: 0 0 10px; padding: 10px 12px; background: var(--ink-1); border: 1px solid var(--line); border-radius: var(--r-m); font-size: 12.5px; overflow: auto; white-space: pre-wrap; }
  .facts { margin: 12px 0 0; display: grid; gap: 4px; }
  .facts div { display: flex; gap: 12px; }
  dt { width: 80px; flex: none; color: var(--paper-mute); font-size: 13px; }
  dd { margin: 0; font-size: 13px; overflow-wrap: anywhere; }
  h3.eyebrow { margin-bottom: 6px; }
  .vers { list-style: none; margin: 0; padding: 0; display: grid; }
  .vers li { display: grid; grid-template-columns: 22px minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 8px 0; border-bottom: 1px solid var(--line-soft); }
  .ok { color: var(--leaf); display: inline-flex; }
  .ok.no { color: var(--mango); }
  .vm { min-width: 0; }
  .vm .hint { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tag { font-size: 11.5px; padding: 1px 6px; border-radius: var(--r-s); background: var(--ink-4); color: var(--paper-dim); margin-left: 6px; }
  .tag.beta, .tag.alpha { background: rgba(255, 173, 50, 0.16); color: var(--mango); }
</style>
