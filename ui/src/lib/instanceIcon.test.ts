import { buildPool } from './bgCycle';
import { describe, expect, it } from 'vitest';
import {
  BRAND_ICON_ID, PANORAMA_BLUR, choiceFromIcon, sameChoice, backgroundStyle, blockErrorKey, blockForSeed, choiceFromBackground, panoramaAngle, panoramaDelay, pickerState, previewBackground, resolveBackground,
  resolveInstanceIcon, sameBackground, type BackgroundContext,
} from './instanceIcon';
import { buildWallpapers } from './wallpapers';
import type { BlockIcon, BlockIconsStatus, PanoramaSet } from './types';

const face = 'x.png';
const blk = (id: string): BlockIcon => ({ id, label: id, faces: { top: face, left: face, right: face } });
const ready: BlockIconsStatus = { state: 'ready', mcVersion: '1.21.4', blocks: [blk('dirt'), blk('tnt'), blk('oak_log')] };
const missing: BlockIconsStatus = { state: 'missing', mcVersion: null, blocks: [] };

describe('the MangoClient icon as an explicit choice', () => {
  it('resolves to the brand icon even with a ready gallery and an auto block', () => {
    const r = resolveInstanceIcon({ coverUrl: null, icon: { kind: 'block', block: BRAND_ICON_ID }, autoBlock: 'dirt' }, ready);
    expect(r).toEqual({ kind: 'brand' });
    expect(BRAND_ICON_ID).toBe('mangoclient');
  });
  it('round-trips through the picker choice and sameChoice', () => {
    const c = choiceFromIcon({ kind: 'block', block: BRAND_ICON_ID });
    expect(c).toEqual({ kind: 'block', block: 'mangoclient' });
    expect(sameChoice(c, { kind: 'block', block: 'mangoclient' })).toBe(true);
    expect(sameChoice(c, { kind: 'auto' })).toBe(false);
  });
});

describe('icon resolution order', () => {
  it('image icon wins over everything', () => {
    const r = resolveInstanceIcon({ coverUrl: 'c.png', icon: { kind: 'image' }, autoBlock: 'dirt' }, ready);
    expect(r).toEqual({ kind: 'image', src: 'c.png' });
  });
  it('a chosen block beats the auto block', () => {
    const r = resolveInstanceIcon({ coverUrl: null, icon: { kind: 'block', block: 'tnt' }, autoBlock: 'dirt' }, ready);
    expect(r.kind === 'block' && r.block.id).toBe('tnt');
    expect(r.kind === 'block' && r.auto).toBe(false);
  });
  it('a block choice with a leftover cover still shows the block', () => {
    const r = resolveInstanceIcon({ coverUrl: 'old.png', icon: { kind: 'block', block: 'tnt' }, autoBlock: null }, ready);
    expect(r.kind).toBe('block');
  });
  it('falls back to autoBlock when the gallery is ready', () => {
    const r = resolveInstanceIcon({ coverUrl: null, icon: null, autoBlock: 'oak_log' }, ready);
    expect(r.kind === 'block' && r.block.id).toBe('oak_log');
    expect(r.kind === 'block' && r.auto).toBe(true);
  });
  it('shows the MangoClient icon while the gallery is not ready, or for unknown blocks, or without a profile', () => {
    expect(resolveInstanceIcon({ coverUrl: null, icon: null, autoBlock: 'dirt' }, missing)).toEqual({ kind: 'brand' });
    expect(resolveInstanceIcon({ coverUrl: null, icon: { kind: 'block', block: 'nope' }, autoBlock: null }, ready)).toEqual({ kind: 'brand' });
    expect(resolveInstanceIcon({ coverUrl: null, icon: { kind: 'block', block: 'tnt' }, autoBlock: null }, null)).toEqual({ kind: 'brand' });
    expect(resolveInstanceIcon(null, ready)).toEqual({ kind: 'brand' });
  });
  it('image icon without a cover falls through', () => {
    expect(resolveInstanceIcon({ coverUrl: null, icon: { kind: 'image' }, autoBlock: 'dirt' }, ready).kind).toBe('block');
  });
  it('blockForSeed is deterministic and empty-safe', () => {
    expect(blockForSeed('a', ready.blocks)?.id).toBe(blockForSeed('a', ready.blocks)?.id);
    expect(blockForSeed('a', [])).toBeNull();
  });
});

describe('picker state mapping', () => {
  it('maps every backend state', () => {
    expect(pickerState(null).state).toBe('missing');
    expect(pickerState(missing).state).toBe('missing');
    expect(pickerState({ state: 'preparing', mcVersion: null, blocks: [] }).state).toBe('preparing');
    expect(pickerState(ready)).toEqual({ state: 'ready', blocks: ready.blocks });
    expect(pickerState({ state: 'error', mcVersion: null, blocks: [], errorCode: 'blocks.jar_unreadable' })).toEqual({ state: 'error', key: 'blocks.err.jar_unreadable' });
  });
  it('maps error codes to message keys', () => {
    expect(blockErrorKey('blocks.incomplete')).toBe('blocks.err.incomplete');
    expect(blockErrorKey('blocks.bad_manifest')).toBe('blocks.err.bad_manifest');
    expect(blockErrorKey('net.request')).toBe('blocks.err.net');
    expect(blockErrorKey(undefined)).toBe('blocks.err.generic');
    expect(blockErrorKey('weird')).toBe('blocks.err.generic');
  });
});

const set: PanoramaSet = { mcVersion: '1.21.4', source: 'own', faces: ['a', 'b', 'c', 'd', 'e', 'f'] };
const walls = buildWallpapers(
  { wallpapers: [{ file: 'pale-garden', label: 'Pale', versions: ['1.21.4'], scrim: 0.4 }, { file: 'badlands', label: 'B', generic: true }] },
  { '/a/pale-garden.webp': 'pale.url', '/a/badlands.jpg': 'bad.url' },
);
const ctx = (o: Partial<BackgroundContext> = {}): BackgroundContext => ({ mcVersion: '1.21.4', profileId: 'p1', panorama: set, wallpapers: walls, ...o });

describe('background resolution', () => {
  it('image and none', () => {
    expect(resolveBackground({ kind: 'image', url: 'u' }, ctx())).toMatchObject({ kind: 'image', url: 'u', source: 'custom', scrim: 0.6 });
    expect(resolveBackground({ kind: 'none' }, ctx())).toEqual({ kind: 'plain' });
  });
  it('auto = the wallpaper of the version rule, with its own scrim', () => {
    expect(resolveBackground({ kind: 'auto' }, ctx())).toMatchObject({ kind: 'image', url: 'pale.url', scrim: 0.4, source: 'wallpaper' });
  });
  it('auto without a version rule: the instance\'s own panorama, else a deterministic pool member (generic wallpapers, panoramas, brand surface)', () => {
    expect(resolveBackground({ kind: 'auto' }, ctx({ mcVersion: '1.7.10' })).kind).toBe('panorama');
    const noOwn = resolveBackground({ kind: 'auto' }, ctx({ mcVersion: '1.7.10', panorama: null, pool: buildPool(walls, []) }));
    expect(noOwn).toMatchObject({ url: 'bad.url' }); // pool = [badlands, brand]; profile p1 hashes to the wallpaper
    expect(resolveBackground({ kind: 'auto' }, ctx({ wallpapers: [], panorama: null }))).toEqual({ kind: 'watermark' });
  });
  it('the version wallpaper beats the own panorama; the persisted panorama kind shows the panorama (watermark until extracted)', () => {
    expect(resolveBackground({ kind: 'auto' }, ctx()).kind).toBe('image');
    expect(resolveBackground({ kind: 'panorama' }, ctx()).kind).toBe('panorama');
    expect(resolveBackground({ kind: 'panorama' }, ctx({ panorama: null })).kind).toBe('watermark');
    expect(resolveBackground({ kind: 'panorama' }, ctx({ panorama: { ...set, faces: ['a'] } })).kind).toBe('watermark');
    expect(resolveBackground(undefined, ctx()).kind).toBe('image');
  });
  it('a wallpaper kind resolves by id with the wallpaper scrim; an unknown id falls back to auto', () => {
    expect(resolveBackground({ kind: 'wallpaper', id: 'badlands' }, ctx())).toMatchObject({ kind: 'image', url: 'bad.url', source: 'wallpaper' });
    expect(resolveBackground({ kind: 'wallpaper', id: 'gone' }, ctx())).toMatchObject({ url: 'pale.url' });
  });
  it('dim and blur: the instance override wins, else the resolved default (panorama is blurred by default)', () => {
    const img = resolveBackground({ kind: 'auto' }, ctx());
    expect(backgroundStyle({ kind: 'auto' }, img)).toEqual({ dim: 0.4, blur: 0 });
    expect(backgroundStyle({ kind: 'auto', dim: 0.8, blur: 12 }, img)).toEqual({ dim: 0.8, blur: 12 });
    expect(backgroundStyle({ kind: 'auto', dim: 0 }, img).dim).toBe(0);
    expect(backgroundStyle({ kind: 'panorama' }, resolveBackground({ kind: 'panorama' }, ctx()))).toEqual({ dim: 0.6, blur: PANORAMA_BLUR });
    expect(backgroundStyle({ kind: 'none' }, { kind: 'plain' })).toEqual({ dim: 0.6, blur: 0 });
  });
  it('picker drafts', () => {
    expect(choiceFromBackground({ kind: 'panorama' })).toEqual({ kind: 'panorama' });
    expect(choiceFromBackground({ kind: 'auto' })).toEqual({ kind: 'auto' });
    expect(choiceFromBackground({ kind: 'image', url: 'u' })).toEqual({ kind: 'keep' });
    expect(choiceFromBackground({ kind: 'wallpaper', id: 'badlands' }, walls)).toEqual({ kind: 'wallpaper', id: 'badlands', label: 'B', url: 'bad.url' });
    expect(choiceFromBackground({ kind: 'wallpaper', id: 'gone' }, walls)).toEqual({ kind: 'auto' });
    expect(sameBackground({ kind: 'screenshot', name: 'a', url: '1' }, { kind: 'screenshot', name: 'b', url: '1' })).toBe(false);
    expect(previewBackground({ kind: 'panorama' }, { kind: 'none' })).toEqual({ kind: 'panorama' });
    expect(previewBackground({ kind: 'wallpaper', id: 'badlands', label: 'B', url: 'x' }, { kind: 'none' })).toEqual({ kind: 'wallpaper', id: 'badlands' });
    expect(previewBackground({ kind: 'keep' }, { kind: 'none' })).toEqual({ kind: 'none' });
  });
});

describe('panorama angle', () => {
  it('is deterministic per id, in range, and differs between instances', () => {
    const a = panoramaAngle('3f1c2a7e-5b1d-4c0a-9d51-0a6f5e2b7c11');
    expect(a).toBe(panoramaAngle('3f1c2a7e-5b1d-4c0a-9d51-0a6f5e2b7c11'));
    const seen = new Set<number>();
    for (let i = 0; i < 50; i++) { const x = panoramaAngle(`id-${i}`); expect(x).toBeGreaterThanOrEqual(0); expect(x).toBeLessThan(360); seen.add(x); }
    expect(seen.size).toBeGreaterThan(30);
  });
  it('start delay is negative and within one revolution', () => {
    expect(panoramaDelay(0)).toBe(-0);
    expect(panoramaDelay(180)).toBe(-60);
  });
});
