// Static catalogue behind mockApi (dev/demo + tests). Pure data, no timers.
import type { ContentType, ProjectType } from './types';

/** [id, title, author, description, categories] */
export type Row = [string, string, string, string, string];

export const MODS: Row[] = [
  ['sodium', 'Sodium', 'jellysquid3', 'A modern rendering engine and optimization mod for Minecraft.', 'optimization'],
  ['lithium', 'Lithium', 'jellysquid3', 'Game logic and server optimizations without changing behaviour.', 'optimization'],
  ['fabric-api', 'Fabric API', 'modmuss50', 'Core hooks and interoperability for Fabric mods.', 'library'],
  ['iris', 'Iris Shaders', 'coderbot', 'A shader pack loader that works with Sodium.', 'decoration,optimization'],
  ['ferrite-core', 'FerriteCore', 'malte0811', 'Memory usage optimizations.', 'optimization'],
  ['modmenu', 'Mod Menu', 'Prospector', 'Adds a screen to view the mods you have installed.', 'utility'],
  ['cloth-config', 'Cloth Config API', 'shedaniel', 'Configuration screens for mods.', 'library'],
  ['xaeros-minimap', "Xaero's Minimap", 'xaero96', 'A minimap with waypoints and entity radar.', 'utility'],
  ['xaeros-worldmap', "Xaero's World Map", 'xaero96', 'A full-screen world map that records what you explore.', 'utility'],
  ['entityculling', 'Entity Culling', 'tr7zw', 'Skips rendering of entities that are not visible.', 'optimization'],
  ['dynamic-fps', 'Dynamic FPS', 'juliand665', 'Reduces resource usage while the window is in the background.', 'optimization'],
  ['sodium-extra', 'Sodium Extra', 'FlashyReese', 'Extra options and features for Sodium.', 'optimization'],
  ['reeses-sodium-options', "Reese's Sodium Options", 'FlashyReese', 'A modern video settings screen for Sodium.', 'optimization,utility'],
  ['immediatelyfast', 'ImmediatelyFast', 'RaphiMC', 'Speeds up immediate mode rendering of text and GUIs.', 'optimization'],
  ['moreculling', 'More Culling', 'FX', 'Culls more faces and blocks that are never seen.', 'optimization'],
  ['continuity', 'Continuity', 'PepperCode1', 'Connected textures for resource packs.', 'decoration'],
  ['appleskin', 'AppleSkin', 'squeek502', 'Shows hunger and saturation values in the HUD.', 'utility'],
  ['jade', 'Jade', 'Snownee', 'Shows what you are looking at, with details.', 'utility'],
  ['mouse-tweaks', 'Mouse Tweaks', 'YaLTeR', 'Faster inventory management with the mouse.', 'utility'],
  ['no-chat-reports', 'No Chat Reports', 'Aizistral', 'Disables chat signing and reporting.', 'utility'],
  ['litematica', 'Litematica', 'masa', 'Schematic mod with a build overlay.', 'utility'],
  ['malilib', 'MaLiLib', 'masa', 'Library for masa mods.', 'library'],
  ['architectury', 'Architectury API', 'shedaniel', 'Abstraction layer for multi-loader mods.', 'library'],
  ['rei', 'Roughly Enough Items', 'shedaniel', 'Recipe viewer and item browser.', 'utility'],
  ['zoomify', 'Zoomify', 'isXander', 'A configurable zoom with smooth transitions.', 'utility'],
  ['fabric-language-kotlin', 'Fabric Language Kotlin', 'Fabric', 'Kotlin language adapter for Fabric.', 'library'],
  ['yacl', 'YetAnotherConfigLib', 'isXander', 'A configuration library with a modern GUI.', 'library'],
  ['capes', 'Capes', 'CapesMod', 'Cape support for Minecraft Capes, OptiFine and more.', 'cosmetic'],
  ['sound-physics-remastered', 'Sound Physics Remastered', 'henkelmax', 'Realistic reverb and sound occlusion.', 'decoration'],
  ['lambdynamiclights', 'LambDynamicLights', 'LambdAurora', 'Held items emit light.', 'decoration'],
  ['inventory-profiles-next', 'Inventory Profiles Next', 'blackd', 'Sort and move items with one click.', 'utility'],
  ['modernfix', 'ModernFix', 'embeddedt', 'Performance and bug fixes for startup and memory.', 'optimization'],
  ['distanthorizons', 'Distant Horizons', 'jeseibel', 'Render far beyond the vanilla view distance.', 'optimization'],
  ['better-stats', 'Better Statistics Screen', 'TeamMidnightDust', 'A clearer statistics screen.', 'utility'],
  ['not-enough-animations', 'Not Enough Animations', 'Tr7zw', 'Animates the third-person player model.', 'cosmetic'],
  ['cull-leaves', 'Cull Leaves', 'Motschen', 'Hides inner leaf faces for fewer draw calls.', 'optimization'],
  ['enhanced-block-entities', 'Enhanced Block Entities', 'FoundationGames', 'Faster rendering of chests, signs and beds.', 'optimization'],
  ['carpet', 'Carpet', 'gnembon', 'Technical tweaks and tools for redstone engineers.', 'technology'],
  ['create-fabric', 'Create Fabric', 'Fabric Port', 'Aesthetic technology that empowers the player.', 'technology'],
  ['ftb-quests', 'FTB Quests', 'FTB', 'Quest book with teams and rewards.', 'adventure'],
  ['farmers-delight', "Farmer's Delight", 'vectorwing', 'Cooking and farming with simple new blocks.', 'food'],
  ['terrablender', 'TerraBlender', 'Glitchfiend', 'Biome registration for modded worlds.', 'library'],
];

export const MODPACKS: Row[] = [
  ['fabulously-optimized', 'Fabulously Optimized', 'robotkoer', 'Performance mods tuned for vanilla visuals.', 'optimization'],
  ['better-mc-fabric', 'Better MC [FABRIC]', 'LunaPixelStudios', 'Vanilla-plus with new structures, mobs and quality of life.', 'adventure'],
  ['cobblemon-fabric', 'Cobblemon Pack', 'Cobblemon', 'Catch and battle creatures in a curated pack.', 'adventure'],
  ['simply-optimized', 'Simply Optimized', 'BorisMN', 'A small pack that makes the game run better.', 'optimization'],
  ['prominence-ii', 'Prominence II', 'Artdude543', 'Hardcore RPG questing pack.', 'adventure'],
  ['all-the-mods-fabric', 'Create: Above and Beyond', 'simibubi', 'A Create progression pack.', 'technology'],
  ['skyblock-classic', 'Skyblock Classic', 'Tecchen', 'Survive on a single tree.', 'challenging'],
  ['vanilla-plus-qol', 'Vanilla Plus QoL', 'Hobo', 'Quality of life only, nothing that changes gameplay.', 'lightweight'],
  ['pvp-essentials', 'PvP Essentials', 'MangoMax', 'Low-latency mods for competitive play.', 'optimization'],
  ['explorers-atlas', "Explorer's Atlas", 'Wanderer', 'Maps, biomes and structures to find.', 'adventure'],
];

export const PACKS: Row[] = [
  ['faithful-32x', 'Faithful 32x', 'Faithful Team', 'Vanilla textures at double resolution.', 'vanilla-like'],
  ['fresh-animations', 'Fresh Animations', 'FreshLX', 'Animated mobs with a vanilla style.', 'modded'],
  ['default-dark-mode', 'Default Dark Mode', 'Pomo', 'A dark theme for the interface.', 'themed'],
  ['bare-bones', 'Bare Bones', 'Bare Bones Team', 'Flat colours and clean outlines.', 'simplistic'],
  ['vanilla-tweaks', 'Vanilla Tweaks', 'Vanilla Tweaks', 'Pick small improvements and merge them.', 'utility'],
  ['stay-true', 'Stay True', 'Stay True', 'Keeps the vanilla look at higher fidelity.', 'vanilla-like'],
  ['modern-arch', 'Modern Arch', 'Arch', 'A 64x pack for builders.', 'realistic'],
  ['soartex-fanver', 'Soartex Fanver', 'Soartex', 'A 32x pack with lively colours.', 'cartoon'],
  ['xalis-enchanted-books', "Xali's Enchanted Books", 'Xali', 'Readable enchanted book icons.', 'utility'],
  ['low-fire', 'Low Fire', 'Snaps', 'Shorter fire overlay.', 'pvp'],
  ['clear-glass', 'Clear Glass', 'Aero', 'Glass without borders.', 'simplistic'],
  ['tiny-ui', 'Tiny UI', 'Mini', 'Compact HUD textures.', 'utility'],
];

export const SHADERS: Row[] = [
  ['complementary-reimagined', 'Complementary Shaders - Reimagined', 'EminGT', 'Vanilla-friendly lighting with great performance.', 'vanilla-like'],
  ['bsl-shaders', 'BSL Shaders', 'capttatsu', 'Realistic lighting, water and clouds.', 'realistic'],
  ['complementary-unbound', 'Complementary Shaders - Unbound', 'EminGT', 'The high-fidelity sibling of Reimagined.', 'realistic'],
  ['photon', 'Photon', 'sixthsurge', 'Fast and good-looking.', 'semi-realistic'],
  ['solas', 'Solas Shader', 'Septonious', 'Soft colours and a clean look.', 'cartoon'],
  ['rethinking-voxels', 'Rethinking Voxels', 'gri573', 'Voxel-based ray tracing.', 'path-tracing'],
  ['miniature', 'Miniature Shader', 'Onnowhere', 'Tilt shift for a tabletop feel.', 'fantasy'],
  ['sildurs-vibrant', "Sildur's Vibrant", 'Sildur', 'The classic with many profiles.', 'vanilla-like'],
  ['makeup-ultra-fast', 'MakeUp - Ultra Fast', 'javiergcm', 'Built for weak hardware.', 'potato'],
  ['kappa', 'Kappa Shader', 'Kappa', 'Cinematic bloom and depth.', 'cinematic'],
  ['nostalgia', 'Nostalgia Shader', 'Nos', 'Old-school lighting.', 'vanilla-like'],
  ['euphoria-patches', 'Euphoria Patches', 'SpacEagle', 'Extra options on top of Complementary.', 'semi-realistic'],
];

export const TABLES: Record<ProjectType, Row[]> = {
  mod: MODS, modpack: MODPACKS, resourcepack: PACKS, shader: SHADERS, datapack: [],
};

/** Installed-content metadata for the seeded profiles: version, hard dependencies, auto-installed flag, pending update. */
export interface ContentMeta { ver: string; deps?: string[]; dep?: boolean; upd?: string; type?: ContentType; local?: boolean }
export const CONTENT: Record<string, ContentMeta> = {
  'fabric-api': { ver: '0.119.4+1.21.4', dep: true },
  sodium: { ver: '0.6.9', upd: '0.6.13' },
  lithium: { ver: '0.14.3' },
  iris: { ver: '1.8.8', deps: ['sodium'], upd: '1.8.9' },
  'ferrite-core': { ver: '7.1.1' },
  modmenu: { ver: '12.0.0', upd: '13.0.0' },
  'cloth-config': { ver: '17.0.144', dep: true },
  'xaeros-minimap': { ver: '25.1.0', upd: '25.2.4' },
  'xaeros-worldmap': { ver: '1.39.2' },
  entityculling: { ver: '1.7.4' },
  'dynamic-fps': { ver: '3.9.2' },
  'sodium-extra': { ver: '0.6.1', deps: ['sodium'] },
  'reeses-sodium-options': { ver: '1.8.1', deps: ['sodium'] },
  immediatelyfast: { ver: '1.5.2' },
  moreculling: { ver: '1.0.1' },
  continuity: { ver: '3.0.0' },
  appleskin: { ver: '3.0.5' },
  jade: { ver: '15.8.1' },
  'mouse-tweaks': { ver: '2.26.1' },
  'no-chat-reports': { ver: '2.11.0' },
  litematica: { ver: '0.21.1', deps: ['malilib'] },
  malilib: { ver: '0.23.0', dep: true },
  architectury: { ver: '15.0.1', dep: true },
  rei: { ver: '17.0.806', deps: ['cloth-config', 'architectury'] },
  zoomify: { ver: '2.14.2', deps: ['fabric-language-kotlin', 'yacl'] },
  'fabric-language-kotlin': { ver: '1.13.1+kotlin.2.1.10', dep: true },
  yacl: { ver: '3.6.2', dep: true },
  capes: { ver: '1.5.6' },
  'sound-physics-remastered': { ver: '1.4.15' },
  lambdynamiclights: { ver: '4.1.4' },
  'inventory-profiles-next': { ver: '2.1.0', deps: ['fabric-language-kotlin'] },
  modernfix: { ver: '5.20.4' },
  distanthorizons: { ver: '2.3.0' },
  // 33 mods above; then:
  'complementary-reimagined': { ver: 'r5.4.1', type: 'shader', upd: 'r5.4.2' },
  'bsl-shaders': { ver: '8.2.09', type: 'shader' },
  'faithful-32x': { ver: '1.21.4', type: 'resourcepack' },
  'fresh-animations': { ver: '1.9.2', type: 'resourcepack' },
  'default-dark-mode': { ver: '1.21.4', type: 'resourcepack' },
};
/** Install order for the full 38-item profile. */
export const FULL_PROFILE = Object.keys(CONTENT);

export const DEPS: Record<string, string[]> = {
  ...Object.fromEntries(Object.entries(CONTENT).map(([k, v]) => [k, v.deps ?? []])),
  'create-fabric': ['fabric-api'], 'ftb-quests': ['architectury'], 'farmers-delight': [], carpet: [],
};

export function titleOf(id: string): string {
  for (const t of Object.values(TABLES)) for (const r of t) if (r[0] === id) return r[1];
  return id;
}
export function rowOf(id: string): { row: Row; type: ProjectType; index: number } | null {
  for (const [type, t] of Object.entries(TABLES) as [ProjectType, Row[]][]) {
    const index = t.findIndex((r) => r[0] === id);
    if (index >= 0) return { row: t[index], type, index };
  }
  return null;
}

/** mulberry32: deterministic pseudo random for stable demo data. */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
