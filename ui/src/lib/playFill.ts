// Fill / label clipping math of the Play button while a launch is in progress. The fill layer and its dark label are clipped to the filled part
// (inset from the right), the light label on the track to the unfilled part (inset from the left), so the colour flips exactly at the fill edge.
export interface FillClip { indeterminate: boolean; /** 0..100, the filled share */ pct: number; fillInset: string; trackInset: string }

/** `progress` is 0..1; null/undefined/NaN-free = indeterminate. Out-of-range values are clamped. */
export function fillClip(progress: number | null | undefined): FillClip {
  if (progress == null) return { indeterminate: true, pct: 0, fillInset: 'inset(0 100% 0 0)', trackInset: 'inset(0)' };
  const f = Number.isFinite(progress) ? Math.max(0, Math.min(1, progress)) : 0;
  const pct = Math.round(f * 1000) / 10;
  return { indeterminate: false, pct, fillInset: `inset(0 ${100 - pct}% 0 0)`, trackInset: `inset(0 0 0 ${pct}%)` };
}
