import { describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import Skeleton from './Skeleton.svelte';
import SkeletonRegion from './SkeletonRegion.svelte';
import ProfileCardSkeleton from './ProfileCardSkeleton.svelte';
import SearchResultSkeleton from './SearchResultSkeleton.svelte';
import Img from '../Img.svelte';
import LoadError from '../LoadError.svelte';

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const html = (c: unknown, props: Record<string, unknown> = {}) => render(c as any, { props } as any).body;

describe('Skeleton', () => {
  it('every shape is aria-hidden, grey (.skel), and has no text, icon or image inside', () => {
    for (const variant of ['line', 'bar', 'box', 'circle', 'avatar'] as const) {
      const out = html(Skeleton, { variant, w: 80, h: 12 });
      expect(out, variant).toContain('aria-hidden="true"');
      expect(out, variant).toContain('skel');
      expect(out, variant).not.toMatch(/<svg|<img|>[^<\s]/);
    }
  });
  it('avatar and circle are round squares of `size`', () => {
    expect(html(Skeleton, { variant: 'avatar' })).toContain('width: 40px');
    expect(html(Skeleton, { variant: 'circle', size: 24 })).toContain('height: 24px');
  });
  it('a notch variant uses the notch shape', () => { expect(html(Skeleton, { variant: 'box', w: 40, h: 40, notch: 8 })).toContain('notch'); });
});

describe('layout presets', () => {
  it('have aria-busy, one visually hidden "Loading…" status and aria-hidden shapes', () => {
    for (const c of [ProfileCardSkeleton, SearchResultSkeleton]) {
      const out = html(c, { count: 3 });
      expect(out).toContain('aria-busy="true"');
      expect(out).toContain('role="status"');
      expect(out.match(/sr-only/g)?.length).toBe(1);
      expect(out).toContain('aria-hidden="true"');
    }
  });
  it('announce=false leaves out the status for nested use', () => {
    const out = html(SearchResultSkeleton, { count: 2, announce: false });
    expect(out).not.toContain('role="status"');
    expect(out).not.toContain('sr-only');
  });
  it('renders `count` cards / rows', () => {
    expect(html(ProfileCardSkeleton, { count: 4 }).match(/class="cell"/g)?.length).toBe(4);
    expect(html(SearchResultSkeleton, { count: 5 }).match(/class="row/g)?.length).toBe(5);
  });
  it('SkeletonRegion renders no status when announce is off', () => {
    const out = html(SkeletonRegion, { announce: false, children: () => {} });
    expect(out).not.toContain('role="status"');
  });
});

describe('Img', () => {
  it('loading: a plain grey pulsing box and the (still transparent) image, no fallback yet', () => {
    const out = html(Img, { src: 'a.png', fallback: 'fb.png' });
    expect(out).toContain('data-phase="loading"');
    expect(out).toContain('skel');
    expect(out).toContain('src="a.png"');
    expect(out).not.toContain('fb.png');
    expect(out).not.toMatch(/class="[^"]*\bready\b/);
  });
  it('no URL: the real fallback straight away, no grey box, no broken image', () => {
    const out = html(Img, { src: null, fallback: 'fb.png' });
    expect(out).toContain('data-phase="none"');
    expect(out).toContain('src="fb.png"');
    expect(out).not.toContain('skel');
  });
  it('explicit aspect-ratio keeps the slot size before the image is there', () => {
    expect(html(Img, { src: 'a.png', aspect: '16 / 9' })).toContain('aspect-ratio: 16 / 9');
  });
});

describe('LoadError', () => {
  it('is an alert with a localized message and a Retry button; the timeout has its own wording', () => {
    const t = html(LoadError, { error: { code: 'net.timeout', message: '', needsRelogin: false }, onretry: () => {} });
    expect(t).toContain('role="alert"');
    expect(t).toContain('button');
    expect(t).toMatch(/Erneut versuchen|Retry/);
    const f = html(LoadError, { error: { code: 'net.request', message: '', needsRelogin: false }, onretry: () => {} });
    expect(f).not.toBe(t);
  });
});
