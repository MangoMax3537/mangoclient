// Menu definitions for every surface that has a context menu (and the three-dots buttons that open the same menu). Each builder returns plain
// `MenuItem[]`; the menu (components/ContextMenu.svelte) renders them. Nothing here touches the DOM except the clipboard.
import ArrowSquareOutIcon from 'phosphor-svelte/lib/ArrowSquareOutIcon';
import ArrowCircleUpIcon from 'phosphor-svelte/lib/ArrowCircleUpIcon';
import ArrowCounterClockwiseIcon from 'phosphor-svelte/lib/ArrowCounterClockwiseIcon';
import ArrowsClockwiseIcon from 'phosphor-svelte/lib/ArrowsClockwiseIcon';
import BookmarkSimpleIcon from 'phosphor-svelte/lib/BookmarkSimpleIcon';
import CopyIcon from 'phosphor-svelte/lib/CopyIcon';
import FloppyDiskIcon from 'phosphor-svelte/lib/FloppyDiskIcon';
import FolderOpenIcon from 'phosphor-svelte/lib/FolderOpenIcon';
import GlobeIcon from 'phosphor-svelte/lib/GlobeIcon';
import PowerIcon from 'phosphor-svelte/lib/PowerIcon';
import SignInIcon from 'phosphor-svelte/lib/SignInIcon';
import TrashIcon from 'phosphor-svelte/lib/TrashIcon';
import UserSwitchIcon from 'phosphor-svelte/lib/UserSwitchIcon';
import { SEP, type MenuItem } from './contextMenu.svelte';
import { copyText } from './clipboard';
import { errorText, getApi, t, toast } from './stores.svelte';
import type { Backup, ContentItem, LibrarySkin, Screenshot, ServerEntry, World } from './types';

const fail = (e: unknown) => toast(errorText(e), 'error');

/* ---- content rows (mods, shaders, resource packs, data packs) ---- */
export interface ContentMenuOps {
  toggle: (it: ContentItem, on: boolean) => void;
  update: (it: ContentItem) => void;
  remove: (it: ContentItem) => void;
}
const MODRINTH = /^[A-Za-z0-9_-]{1,64}$/;
/** The project page on Modrinth, or null for a local file (it has no project). */
export function projectUrl(it: ContentItem): string | null {
  const slug = it.slug ?? it.projectId;
  if (it.local || !MODRINTH.test(slug)) return null;
  const seg = it.type === 'resourcepack' ? 'resourcepack' : it.type === 'datapack' ? 'datapack' : it.type === 'shader' ? 'shader' : 'mod';
  return `https://modrinth.com/${seg}/${slug}`;
}
export function contentMenu(it: ContentItem, ops: ContentMenuOps): MenuItem[] {
  const url = projectUrl(it);
  const blockedOff = it.enabled && it.requiredBy.length > 0;
  return [
    { id: 'toggle', label: it.enabled ? t('menu.disable') : t('menu.enable'), icon: PowerIcon, disabled: blockedOff, reason: t('menu.requiredReason'), run: () => ops.toggle(it, !it.enabled) },
    { id: 'update', label: t('menu.updateTo', { v: it.updateAvailable ?? '' }), icon: ArrowCircleUpIcon, hidden: !it.updateAvailable, disabled: it.local, reason: t('menu.localNoPage'), run: () => ops.update(it) },
    { id: 'page', label: t('menu.projectPage'), icon: GlobeIcon, hidden: !url, run: () => { if (url) void getApi().openExternal({ url }).catch(fail); } },
    { id: 'copy', label: t('menu.copyName'), icon: CopyIcon, run: () => void copyText(it.title) },
    SEP,
    { id: 'remove', label: t('content.uninstall'), icon: TrashIcon, danger: true, run: () => ops.remove(it) },
  ];
}

/* ---- worlds and backups ---- */
export interface WorldMenuOps { backup: (w: World) => void; reveal: () => void; remove: (w: World) => void; busy: boolean; running: boolean }
export function worldMenu(w: World, o: WorldMenuOps): MenuItem[] {
  return [
    { id: 'backup', label: t('worlds.backup'), icon: FloppyDiskIcon, disabled: o.busy, reason: t('menu.busyReason'), run: () => o.backup(w) },
    { id: 'folder', label: t('worlds.openFolder'), icon: FolderOpenIcon, run: o.reveal },
    { id: 'copy', label: t('menu.copyName'), icon: CopyIcon, run: () => void copyText(w.name) },
    SEP,
    { id: 'delete', label: t('worlds.delete'), icon: TrashIcon, danger: true, disabled: o.running || o.busy, reason: o.running ? t('menu.gameRunningReason') : t('menu.busyReason'), run: () => o.remove(w) },
  ];
}
export interface BackupMenuOps { restore: (b: Backup) => void; remove: (b: Backup) => void; busy: boolean; running: boolean }
export function backupMenu(b: Backup, o: BackupMenuOps): MenuItem[] {
  return [
    { id: 'restore', label: t('backups.restore'), icon: ArrowCounterClockwiseIcon, disabled: o.running || o.busy, reason: o.running ? t('menu.gameRunningReason') : t('menu.busyReason'), run: () => o.restore(b) },
    SEP,
    { id: 'delete', label: t('backups.delete'), icon: TrashIcon, danger: true, disabled: o.busy, reason: t('menu.busyReason'), run: () => o.remove(b) },
  ];
}

/* ---- servers ---- */
export interface ServerMenuOps { join: (s: ServerEntry) => void; ping: (s: ServerEntry) => void; canJoin: boolean }
export function serverMenu(s: ServerEntry, o: ServerMenuOps): MenuItem[] {
  return [
    { id: 'join', label: t('servers.join'), icon: SignInIcon, disabled: !o.canJoin, reason: t('play.noAccount'), run: () => o.join(s) },
    { id: 'copy', label: t('menu.copyAddress'), icon: CopyIcon, run: () => void copyText(s.address) },
    { id: 'ping', label: t('menu.pingNow'), icon: ArrowsClockwiseIcon, run: () => o.ping(s) },
  ];
}

/* ---- skins ---- */
export interface SkinMenuOps { apply: (s: LibrarySkin) => void; remove: (s: LibrarySkin) => void; busy: boolean }
export function librarySkinMenu(s: LibrarySkin, o: SkinMenuOps): MenuItem[] {
  return [
    { id: 'apply', label: t('skins.applyShort'), icon: UserSwitchIcon, disabled: o.busy, reason: t('menu.busyReason'), run: () => o.apply(s) },
    { id: 'copy', label: t('menu.copyName'), icon: CopyIcon, run: () => void copyText(s.name) },
    SEP,
    { id: 'remove', label: t('skins.remove'), icon: TrashIcon, danger: true, disabled: o.busy, reason: t('menu.busyReason'), run: () => o.remove(s) },
  ];
}
/** The skin box: save what is shown into the library. */
export const skinStageMenu = (save: () => void, editable: boolean): MenuItem[] => [
  { id: 'save', label: t('skins.save'), icon: BookmarkSimpleIcon, disabled: !editable, reason: t('skins.offlineNote'), run: save },
];

/* ---- screenshots ---- */
export interface ShotMenuOps { open: () => void; act: (a: 'copy' | 'reveal' | 'delete') => void }
export function screenshotMenu(s: Screenshot, o: ShotMenuOps): MenuItem[] {
  return [
    { id: 'open', label: t('menu.open'), icon: ArrowSquareOutIcon, run: o.open },
    { id: 'copy', label: t('media.copy'), icon: CopyIcon, run: () => o.act('copy') },
    { id: 'reveal', label: t('media.reveal'), icon: FolderOpenIcon, run: () => o.act('reveal') },
    { id: 'name', label: t('menu.copyName'), icon: CopyIcon, run: () => void copyText(s.name) },
    SEP,
    { id: 'delete', label: t('media.delete'), icon: TrashIcon, danger: true, run: () => o.act('delete') },
  ];
}
