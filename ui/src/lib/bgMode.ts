import type { ResolvedBackground } from './instanceIcon';
import { DEFAULT_SCRIM } from './wallpapers';

/** Full-window background behind every view except the Home hero: strong uniform scrim and blur so content stays readable. */
export const FULL_DIM = 0.78;
export const FULL_BLUR = 10;

/** The panorama cube stays hero-only: the full-window layer uses its front face as a flat image. */
export function flatten(r: ResolvedBackground): ResolvedBackground {
  return r.kind === 'panorama' && r.set.faces[0] ? { kind: 'image', url: r.set.faces[0], scrim: DEFAULT_SCRIM, source: 'wallpaper' } : r;
}

/** Which instance's background the engine shows: the open instance page, else the Home/selected instance, else none (zero instances). */
export function engineProfile<P extends { id: string }>(view: string, profileId: string | null, profiles: readonly P[], selected: P | null): P | null {
  if (view === 'profile' && profileId) return profiles.find((p) => p.id === profileId) ?? selected;
  return selected;
}
