<script lang="ts">
  // Skins: an account switcher, the 3D player model (or the flat fallback), actions (upload, use another player's skin, save to the library),
  // the local skin library and the capes of the account. Offline accounts get the viewer and one line instead of the editing surface.
  // Everything refreshes on `skin:updated` (skinRev), which also gives every head in the app a new `?v=` (stores refreshAccounts).
  import { untrack } from 'svelte';
  import CloudArrowUpIcon from 'phosphor-svelte/lib/CloudArrowUpIcon';
  import UserPlusIcon from 'phosphor-svelte/lib/UserPlusIcon';
  import BookmarkSimpleIcon from 'phosphor-svelte/lib/BookmarkSimpleIcon';
  import PlusIcon from 'phosphor-svelte/lib/PlusIcon';
  import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
  import ProhibitIcon from 'phosphor-svelte/lib/ProhibitIcon';
  import CheckIcon from 'phosphor-svelte/lib/CheckIcon';
  import InfoIcon from 'phosphor-svelte/lib/InfoIcon';
  import PersonSimpleIcon from 'phosphor-svelte/lib/PersonSimpleIcon';
  import PersonSimpleWalkIcon from 'phosphor-svelte/lib/PersonSimpleWalkIcon';
  import FlagIcon from 'phosphor-svelte/lib/FlagIcon';
  import ArrowCounterClockwiseIcon from 'phosphor-svelte/lib/ArrowCounterClockwiseIcon';
  import CircleNotchIcon from 'phosphor-svelte/lib/CircleNotchIcon';
  import SkinViewer from './SkinViewer.svelte';
  import SkinFlat from './SkinFlat.svelte';
  import SkinUploadDialog from './SkinUploadDialog.svelte';
  import SkinUsernameDialog from './SkinUsernameDialog.svelte';
  import SkinSaveDialog from './SkinSaveDialog.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import EmptyState from './EmptyState.svelte';
  import LoadError from './LoadError.svelte';
  import CapeThumb from './CapeThumb.svelte';
  import Img from './Img.svelte';
  import Tile from './Tile.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import SkeletonRegion from './skeleton/SkeletonRegion.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { toAppError } from '../lib/api';
  import { brandIcon } from '../lib/fallbackImage';
  import { cachedLibrary, cachedSkin, rememberLibrary, rememberSkin } from '../lib/skinCache';
  import { createCapeFlow } from '../lib/capeFlow';
  import { contextmenu } from '../lib/contextMenu.svelte';
  import { librarySkinMenu, skinStageMenu } from '../lib/menus';
  import { activeCape, canEditSkin, sortLibrary } from '../lib/skinFlows';
  import type { Pose } from '../lib/skin3d';
  import type { TexPath } from '../lib/skinTexture';
  import { tip } from '../lib/tooltip';
  import { accountNeedsLogin, app, auth, errorText, getApi, nav, selectedAccount, skinRev, t, toast } from '../lib/stores.svelte';
  import type { LibrarySkin, SkinInfo } from '../lib/types';

  let uuid = $state(untrack(() => selectedAccount()?.uuid ?? app.accounts[0]?.uuid ?? ''));
  const acc = $derived(app.accounts.find((a) => a.uuid === uuid) ?? null);
  // an account removed while the page is open: fall back to the first one
  $effect(() => { if (!acc && app.accounts.length) uuid = selectedAccount()?.uuid ?? app.accounts[0].uuid; });

  /** The loaded info is tagged with its account, so a slow answer for the previous account never shows for the new one. */
  const info = createLoadState<{ uuid: string; info: SkinInfo }>({ isEmpty: () => false });
  const setInfo = (i: SkinInfo): void => info.set({ uuid, info: i });
  const lib = createLoadState<LibrarySkin[]>({ isEmpty: () => false });
  const flows = new Map<string, ReturnType<typeof createCapeFlow>>();
  $effect(() => {
    const id = uuid;
    void skinRev[id]; // `skin:updated`: reload, keeping what is on screen
    if (!id) return;
    if (untrack(() => flows.get(id)?.busy)) return; // a cape change in flight owns the state; its answer (or rollback) is the truth
    // cached-first: the last known skin shows at once, the skeleton is only for the very first load of an account
    const cached = untrack(() => (info.data?.uuid === id ? null : cachedSkin(id)));
    if (cached) info.set({ uuid: id, info: cached });
    const silent = untrack(() => info.data?.uuid === id);
    info.run(async () => ({ uuid: id, info: await getApi().skinGet({ uuid: id }) }), { silent });
    return () => info.cancel();
  });
  $effect(() => { const d = info.data; if (d) rememberSkin(d.uuid, d.info); });
  const cachedLib = cachedLibrary();
  if (cachedLib) lib.set(cachedLib);
  $effect(() => { lib.run(() => getApi().skinLibraryList(), { silent: untrack(() => lib.data !== null) }); return () => lib.cancel(); });
  $effect(() => { if (lib.data) rememberLibrary(lib.data); });

  const data = $derived(info.data?.uuid === uuid ? info.data.info : null);
  const editable = $derived(canEditSkin(data));
  const cape = $derived(activeCape(data));
  const library = $derived(sortLibrary(lib.data ?? []));
  let pose = $state<Pose>('idle');
  let capeAnim = $state(true);
  let viewer: SkinViewer | undefined = $state();
  /** Cape thumbnails start after the model is on screen (or after a short grace), never before. */
  let thumbsOn = $state(false);
  let viewMode = $state<'loading' | 'gl' | 'flat' | 'error'>('loading');
  let texPath = $state<TexPath | null>(null);
  $effect(() => {
    if (!data) { thumbsOn = false; return; }
    if (viewMode !== 'loading') { thumbsOn = true; return; }
    const h = setTimeout(() => { thumbsOn = true; }, 1200);
    return () => clearTimeout(h);
  });
  const stale = $derived(!!data && (data as { stale?: boolean }).stale === true);

  // `stale` = the service answered from its cache and revalidates in the background (`skin:updated` only fires on a real change): show the cached
  // skin at once and quietly ask again a few times until it is fresh; the hint is visible only meanwhile.
  $effect(() => {
    if (!stale) return;
    const id = uuid; let n = 0;
    const h = setInterval(() => {
      if (++n > 4) { clearInterval(h); return; }
      if (!flows.get(id)?.busy) info.run(async () => ({ uuid: id, info: await getApi().skinGet({ uuid: id }) }), { silent: true });
    }, 2000);
    return () => clearInterval(h);
  });

  let dialog = $state<null | 'upload' | 'addLib' | 'user' | 'save'>(null);
  let removing = $state<LibrarySkin | null>(null);
  let busy = $state<string | null>(null);
  let actionError = $state('');
  const model = $derived(data ? (data.slim ? t('skins.slim') : t('skins.classic')) : '');

  /** The one place edit errors go: a dead session opens the relogin dialog and retries, anything else shows inline. */
  function onrelogin(retry: () => void) {
    auth.error = null;
    auth.relogin = { profileId: '', retry };
  }
  function fail(e: unknown, retry?: () => void) {
    if (toAppError(e).needsRelogin) { onrelogin(retry ?? (() => {})); return; }
    actionError = errorText(e);
  }
  function applied(i?: SkinInfo) {
    if (i) setInfo(i);
    dialog = null; actionError = '';
    toast(t('skins.applied'), 'ok');
  }
  async function applyLib(s: LibrarySkin) {
    if (busy) return;
    busy = `lib:${s.id}`; actionError = '';
    try { setInfo(await getApi().skinLibraryApply({ uuid, id: s.id })); toast(t('skins.appliedNamed', { name: s.name }), 'ok'); }
    catch (e) { fail(e, () => void applyLib(s)); }
    busy = null;
  }
  async function removeLib() {
    const s = removing;
    if (!s) return;
    busy = `rm:${s.id}`;
    try { lib.set(await getApi().skinLibraryRemove({ id: s.id })); toast(t('skins.removed', { name: s.name }), 'info'); }
    catch (e) { actionError = errorText(e); }
    busy = null; removing = null;
  }

  /** The cape being saved (`null` = "no cape"), `undefined` when none; drives the inline indicator on that item. */
  let savingCape = $state<string | null | undefined>(undefined);
  let lastCape: string | null = null;
  function capeFlowFor(u: string) {
    let f = flows.get(u);
    if (!f) {
      f = createCapeFlow({
        current: () => (info.data?.uuid === u ? info.data.info : cachedSkin(u)),
        commit: (i) => { if (uuid === u) info.set({ uuid: u, info: i }); else rememberSkin(u, i); },
        call: (capeId) => getApi().skinSetCape({ uuid: u, capeId }),
        saving: (id) => { if (uuid === u) savingCape = id; },
        fail: (e, raw) => {
          if (e.relogin) { fail(raw, () => void setCape(lastCape)); return; }
          toast(t(e.key, e.vars), 'error', 8000, e.retry ? { label: t('common.retry'), run: () => void setCape(lastCape) } : undefined);
        },
      });
      flows.set(u, f);
    }
    return f;
  }
  /** Optimistic: the cape is active in the list and on the model at once; on failure the previous cape is back and a specific error shows. */
  function setCape(id: string | null) {
    lastCape = id;
    return capeFlowFor(uuid).request(id);
  }
</script>

<div class="skins">
  <header>
    <div class="ttl"><h1 class="display">{t('nav.skins')}</h1><div class="hint">{t('skins.hint')}</div></div>
    {#if app.accounts.length > 1}
      <div class="who" role="radiogroup" aria-label={t('skins.account')}>
        {#each app.accounts as a (a.uuid)}
          <button type="button" class="acc" class:on={a.uuid === uuid} role="radio" aria-checked={a.uuid === uuid} onclick={() => (uuid = a.uuid)}>
            <Tile kind="account" id={a.uuid} src={a.headUrl} size={28} radius={4} />
            <span class="nm">{a.name}</span>
            <span class="kind">{a.type === 'microsoft' ? t('accounts.msa') : t('accounts.offline')}</span>
          </button>
        {/each}
      </div>
    {/if}
  </header>

  {#if !app.accounts.length}
    <EmptyState title={t('skins.noAccount')} hint={t('skins.noAccountHint')}>
      <button class="btn accent" type="button" onclick={(e) => { e.stopPropagation(); nav.accounts = true; }}>{t('play.addAccount')}</button>
    </EmptyState>
  {:else}
    <div class="layout">
      <div class="stagecol" use:contextmenu={() => skinStageMenu(() => { actionError = ''; dialog = 'save'; }, editable)}>
        <section class="stage on-stage nb" style:--n="18px" aria-label={t('skins.viewer')}>
          <span class="wm px" aria-hidden="true"><Img src={brandIcon(512)} fallback={brandIcon(512)} fit="contain" pixel fbPad="0" /></span>
          <span class="glow" aria-hidden="true"></span>
          <span class="floor" aria-hidden="true"></span>
          {#if info.phase === 'error' && !data}
            <div class="serr"><LoadError compact error={info.error} onretry={() => info.retry()} /></div>
          {:else if data}
            {#key uuid}
              <SkinViewer bind:this={viewer} skinUrl={data.skinUrl} capeUrl={data.capeUrl} slim={data.slim} {pose} {capeAnim} label={t('skins.viewer.label', { name: acc?.name ?? '' })} bind:mode={viewMode} bind:path={texPath} />
            {/key}
          {:else if info.showSkeleton}
            <span class="skel sbox" aria-hidden="true"></span>
          {/if}
        </section>
        {#if data}
          {#if viewMode === 'flat'}
            <p class="hint flatnote"><InfoIcon weight="bold" size={14} aria-hidden="true" />{t('skins.flat')}</p>
          {:else}
            <div class="tools" role="toolbar" aria-label={t('skins.toolbar')}>
              <span class="tag mono">{model}</span>
              {#if cape}<span class="tag">{cape.alias}</span>{/if}
              <span class="grow"></span>
              <div class="grp" role="group" aria-label={t('skins.pose.label')}>
                <button type="button" class="ib" aria-pressed={pose === 'idle'} aria-label={t('skins.pose.idle')} use:tip={{ text: t('skins.pose.idle'), side: 'top' }} onclick={() => (pose = 'idle')}><PersonSimpleIcon weight="bold" size={18} aria-hidden="true" /></button>
                <button type="button" class="ib" aria-pressed={pose === 'walking'} aria-label={t('skins.pose.walking')} use:tip={{ text: t('skins.pose.walking'), side: 'top' }} onclick={() => (pose = 'walking')}><PersonSimpleWalkIcon weight="bold" size={18} aria-hidden="true" /></button>
              </div>
              <button type="button" class="ib solo" aria-pressed={capeAnim} aria-disabled={!cape ? 'true' : undefined} aria-label={t('skins.capeAnim')} use:tip={{ text: cape ? t('skins.capeAnim') : t('skins.noCape'), side: 'top' }}
                onclick={() => { if (cape) capeAnim = !capeAnim; }}><FlagIcon weight="bold" size={18} aria-hidden="true" /></button>
              <button type="button" class="ib solo" aria-label={t('skins.resetView')} use:tip={{ text: t('skins.resetView'), side: 'top' }} onclick={() => viewer?.resetView()}><ArrowCounterClockwiseIcon weight="bold" size={18} aria-hidden="true" /></button>
            </div>
            {#if viewMode === 'gl'}<span class="hint drag">{t('skins.drag')}</span>{/if}
          {/if}
          {#if stale}<p class="hint stale" role="status">{t('skins.skinCached')}</p>{/if}
        {/if}
      </div>

      <div class="side" aria-busy={info.phase === 'loading' && !data}>
        {#if !data && info.phase !== 'error'}
          {#if info.showSkeleton}
            <SkeletonRegion class="sk">
              <div class="skbtns"><Skeleton variant="box" w={132} h={36} radius={8} /><Skeleton variant="box" w={196} h={36} radius={8} /><Skeleton variant="box" w={150} h={36} radius={8} /></div>
              <Skeleton variant="bar" lh={30} h={18} w={120} />
              <div class="skgrid">{#each Array(4) as _, i (i)}<Skeleton variant="box" w="100%" h={190} radius={8} />{/each}</div>
            </SkeletonRegion>
          {/if}
        {:else if data}
          {#if !editable}
            <p class="offline"><InfoIcon weight="bold" size={16} aria-hidden="true" />{t('skins.offlineNote')}</p>
          {:else}
            <div class="acts">
              <button class="btn accent" type="button" onclick={() => { actionError = ''; dialog = 'upload'; }}><CloudArrowUpIcon weight="bold" size={16} aria-hidden="true" />{t('skins.upload')}</button>
              <button class="btn" type="button" onclick={() => { actionError = ''; dialog = 'user'; }}><UserPlusIcon weight="bold" size={16} aria-hidden="true" />{t('skins.user')}</button>
              <button class="btn" type="button" onclick={() => { actionError = ''; dialog = 'save'; }}><BookmarkSimpleIcon weight="bold" size={16} aria-hidden="true" />{t('skins.save')}</button>
            </div>
            {#if actionError}<div class="err" role="alert">{actionError}</div>{/if}
            {#if acc && accountNeedsLogin(acc)}<p class="hint warn">{t('skins.needsLogin')}</p>{/if}

            <section aria-labelledby="sk-lib-h">
              <div class="sh">
                <h2 id="sk-lib-h">{t('skins.library')}{#if lib.data}<span class="cnt mono">{lib.data.length}</span>{/if}</h2>
                <button class="btn text" type="button" onclick={() => (dialog = 'addLib')}><PlusIcon weight="bold" size={14} aria-hidden="true" />{t('skins.addLib')}</button>
              </div>
              {#if lib.phase === 'error'}
                <LoadError compact error={lib.error} onretry={() => lib.retry()} />
              {:else if lib.phase === 'loading' || lib.phase === 'idle'}
                {#if lib.showSkeleton}
                  <SkeletonRegion><div class="grid">{#each Array(4) as _, i (i)}<Skeleton variant="box" w="100%" h={196} radius={8} />{/each}</div></SkeletonRegion>
                {/if}
              {:else if library.length === 0}
                <EmptyState title={t('skins.libEmptyTitle')} hint={t('skins.libEmpty')}>
                  <button class="btn" type="button" onclick={() => (dialog = 'save')}><BookmarkSimpleIcon weight="bold" size={16} aria-hidden="true" />{t('skins.save')}</button>
                </EmptyState>
              {:else}
                <ul class="grid">
                  {#each library as s (s.id)}
                    <li class="card nb" style:--n="8px" use:contextmenu={() => librarySkinMenu(s, { apply: (x) => void applyLib(x), remove: (x) => (removing = x), busy: busy !== null })}>
                      <div class="thumb"><SkinFlat url={s.url} slim={s.variant === 'slim'} scale={4} label={s.name} /></div>
                      <div class="meta"><b class="n" title={s.name}>{s.name}</b><span class="v mono">{s.variant === 'slim' ? t('skins.slim') : t('skins.classic')}</span></div>
                      <div class="cacts">
                        <button class="btn small" type="button" disabled={busy !== null} aria-label={t('skins.apply', { name: s.name })} onclick={() => applyLib(s)}>{busy === `lib:${s.id}` ? t('skins.applying') : t('skins.applyShort')}</button>
                        <button class="btn small ghost ic" type="button" disabled={busy !== null} use:tip={{ text: t('skins.remove'), side: 'top' }} aria-label={t('skins.removeNamed', { name: s.name })} onclick={() => (removing = s)}><TrashIcon weight="bold" size={14} aria-hidden="true" /></button>
                      </div>
                    </li>
                  {/each}
                </ul>
              {/if}
            </section>

            <section aria-labelledby="sk-cape-h">
              <div class="sh"><h2 id="sk-cape-h">{t('skins.capes')}</h2></div>
              {#if data.capes.length === 0}
                <p class="hint">{t('skins.noCapes')}</p>
              {:else}
                <ul class="capes" role="radiogroup" aria-label={t('skins.capes')} aria-busy={savingCape !== undefined}>
                  <li>
                    <button class="cape none" type="button" role="radio" aria-checked={!cape} data-wait={savingCape !== undefined && savingCape !== null ? '' : undefined} onclick={() => setCape(null)}>
                      <span class="cimg"><ProhibitIcon weight="bold" size={26} aria-hidden="true" /></span>
                      <span class="cn">{t('skins.noCape')}</span>
                      {#if savingCape === null}<span class="mark saving" role="status"><CircleNotchIcon class="spin" weight="bold" size={12} aria-hidden="true" />{t('skins.saving')}</span>
                      {:else if !cape}<span class="mark"><CheckIcon weight="bold" size={12} aria-hidden="true" />{t('skins.active')}</span>{/if}
                    </button>
                  </li>
                  {#each data.capes as c (c.id)}
                    <li>
                      <button class="cape" class:on={c.active} type="button" role="radio" aria-checked={c.active} data-wait={savingCape !== undefined && savingCape !== c.id ? '' : undefined} onclick={() => setCape(c.id)}>
                        <span class="cimg"><CapeThumb url={c.url} start={thumbsOn} /></span>
                        <span class="cn">{c.alias}</span>
                        {#if savingCape === c.id}<span class="mark saving" role="status"><CircleNotchIcon class="spin" weight="bold" size={12} aria-hidden="true" />{t('skins.saving')}</span>
                        {:else if c.active}<span class="mark"><CheckIcon weight="bold" size={12} aria-hidden="true" />{t('skins.active')}</span>{/if}
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </section>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>

{#if dialog === 'upload'}<SkinUploadDialog mode="upload" {uuid} onclose={() => (dialog = null)} ondone={applied} {onrelogin} />{/if}
{#if dialog === 'addLib'}<SkinUploadDialog mode="library" {uuid} onclose={() => (dialog = null)} ondone={() => { dialog = null; lib.retry(); toast(t('skins.addedLib'), 'ok'); }} {onrelogin} />{/if}
{#if dialog === 'user'}<SkinUsernameDialog {uuid} current={data} accountName={acc?.name ?? ''} onclose={() => { dialog = null; lib.retry(); }} ondone={setInfo} {onrelogin} />{/if}
{#if dialog === 'save'}<SkinSaveDialog {uuid} defaultName={acc?.name ?? 'Skin'} onclose={() => (dialog = null)} ondone={() => { dialog = null; lib.retry(); toast(t('skins.savedLib'), 'ok'); }} />{/if}
{#if removing}
  <ConfirmDialog danger title={t('skins.removeTitle', { name: removing.name })} body={t('skins.removeBody')} confirmLabel={t('skins.remove')} busy={busy !== null} onconfirm={removeLib} oncancel={() => (removing = null)} />
{/if}

<style>
  .skins { height: 100%; overflow: auto; padding: var(--s-5) 32px 32px; display: flex; flex-direction: column; gap: var(--s-4); }
  header { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; flex-wrap: wrap; }
  h1 { font-size: var(--fs-3xl); }
  .who { display: flex; gap: 6px; flex-wrap: wrap; }
  .acc { display: inline-flex; align-items: center; gap: 10px; height: 44px; padding: 0 14px 0 8px; border: 1px solid var(--line); background: var(--ink-2); color: var(--paper-dim); transition: background var(--dur) var(--ease), border-color var(--dur) var(--ease); }
  .acc:hover { background: var(--ink-3); color: var(--paper); }
  .acc.on { border-color: var(--accent-fill); color: var(--paper); background: var(--ink-3); }
  .nm { font-weight: 600; }
  .kind { font-size: var(--fs-xs); color: var(--paper-mute); }
  .layout { display: grid; grid-template-columns: minmax(280px, 360px) minmax(0, 1fr); gap: var(--s-5); align-items: start; }
  .stagecol { position: sticky; top: 0; display: grid; gap: 10px; min-width: 0; align-content: start; }
  /* the box holds only the model: a faint MangoClient watermark, a soft light behind the head and a floor ellipse under the feet, all CSS */
  .stage { --fill: radial-gradient(70% 55% at 50% 34%, color-mix(in srgb, var(--accent-fill) 13%, var(--hero-bg)), var(--hero-bg) 72%); height: min(560px, calc(100vh - 250px)); min-height: 340px; }
  .wm { position: absolute; inset: 14% 8% 22%; opacity: 0.07; pointer-events: none; filter: grayscale(0.4); }
  .glow { position: absolute; left: 50%; top: 4%; width: 78%; aspect-ratio: 1; transform: translateX(-50%); background: radial-gradient(closest-side, color-mix(in srgb, var(--accent-fill) 22%, transparent), transparent); opacity: 0.55; pointer-events: none; }
  .floor { position: absolute; left: 50%; bottom: 7%; width: 66%; height: 9%; transform: translateX(-50%); pointer-events: none;
    background: radial-gradient(closest-side, rgba(0, 0, 0, 0.62), rgba(0, 0, 0, 0.28) 55%, transparent 100%), radial-gradient(closest-side, color-mix(in srgb, var(--accent-fill) 20%, transparent) 58%, transparent 62%); }
  .stage :global(.viewer) { position: absolute; inset: 0; z-index: 1; }
  .sbox { position: absolute; left: 50%; top: 8%; bottom: 14%; width: min(40%, 150px); transform: translateX(-50%); z-index: 1; }
  .serr { position: relative; z-index: 1; padding: 20px; }
  .tools { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .grow { flex: 1; }
  .tag { display: inline-flex; align-items: center; height: 26px; padding: 0 9px; border: 1px solid var(--line); background: var(--ink-2); font-size: var(--fs-sm); color: var(--paper-dim); }
  .grp { display: inline-flex; border: 1px solid var(--line); background: var(--ink-1); }
  .ib { width: 34px; height: 32px; display: grid; place-items: center; color: var(--paper-dim); transition: background var(--dur) var(--ease), color var(--dur) var(--ease); }
  .ib.solo { border: 1px solid var(--line); background: var(--ink-1); }
  .ib:hover:not([aria-disabled='true']) { background: var(--ink-3); color: var(--paper); }
  .ib[aria-pressed='true'] { background: var(--ink-4); color: var(--mango); box-shadow: inset 0 -2px 0 var(--accent-fill); }
  .ib[aria-disabled='true'] { opacity: 0.45; }
  .ib:focus-visible { outline-offset: -3px; }
  .drag, .stale { font-size: var(--fs-xs); margin: 0; }
  .flatnote { margin: 0; display: flex; gap: 6px; align-items: center; font-size: var(--fs-xs); }
  .side { display: grid; gap: var(--s-4); min-width: 0; align-content: start; }
  .acts { display: flex; flex-wrap: wrap; gap: 8px; }
  .offline { display: flex; gap: 10px; align-items: flex-start; margin: 0; padding: 12px 14px; border: 1px solid var(--line); background: var(--ink-2); color: var(--paper-dim); max-width: 60ch; }
  .offline :global(svg) { flex: none; margin-top: 2px; color: var(--paper-mute); }
  .warn { margin: 0; color: var(--mango); }
  section { display: grid; gap: var(--s-3); }
  .sh { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
  h2 { font-size: var(--fs-xl); display: flex; align-items: baseline; gap: 8px; }
  .cnt { font-size: var(--fs-sm); color: var(--paper-mute); font-weight: 400; }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(136px, 1fr)); gap: 10px; }
  .card { display: grid; gap: 8px; padding: 11px; }
  .thumb { height: 128px; display: flex; justify-content: center; background: var(--ink-3); padding: 6px 0; }
  .meta { display: grid; gap: 1px; min-width: 0; }
  .n { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .v { font-size: var(--fs-xs); color: var(--paper-mute); }
  .cacts { display: flex; gap: 4px; }
  .cacts .btn:first-child { flex: 1; }
  .ic { width: 28px; padding: 0; }
  .capes { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 10px; }
  .cape { position: relative; display: grid; justify-items: center; gap: 6px; width: 108px; padding: 10px 8px 8px; background: var(--ink-2); border: 1px solid var(--line); transition: border-color var(--dur) var(--ease), background var(--dur) var(--ease), opacity var(--dur) var(--ease); }
  .cape:hover { background: var(--ink-3); }
  .cape[data-wait] { opacity: 0.55; }
  .mark.saving { color: var(--paper-dim); }
  .mark :global(.spin) { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .cape.on, .cape[aria-checked='true'] { border-color: var(--accent-fill); background: var(--ink-3); }
  .cimg { display: grid; place-items: center; width: 60px; height: 96px; color: var(--paper-mute); }
  .cn { font-size: var(--fs-sm); text-align: center; line-height: 1.2; overflow-wrap: anywhere; }
  .mark { display: inline-flex; align-items: center; gap: 3px; font-size: var(--fs-xs); font-weight: 600; color: var(--mango); }
  .skbtns { display: flex; gap: 8px; flex-wrap: wrap; }
  .skgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(136px, 1fr)); gap: 10px; }
  :global(.sk) { display: grid; gap: var(--s-4); }
  @media (max-width: 1100px) { .layout { grid-template-columns: minmax(260px, 320px) minmax(0, 1fr); } }
</style>
