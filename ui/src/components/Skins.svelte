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
  import SkinViewer from './SkinViewer.svelte';
  import SkinFlat from './SkinFlat.svelte';
  import SkinUploadDialog from './SkinUploadDialog.svelte';
  import SkinUsernameDialog from './SkinUsernameDialog.svelte';
  import SkinSaveDialog from './SkinSaveDialog.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import EmptyState from './EmptyState.svelte';
  import LoadError from './LoadError.svelte';
  import Seg from './Seg.svelte';
  import Tile from './Tile.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import SkeletonRegion from './skeleton/SkeletonRegion.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { toAppError } from '../lib/api';
  import { drawCapeFront } from '../lib/skinFlat';
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
  $effect(() => {
    const id = uuid;
    void skinRev[id]; // `skin:updated`: reload, keeping what is on screen
    if (!id) return;
    const silent = untrack(() => info.data?.uuid === id);
    info.run(async () => ({ uuid: id, info: await getApi().skinGet({ uuid: id }) }), { silent });
    return () => info.cancel();
  });
  $effect(() => { lib.run(() => getApi().skinLibraryList()); return () => lib.cancel(); });

  const data = $derived(info.data?.uuid === uuid ? info.data.info : null);
  const editable = $derived(canEditSkin(data));
  const cape = $derived(activeCape(data));
  const library = $derived(sortLibrary(lib.data ?? []));
  let pose = $state<Pose>('idle');
  let viewMode = $state<'loading' | 'gl' | 'flat' | 'error'>('loading');
  let texPath = $state<TexPath | null>(null);

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
  async function setCape(id: string | null) {
    if (busy || (id === null ? !cape : cape?.id === id)) return;
    busy = 'cape'; actionError = '';
    try { setInfo(await getApi().skinSetCape({ uuid, capeId: id })); }
    catch (e) { fail(e, () => void setCape(id)); }
    busy = null;
  }

  /** Cape thumbnail: the outer face drawn from the cape image at integer scale. */
  function capeThumb(node: HTMLCanvasElement, url: string) {
    const draw = (u: string) => {
      const i = new Image();
      i.onload = () => { const c = node.getContext('2d'); if (c) { drawCapeFront(c, i, 6); node.dataset.ready = '1'; } };
      i.src = u;
    };
    draw(url);
    return { update: draw };
  }
  const poseOpts = $derived([{ value: 'idle', label: t('skins.pose.idle') }, { value: 'walking', label: t('skins.pose.walking') }]);
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
      <section class="stage on-stage" aria-label={t('skins.viewer')}>
        {#if info.phase === 'error' && !data}
          <div class="serr"><LoadError compact error={info.error} onretry={() => info.retry()} /></div>
        {:else if data}
          {#key uuid}
            <SkinViewer skinUrl={data.skinUrl} capeUrl={data.capeUrl} slim={data.slim} {pose} label={t('skins.viewer.label', { name: acc?.name ?? '' })} bind:mode={viewMode} bind:path={texPath} />
          {/key}
          <div class="badges">
            <span class="chip">{model}</span>
            {#if cape}<span class="chip">{cape.alias}</span>{/if}
          </div>
          {#if viewMode === 'gl'}
            <div class="bar">
              <span class="hint drag">{t('skins.drag')}</span>
              <Seg label={t('skins.pose')} value={pose} idPrefix="skpose" options={poseOpts} onchange={(v) => (pose = v as Pose)} />
            </div>
          {:else if viewMode === 'flat'}
            <p class="hint flatnote"><InfoIcon weight="bold" size={14} aria-hidden="true" />{t('skins.flat')}</p>
          {/if}
        {:else if info.showSkeleton}
          <span class="skel sbox" aria-hidden="true"></span>
        {/if}
      </section>

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
                    <li class="card">
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
                <ul class="capes" role="radiogroup" aria-label={t('skins.capes')}>
                  <li>
                    <button class="cape none" type="button" role="radio" aria-checked={!cape} disabled={busy === 'cape'} onclick={() => setCape(null)}>
                      <span class="cimg"><ProhibitIcon weight="bold" size={26} aria-hidden="true" /></span>
                      <span class="cn">{t('skins.noCape')}</span>
                      {#if !cape}<span class="mark"><CheckIcon weight="bold" size={12} aria-hidden="true" />{t('skins.active')}</span>{/if}
                    </button>
                  </li>
                  {#each data.capes as c (c.id)}
                    <li>
                      <button class="cape" class:on={c.active} type="button" role="radio" aria-checked={c.active} disabled={busy === 'cape'} onclick={() => setCape(c.id)}>
                        <span class="cimg"><canvas class="px" use:capeThumb={c.url} width="60" height="96" aria-hidden="true"></canvas></span>
                        <span class="cn">{c.alias}</span>
                        {#if c.active}<span class="mark"><CheckIcon weight="bold" size={12} aria-hidden="true" />{t('skins.active')}</span>{/if}
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
  .acc { display: inline-flex; align-items: center; gap: 10px; height: 44px; padding: 0 14px 0 8px; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-2); color: var(--paper-dim); transition: background var(--dur) var(--ease), border-color var(--dur) var(--ease); }
  .acc:hover { background: var(--ink-3); color: var(--paper); }
  .acc.on { border-color: var(--accent-fill); color: var(--paper); background: var(--ink-3); }
  .nm { font-weight: 600; }
  .kind { font-size: var(--fs-xs); color: var(--paper-mute); }
  .layout { display: grid; grid-template-columns: minmax(280px, 360px) minmax(0, 1fr); gap: var(--s-5); align-items: start; }
  .stage { position: sticky; top: 0; height: min(600px, calc(100vh - 190px)); min-height: 360px; border: 1px solid var(--hero-line); border-radius: var(--r-l); overflow: hidden; background: radial-gradient(70% 60% at 50% 38%, color-mix(in srgb, var(--accent-fill) 9%, var(--hero-bg)), var(--hero-bg) 70%); }
  .sbox { position: absolute; left: 50%; top: 8%; bottom: 14%; width: min(40%, 150px); transform: translateX(-50%); border-radius: var(--r-m); }
  .serr { padding: 20px; }
  .badges { position: absolute; left: 12px; top: 12px; display: flex; gap: 6px; flex-wrap: wrap; }
  .badges .chip { background: rgba(var(--scrim), 0.5); border-color: var(--hero-line); color: var(--paper); }
  .bar { position: absolute; left: 12px; right: 12px; bottom: 12px; display: grid; justify-items: start; gap: 8px; }
  .bar :global(.seg) { background: rgba(var(--scrim), 0.55); }
  .drag { font-size: var(--fs-xs); }
  .flatnote { position: absolute; left: 12px; right: 12px; bottom: 10px; margin: 0; display: flex; gap: 6px; align-items: center; font-size: var(--fs-xs); }
  .side { display: grid; gap: var(--s-4); min-width: 0; align-content: start; }
  .acts { display: flex; flex-wrap: wrap; gap: 8px; }
  .offline { display: flex; gap: 10px; align-items: flex-start; margin: 0; padding: 12px 14px; border: 1px solid var(--line); border-radius: var(--r-m); background: var(--ink-2); color: var(--paper-dim); max-width: 60ch; }
  .offline :global(svg) { flex: none; margin-top: 2px; color: var(--paper-mute); }
  .warn { margin: 0; color: var(--mango); }
  section { display: grid; gap: var(--s-3); }
  .sh { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
  h2 { font-size: var(--fs-xl); display: flex; align-items: baseline; gap: 8px; }
  .cnt { font-size: var(--fs-sm); color: var(--paper-mute); font-weight: 400; }
  .grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(136px, 1fr)); gap: 10px; }
  .card { display: grid; gap: 8px; padding: 10px; background: var(--ink-2); border: 1px solid var(--line); border-radius: var(--r-m); }
  .thumb { height: 128px; display: flex; justify-content: center; background: var(--ink-3); border-radius: var(--r-s); padding: 6px 0; }
  .meta { display: grid; gap: 1px; min-width: 0; }
  .n { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .v { font-size: var(--fs-xs); color: var(--paper-mute); }
  .cacts { display: flex; gap: 4px; }
  .cacts .btn:first-child { flex: 1; }
  .ic { width: 28px; padding: 0; }
  .capes { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 10px; }
  .cape { position: relative; display: grid; justify-items: center; gap: 6px; width: 108px; padding: 10px 8px 8px; background: var(--ink-2); border: 1px solid var(--line); border-radius: var(--r-m); transition: border-color var(--dur) var(--ease), background var(--dur) var(--ease); }
  .cape:hover:not(:disabled) { background: var(--ink-3); }
  .cape.on, .cape[aria-checked='true'] { border-color: var(--accent-fill); background: var(--ink-3); }
  .cimg { display: grid; place-items: center; width: 60px; height: 96px; color: var(--paper-mute); }
  .cimg canvas { width: 60px; height: 96px; }
  .cn { font-size: var(--fs-sm); text-align: center; line-height: 1.2; overflow-wrap: anywhere; }
  .mark { display: inline-flex; align-items: center; gap: 3px; font-size: var(--fs-xs); font-weight: 600; color: var(--mango); }
  .skbtns { display: flex; gap: 8px; flex-wrap: wrap; }
  .skgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(136px, 1fr)); gap: 10px; }
  :global(.sk) { display: grid; gap: var(--s-4); }
  @media (max-width: 1100px) { .layout { grid-template-columns: minmax(260px, 320px) minmax(0, 1fr); } }
</style>
