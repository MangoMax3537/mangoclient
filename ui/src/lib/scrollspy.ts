// Scrollspy for the "on this page" navigator. Pure: section offsets in, the active section id out.
export interface SectionTop { id: string; /** distance of the section top from the top edge of the scroll area (px, negative = already scrolled past) */ top: number }

/**
 * The active section is the last one whose top has reached the line `offset` px below the top edge (the section "nearest the top").
 * Scrolled to the very bottom the last section wins (a short last section could never reach the line). Before the first section: the first.
 */
export function activeSection(tops: readonly SectionTop[], opts: { offset?: number; atBottom?: boolean } = {}): string | null {
  if (!tops.length) return null;
  if (opts.atBottom) return tops[tops.length - 1].id;
  const line = opts.offset ?? 96;
  let cur = tops[0].id;
  for (const s of tops) if (s.top <= line) cur = s.id;
  return cur;
}

/** The nav lists the sections that exist; fewer than two means there is nothing to navigate and the nav is not rendered. */
export const showNavigator = (items: readonly unknown[]): boolean => items.length >= 2;

export const NAV_MIN_WIDTH = 1100;
