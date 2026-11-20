// The instance context menu (Library cards and rows, Home cards and hero, rail tiles, the profile page "..." button): one definition for all of them.
import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon';
import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
import ImageIcon from 'phosphor-svelte/lib/ImageIcon';
import PackageIcon from 'phosphor-svelte/lib/PackageIcon';
import PaletteIcon from 'phosphor-svelte/lib/PaletteIcon';
import PencilSimpleIcon from 'phosphor-svelte/lib/PencilSimpleIcon';
import PlayIcon from 'phosphor-svelte/lib/PlayIcon';
import StopIcon from 'phosphor-svelte/lib/StopIcon';
import TagIcon from 'phosphor-svelte/lib/TagIcon';
import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
import BugIcon from 'phosphor-svelte/lib/BugIcon';
import FirstAidKitIcon from 'phosphor-svelte/lib/FirstAidKitIcon';
import { SEP, type MenuItem } from './contextMenu.svelte';
import { openCrash, openDiagnostics, openExport } from './portability.svelte';
import { actions, app, errorText, getApi, isBusy, openBackgroundPicker, openIconPicker, openProfile, platform, selectedAccount, t, toast, ui } from './stores.svelte';
import type { ProfileView } from './types';

const fail = (e: unknown) => toast(errorText(e), 'error');
const mod = (): string => (platform.mac ? '⌘' : 'Ctrl');

/* ---- instances ---- */
export interface InstanceMenuOpts {
  /** The menu opens on this instance's own page: "Open" is left out. */
  onPage?: boolean;
}
export function instanceMenu(p: ProfileView, o: InstanceMenuOpts = {}): MenuItem[] {
  const busy = isBusy(p.id);
  const noAccount = !selectedAccount() && !app.accounts.length;
  return [
    busy
      ? { id: 'stop', label: t('home.stop'), icon: StopIcon, shortcut: `${mod()}+Shift+P`, run: () => void actions.stop(p.id) }
      : { id: 'play', label: t('home.play'), icon: PlayIcon, disabled: noAccount, reason: t('play.noAccount'), run: () => { void actions.selectProfile(p.id); void actions.play(p.id); } },
    { id: 'open', label: t('menu.open'), icon: ArrowSquareOutIcon, shortcut: 'Enter', hidden: !!o.onPage, run: () => openProfile(p.id) },
    SEP,
    { id: 'rename', label: t('menu.rename'), icon: PencilSimpleIcon, shortcut: 'F2', run: () => { if (o.onPage) ui.renameInline = p.id; else ui.rename = p.id; } },
    { id: 'icon', label: t('icons.change'), icon: PaletteIcon, run: () => openIconPicker(p.id) },
    { id: 'bg', label: t('bg.change'), icon: ImageIcon, run: () => openBackgroundPicker(p.id) },
    { id: 'group', label: t('library.menu.group'), icon: TagIcon, run: () => openProfile(p.id, 'settings') },
    SEP,
    { id: 'dup', label: t('profile.duplicate'), icon: CopyIcon, run: () => void duplicateInstance(p) },
    { id: 'folder', label: t('menu.openFolder'), icon: FolderOpenIcon, run: () => void getApi().openFolder({ target: p.id }).catch(fail) },
    { id: 'export', label: t('export.menu'), icon: PackageIcon, run: () => openExport(p.id) },
    SEP,
    { id: 'delete', label: t('profile.delete'), icon: TrashIcon, danger: true, disabled: busy, reason: t('menu.deleteRunning'), run: () => { ui.confirmDelete = p.id; } },
  ];
}
export async function duplicateInstance(p: ProfileView): Promise<void> {
  try {
    const c = await getApi().profileDuplicate({ id: p.id });
    app.profiles = [...app.profiles.filter((x) => x.id !== c.id), c];
    toast(t('menu.duplicated', { name: c.name }), 'ok');
  } catch (e) { fail(e); }
}


/** The menu of the instance's own page: the shared items without "Open", plus the crash analysis and the diagnostics export. */
export function pageMenu(p: ProfileView): MenuItem[] {
  const base = instanceMenu(p, { onPage: true });
  const at = base.findIndex((i) => i.id === 'delete') - 1; // before the separator above Delete
  return [
    ...base.slice(0, at),
    SEP,
    { id: 'crash', label: t('crash.title'), icon: BugIcon, run: () => openCrash(p.id) },
    { id: 'diag', label: t('diag.button'), icon: FirstAidKitIcon, run: () => openDiagnostics(p.id) },
    ...base.slice(at),
  ];
}
