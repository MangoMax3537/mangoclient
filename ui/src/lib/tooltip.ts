// Shared tooltip as a Svelte action: `use:tip={'Label'}` or `use:tip={{ text, side: 'right' | 'top' }}`.
// One fixed element for the whole app so it is never clipped by overflow or clip-path. It is a visual
// duplicate of the control's accessible name (aria-label), so it is aria-hidden. Esc dismisses it.
type Opts = string | { text: string; side?: 'right' | 'top' } | null | undefined;

let el: HTMLDivElement | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;

function ensure(): HTMLDivElement {
  if (!el) {
    el = document.createElement('div');
    el.className = 'tooltip';
    el.setAttribute('aria-hidden', 'true');
    document.body.appendChild(el);
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') hide(); });
    document.addEventListener('pointerdown', hide, true);
    document.addEventListener('scroll', hide, true);
  }
  return el;
}
function hide() {
  clearTimeout(timer);
  if (el) el.dataset.open = 'false';
}
function show(node: HTMLElement, text: string, side: 'right' | 'top', delay: number) {
  clearTimeout(timer);
  timer = setTimeout(() => {
    if (!node.isConnected) return;
    const t = ensure();
    t.textContent = text;
    t.dataset.side = side;
    const r = node.getBoundingClientRect();
    t.style.left = '0'; t.style.top = '0';
    const w = t.offsetWidth, h = t.offsetHeight;
    if (side === 'right') { t.style.left = `${Math.round(r.right + 10)}px`; t.style.top = `${Math.round(r.top + (r.height - h) / 2)}px`; }
    else { t.style.left = `${Math.max(8, Math.round(r.left + (r.width - w) / 2))}px`; t.style.top = `${Math.round(r.top - h - 8)}px`; }
    t.dataset.open = 'true';
  }, delay);
}

export function tip(node: HTMLElement, opts: Opts) {
  let cur = opts;
  const get = () => (typeof cur === 'string' ? { text: cur, side: 'right' as const } : cur ? { side: 'right' as const, ...cur } : null);
  const on = (delay: number) => () => { const o = get(); if (o?.text) show(node, o.text, o.side, delay); };
  const enter = on(350), focus = on(0);
  node.addEventListener('mouseenter', enter);
  node.addEventListener('mouseleave', hide);
  node.addEventListener('focus', () => { if (node.matches(':focus-visible')) focus(); });
  node.addEventListener('blur', hide);
  node.addEventListener('click', hide);
  return {
    update(o: Opts) { cur = o; },
    destroy() { hide(); node.removeEventListener('mouseenter', enter); node.removeEventListener('mouseleave', hide); },
  };
}
