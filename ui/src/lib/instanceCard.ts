// Click behaviour of an instance card (Library grid / list, Home "Weiter spielen", rail tiles): a single click SELECTS the instance (it becomes the
// Home instance) without navigating, a double click or Enter OPENS its page. The second click of a double click does nothing of its own, so the
// pair never triggers a conflicting action; Enter is handled on keydown and its synthesized click is cancelled.
export interface CardOps {
  select: (id: string) => void;
  open: (id: string) => void;
  isSelected: (id: string) => boolean;
}
export function cardHandlers(id: string, o: CardOps) {
  return {
    onclick: (e: { detail: number }) => { if (e.detail > 1) return; if (!o.isSelected(id)) o.select(id); },
    ondblclick: () => o.open(id),
    onkeydown: (e: { key: string; repeat?: boolean; preventDefault: () => void; defaultPrevented?: boolean }) => {
      if (e.key !== 'Enter' || e.defaultPrevented) return;
      e.preventDefault();
      if (!e.repeat) o.open(id);
    },
  };
}
