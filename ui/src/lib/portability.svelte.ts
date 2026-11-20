// State and actions of Part 9: import (plan -> import with progress), export, diagnostics, the crash panel and the `files:dropped` handling.
// Dialog state lives here (not in the dialogs) so an import keeps running when its dialog is hidden, and so tests can drive the flows.
import { toAppError } from './api';
import {
  actions, app, eventHooks, errorText, getApi, installs, nav, openBlocked, openProfile, t, toast, tp,
} from './stores.svelte';
import {
  classifyDrop, importDefaultName, importReduce, initialImport, nameValid, type ImportEvent, type ImportState,
} from './portability';

export const pf = $state({
  imp: null as ImportState | null,
  /** The import dialog is visible (an import in progress can run with the dialog hidden). */
  impOpen: false,
  exp: null as { profileId: string } | null,
  diag: null as { profileId: string } | null,
  crash: null as { profileId: string; logId?: string } | null,
  /** A file is being dragged over the window (drop hint overlay). */
  dragging: false,
});

export function resetPortability(): void {
  pf.imp = null; pf.impOpen = false; pf.exp = null; pf.diag = null; pf.crash = null; pf.dragging = false;
}

const step = (ev: ImportEvent): void => { if (pf.imp) pf.imp = importReduce(pf.imp, ev); };
const importBusy = (): boolean => pf.imp?.s === 'planning' || pf.imp?.s === 'importing';

/** Opens the import dialog for a file and reads its plan. */
export async function openImportFor(path: string): Promise<void> {
  if (importBusy()) { toast(t('import.drop.busy'), 'info'); pf.impOpen = true; return; }
  pf.imp = initialImport(path);
  pf.impOpen = true;
  const mine = pf.imp;
  try {
    const plan = await getApi().instanceImportPlan({ filePath: path });
    if (pf.imp === mine) step({ e: 'planned', plan });
  } catch (e) {
    if (pf.imp === mine) step({ e: 'failed', code: toAppError(e).code });
  }
}

export function renameImport(name: string): void { step({ e: 'rename', name }); }

/** Starts the import of the planned file. Progress arrives through `install:progress` (`installs['import']`). */
export async function confirmImport(): Promise<void> {
  const st = pf.imp;
  if (st?.s !== 'plan' || !nameValid(st.name)) return;
  step({ e: 'start' });
  const started = pf.imp;
  try {
    const p = await getApi().instanceImport({ filePath: st.path, name: st.name.trim() || importDefaultName(st.plan, st.path) });
    await actions.addProfile(p);
    const visible = pf.impOpen && pf.imp === started;
    if (pf.imp === started) { step({ e: 'imported', profileId: p.id }); pf.imp = null; pf.impOpen = false; }
    delete installs['import'];
    toast(t('import.done', { name: p.name }), 'ok');
    if (p.pendingBlocked > 0) {
      openBlocked(p.id);
      openProfile(p.id, 'content');
      if (!visible) toast(tp('blocked.toast', p.pendingBlocked, { name: p.name }), 'info', 9000);
    } else if (visible) openProfile(p.id, 'overview');
    else toast(p.name, 'ok', 8000, { label: t('import.open'), run: () => openProfile(p.id) });
  } catch (e) {
    delete installs['import'];
    if (pf.imp === started) { step({ e: 'failed', code: toAppError(e).code }); pf.impOpen = true; }
    else toast(errorText(e), 'error', 9000);
  }
}

export const backToPlan = (): void => step({ e: 'back' });

/** Closing never cancels a running import: the dialog just hides and the work continues. */
export function closeImport(): void {
  if (pf.imp?.s === 'importing') { pf.impOpen = false; return; }
  pf.imp = null; pf.impOpen = false;
}

/** Library "Import" button: native file dialog, then the plan. A cancelled dialog does nothing. */
export async function pickAndImport(): Promise<void> {
  if (importBusy()) { pf.impOpen = true; return; }
  try {
    const path = await getApi().pickImportFile({ title: t('import.title') });
    if (typeof path === 'string' && path) await openImportFor(path);
  } catch (e) { toast(errorText(e), 'error'); }
}

/** `files:dropped`: the first .mrpack/.zip starts an import; anything else is said out loud instead of silently dropped. */
export function handleDropped(paths: readonly string[]): void {
  if (!paths.length) return;
  const d = classifyDrop(paths);
  if (!d.file) { toast(t('import.drop.nothing'), 'info'); return; }
  if (d.extra) toast(tp('import.drop.extra', d.extra), 'info', 6000);
  void openImportFor(d.file);
}

export const openExport = (profileId: string): void => { pf.exp = { profileId }; };
export const openDiagnostics = (profileId: string): void => { pf.diag = { profileId }; };
export const openCrash = (profileId: string, logId?: string): void => { pf.crash = logId ? { profileId, logId } : { profileId }; };

/** After a crash the game reports `crashed`: offer the analysis once, as a toast that stays long enough to read. */
export function offerCrash(profileId: string): void {
  if (pf.crash?.profileId === profileId) return;
  const name = app.profiles.find((p) => p.id === profileId)?.name ?? '';
  toast(t('crash.toast', { name }), 'error', 20000, { label: t('crash.toastAction'), run: () => openCrash(profileId) });
}

eventHooks.crashed.push(offerCrash);
eventHooks.dropped.push((paths) => handleDropped(paths));

/** Where the profile page shows the Content tab filtered to one mod (crash cause links). */
export function showModInContent(profileId: string, mod: string): void {
  nav.contentQuery = mod;
  openProfile(profileId, 'content');
}

/** True while a modal dialog is open: a drag over it belongs to that dialog (an icon or background picker has its own drop zone). */
const dialogOpen = (): boolean => typeof document !== 'undefined' && !!document.querySelector('dialog[open]');

/**
 * Window-wide drag hint. In the Tauri shell the webview swallows DOM file drops, so its drag-drop event (with paths) drives the overlay and
 * the real drop arrives as `files:dropped`. In a browser the standard HTML drag events do; only the browser mock turns a dropped `File` into
 * a path (the dev server cannot read the user's disk through a browser, so it ignores DOM drops).
 */
export function watchDrops(opts: { tauri: boolean; mock: boolean }): () => void {
  if (typeof window === 'undefined') return () => {};
  const files = (e: DragEvent): boolean => !!e.dataTransfer?.types?.includes('Files');
  const enter = (e: DragEvent) => { if (files(e) && !dialogOpen()) { e.preventDefault(); pf.dragging = true; } };
  const over = (e: DragEvent) => { if (files(e)) e.preventDefault(); };
  const leave = (e: DragEvent) => { if (e.relatedTarget === null) pf.dragging = false; };
  const drop = (e: DragEvent) => {
    if (!files(e)) return;
    e.preventDefault(); pf.dragging = false;
    if (opts.mock && !dialogOpen()) handleDropped([...(e.dataTransfer?.files ?? [])].map((f) => `/home/mango/Downloads/${f.name}`));
  };
  let off = (): void => {};
  let dead = false;
  if (opts.tauri) {
    void import('@tauri-apps/api/webview').then(({ getCurrentWebview }) => getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === 'enter') pf.dragging = !dialogOpen() && p.paths.some((x) => classifyDrop([x]).file !== null);
      else if (p.type === 'leave' || p.type === 'drop') pf.dragging = false;
    })).then((u) => { if (dead) u(); else off = u; }, () => {});
  } else {
    window.addEventListener('dragenter', enter); window.addEventListener('dragover', over); window.addEventListener('dragleave', leave); window.addEventListener('drop', drop);
  }
  return () => {
    dead = true; off();
    window.removeEventListener('dragenter', enter); window.removeEventListener('dragover', over); window.removeEventListener('dragleave', leave); window.removeEventListener('drop', drop);
  };
}
