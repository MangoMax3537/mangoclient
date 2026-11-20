// Which tabs an instance page shows. A tab that does not apply is not rendered: mods need a loader (an instance that has mods anyway keeps
// the tab), data packs only appear when the instance has some. Shaders and resource packs always apply.
import type { ProfileTab } from './stores.svelte';
import type { ContentType, ProfileView } from './types';

export const CONTENT_TAB_TYPE = { mods: 'mod', shaders: 'shader', resourcepacks: 'resourcepack', datapacks: 'datapack' } as const satisfies Partial<Record<ProfileTab, ContentType>>;
export type ContentTab = keyof typeof CONTENT_TAB_TYPE;
export const isContentTab = (t: ProfileTab): t is ContentTab => t in CONTENT_TAB_TYPE;

export function profileTabs(loader: ProfileView['loader'], counts: Record<ContentType, number>): ProfileTab[] {
  const tabs: ProfileTab[] = ['overview'];
  if (loader !== 'vanilla' || counts.mod > 0) tabs.push('mods');
  tabs.push('shaders', 'resourcepacks');
  if (counts.datapack > 0) tabs.push('datapacks');
  tabs.push('media', 'backups', 'logs', 'settings');
  return tabs;
}
/** The tab to show: the wanted one when it is rendered, else Overview. While the content list is still loading a content tab is trusted. */
export function effectiveTab(want: ProfileTab, tabs: readonly ProfileTab[], loaded: boolean): ProfileTab {
  if (tabs.includes(want)) return want;
  return !loaded && isContentTab(want) ? want : 'overview';
}
