import { describe, expect, it } from 'vitest';
import { effectiveTab, isContentTab, profileTabs } from './profileTabs';
import { tabForType } from './stores.svelte';
import { dictionaries } from './i18n';

const none = { mod: 0, shader: 0, resourcepack: 0, datapack: 0 };

describe('instance tabs', () => {
  it('the content tabs are separate: Mods, Shaders, Resource packs', () => {
    expect(profileTabs('fabric', none)).toEqual(['overview', 'mods', 'shaders', 'resourcepacks', 'media', 'backups', 'logs', 'settings']);
    expect(profileTabs('fabric', none)).not.toContain('content' as never);
  });
  it('data packs get a tab only when the instance has some', () => {
    expect(profileTabs('fabric', none)).not.toContain('datapacks');
    expect(profileTabs('fabric', { ...none, datapack: 2 })).toContain('datapacks');
  });
  it('a vanilla instance has no Mods tab unless it holds mods anyway', () => {
    expect(profileTabs('vanilla', none)).not.toContain('mods');
    expect(profileTabs('vanilla', { ...none, mod: 1 })).toContain('mods');
  });
  it('a tab that is not rendered falls back to Overview (but a content tab is trusted while the list loads)', () => {
    const tabs = profileTabs('vanilla', none);
    expect(effectiveTab('datapacks', tabs, true)).toBe('overview');
    expect(effectiveTab('mods', tabs, true)).toBe('overview');
    expect(effectiveTab('mods', tabs, false)).toBe('mods');
    expect(effectiveTab('settings', tabs, true)).toBe('settings');
  });
  it('Discover scope and the "Add" buttons map type <-> tab', () => {
    expect(tabForType('mod')).toBe('mods');
    expect(tabForType('shader')).toBe('shaders');
    expect(tabForType('resourcepack')).toBe('resourcepacks');
    expect(tabForType('datapack')).toBe('datapacks');
    expect(isContentTab('mods')).toBe(true);
    expect(isContentTab('media')).toBe(false);
  });
  it('every tab and every per-type string exists in both languages', () => {
    const tabs = profileTabs('fabric', { ...none, datapack: 1 });
    for (const t of tabs) { expect(dictionaries.en[`profile.tab.${t}`], t).toBeTruthy(); expect(dictionaries.de[`profile.tab.${t}`], t).toBeTruthy(); }
    for (const type of ['mod', 'shader', 'resourcepack', 'datapack']) {
      for (const base of ['content.search', 'content.add', 'content.emptyTitle', 'content.empty', 'content.type']) {
        expect(dictionaries.en[`${base}.${type}`], `${base}.${type}`).toBeTruthy();
        expect(dictionaries.de[`${base}.${type}`], `${base}.${type}`).toBeTruthy();
      }
      expect(dictionaries.en[`content.updatesAvailable.${type}.other`]).toBeTruthy();
      expect(dictionaries.de[`content.updatesAvailable.${type}.one`]).toBeTruthy();
      expect(dictionaries.en[`discover.type.${type}`]).toBeTruthy();
    }
  });
});
