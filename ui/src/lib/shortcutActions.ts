// Runs the global shortcuts (lib/keybinds.ts) against the stores, and installs the window listeners (keyboard + mouse side buttons).
import { tick } from 'svelte';
import { isTypingTarget, matchShortcut, mouseNav, type ShortcutAction } from './keybinds';
import { actions, app, getApi, goView, isBusy, nav, navBack, navForward, platform, selectedAccount, selectedProfile, toast, ui, t, type View } from './stores.svelte';
import { windowControls } from './titlebar';

const VIEW_OF: Partial<Record<ShortcutAction, View>> = {
  home: 'home', library: 'library', discover: 'discover', servers: 'servers', skins: 'skins', stats: 'stats', settings: 'settings',
};

/** The first visible search field of the current page (`data-search`), else null. */
export function findSearchField(root: ParentNode = document): HTMLInputElement | null {
  for (const el of root.querySelectorAll<HTMLInputElement>('input[data-search]')) {
    if (!el.disabled && el.offsetParent !== null) return el;
  }
  return null;
}

export async function focusSearch(): Promise<void> {
  let el = findSearchField();
  if (!el && nav.view !== 'library' && nav.view !== 'discover' && nav.view !== 'settings' && nav.view !== 'profile') {
    nav.view = 'library'; // no search here: Library is where "find an instance" lives
    await tick(); await new Promise((r) => setTimeout(r, 30));
    el = findSearchField();
  }
  el?.focus(); el?.select();
}

export function playStop(): void {
  const p = selectedProfile();
  if (!p || !selectedAccount()) { if (p) toast(t('play.noAccount'), 'info'); return; }
  if (isBusy(p.id)) { ui.confirmStop = p.id; return; }
  void actions.selectProfile(p.id);
  void actions.play(p.id);
}

/** F2: rename the open instance (inline on its page) or the selected one (dialog). */
export function renameCurrent(): void {
  if (!app.ready) return;
  if (nav.view === 'profile' && nav.profileId) { ui.renameInline = nav.profileId; return; }
  const p = selectedProfile();
  if (p) ui.rename = p.id;
}

export function runShortcut(a: ShortcutAction): void {
  const v = VIEW_OF[a];
  if (v) { goView(v); return; }
  switch (a) {
    case 'newProfile': if (app.ready) nav.newProfile = true; break;
    case 'focusSearch': void focusSearch(); break;
    case 'playStop': playStop(); break;
    case 'back': navBack(); break;
    case 'forward': navForward(); break;
    case 'shortcuts': ui.shortcuts = !ui.shortcuts; break;
    case 'rename': renameCurrent(); break;
    case 'maximize': void windowControls(getApi()).toggleMaximize(); break;
  }
}

const modalOpen = (): boolean => !!document.querySelector('dialog[open]');

/** Installs the listeners; returns the remover. */
export function installShortcuts(win: Window = window): () => void {
  const onKey = (e: KeyboardEvent) => {
    const a = matchShortcut(e, { mac: platform.mac, typing: isTypingTarget(e.target), modalOpen: modalOpen() });
    if (!a) return;
    if (a === 'maximize' && platform.mode === 'browser') return; // the browser's own F11
    e.preventDefault(); e.stopPropagation();
    runShortcut(a);
  };
  const onMouse = (e: MouseEvent) => {
    const n = mouseNav(e.button);
    if (!n) return;
    e.preventDefault();
    if (e.type === 'mouseup') runShortcut(n);
  };
  win.addEventListener('keydown', onKey, true);
  win.addEventListener('mousedown', onMouse, true);
  win.addEventListener('mouseup', onMouse, true);
  return () => { win.removeEventListener('keydown', onKey, true); win.removeEventListener('mousedown', onMouse, true); win.removeEventListener('mouseup', onMouse, true); };
}
