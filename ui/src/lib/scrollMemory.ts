// Keeps the scroll position of a scroller per key (the instance tab, for example) while its content is swapped.
const positions = new Map<string, number>();

/** Svelte action. `key` changes when the content changes; the old position is saved, the new one restored (retried while a lazy tab renders). */
export function rememberScroll(node: HTMLElement, key: string) {
  let cur = key;
  let last = node.scrollTop;
  let restoring = false;
  let raf = 0;
  const onScroll = () => { if (!restoring) last = node.scrollTop; };
  const restore = (k: string) => {
    cancelAnimationFrame(raf);
    const want = positions.get(k) ?? 0;
    restoring = true;
    let tries = 0;
    const step = () => {
      node.scrollTop = want;
      if (want > 0 && Math.abs(node.scrollTop - want) > 1 && tries++ < 20) { raf = requestAnimationFrame(step); return; }
      last = node.scrollTop;
      restoring = false;
    };
    node.scrollTop = want;
    if (want > 0) raf = requestAnimationFrame(step); else { last = 0; restoring = false; }
  };
  node.addEventListener('scroll', onScroll, { passive: true });
  return {
    update(next: string) {
      if (next === cur) return;
      positions.set(cur, restoring ? (positions.get(cur) ?? 0) : last);
      cur = next;
      restore(next);
    },
    destroy() { cancelAnimationFrame(raf); node.removeEventListener('scroll', onScroll); },
  };
}
export const _positions = positions;
