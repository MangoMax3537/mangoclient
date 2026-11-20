// Code splitting helpers. A loader is `() => import('./X.svelte')`; `memo` shares one in-flight/finished promise and retries after a failure
// (a flaky chunk fetch must not poison the screen until restart).
import type { Component } from 'svelte';

export type ComponentModule = { default: Component<any> }; // eslint-disable-line @typescript-eslint/no-explicit-any
export type Loader = () => Promise<ComponentModule>;

export function memo(load: Loader): Loader {
  let p: Promise<ComponentModule> | null = null;
  return () => {
    if (!p) p = load().catch((e) => { p = null; throw e; });
    return p;
  };
}

/**
 * Every heavy screen and dialog, loaded on first use. Keep static imports of these files out of the entry graph (a test enforces it):
 * the initial bundle only holds the shell, Home, Library and the profile page frame.
 */
export const screens = {
  discover: memo(() => import('../components/Discover.svelte')),
  servers: memo(() => import('../components/Servers.svelte')),
  skins: memo(() => import('../components/Skins.svelte')),
  stats: memo(() => import('../components/Stats.svelte')),
  settings: memo(() => import('../components/Settings.svelte')),
  onboarding: memo(() => import('../components/Onboarding.svelte')),
  newProfile: memo(() => import('../components/NewProfile.svelte')),
} satisfies Record<string, Loader>;

export const dialogs = {
  installFlow: memo(() => import('../components/InstallFlow.svelte')),
  iconPicker: memo(() => import('../components/IconPicker.svelte')),
  backgroundPicker: memo(() => import('../components/BackgroundPicker.svelte')),
  importDialog: memo(() => import('../components/ImportDialog.svelte')),
  exportDialog: memo(() => import('../components/ExportDialog.svelte')),
  diagnosticsDialog: memo(() => import('../components/DiagnosticsDialog.svelte')),
  crashPanel: memo(() => import('../components/CrashPanel.svelte')),
  projectDrawer: memo(() => import('../components/ProjectDrawer.svelte')),
  instancePicker: memo(() => import('../components/InstancePicker.svelte')),
  dependencyDialog: memo(() => import('../components/DependencyDialog.svelte')),
  modpackDialog: memo(() => import('../components/ModpackDialog.svelte')),
  updatePreview: memo(() => import('../components/UpdatePreview.svelte')),
  runDialog: memo(() => import('../components/RunDialog.svelte')),
  reloginDialog: memo(() => import('../components/ReloginDialog.svelte')),
  updateOverlay: memo(() => import('../components/UpdateOverlay.svelte')),
  shortcutSheet: memo(() => import('../components/ShortcutSheet.svelte')),
} satisfies Record<string, Loader>;

export const tabs = {
  content: memo(() => import('../components/profile/ContentTab.svelte')),
  media: memo(() => import('../components/profile/Media.svelte')),
  backups: memo(() => import('../components/profile/Backups.svelte')),
  logs: memo(() => import('../components/profile/Logs.svelte')),
  settings: memo(() => import('../components/profile/ProfileSettings.svelte')),
} satisfies Record<string, Loader>;

/** Skeleton layouts: only needed once a load takes longer than ~150 ms, so they stay out of the entry bundle. */
export const skeletons = {
  view: memo(() => import('../components/ViewSkeleton.svelte')),
  profileTab: memo(() => import('../components/skeleton/ProfileTabSkeleton.svelte')),
} satisfies Record<string, Loader>;
