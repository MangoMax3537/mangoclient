// Back / forward history of the main navigation. Only restorable states are recorded (view, profile page + tab, discover scope);
// dialogs, menus and popovers are never part of it. Pure data structure, the store wires it to `nav`.
export interface NavSnapshot {
  view: string;
  profileId: string | null;
  profileTab: string;
  discoverProfile: string | null;
  discoverType: string;
}

/** Fields that matter for a view: a profile page has an id and a tab, Discover has its scope, everything else only the view. */
export function normalize(s: NavSnapshot): NavSnapshot {
  return {
    view: s.view,
    profileId: s.view === 'profile' ? s.profileId : null,
    profileTab: s.view === 'profile' ? s.profileTab : 'overview',
    discoverProfile: s.view === 'discover' ? s.discoverProfile : null,
    discoverType: s.view === 'discover' ? s.discoverType : 'mod',
  };
}
export const sameState = (a: NavSnapshot, b: NavSnapshot): boolean => {
  const x = normalize(a), y = normalize(b);
  return x.view === y.view && x.profileId === y.profileId && x.profileTab === y.profileTab && x.discoverProfile === y.discoverProfile && x.discoverType === y.discoverType;
};

export const HISTORY_CAP = 100;

export class NavHistory {
  #stack: NavSnapshot[] = [];
  #i = -1;
  /** Bumped on every change so a reactive wrapper can follow. */
  rev = 0;

  get current(): NavSnapshot | null { return this.#stack[this.#i] ?? null; }
  get size(): number { return this.#stack.length; }
  get index(): number { return this.#i; }
  get canBack(): boolean { return this.#i > 0; }
  get canForward(): boolean { return this.#i >= 0 && this.#i < this.#stack.length - 1; }

  /** Records a state the user navigated to. The same state as the current one is ignored; a new state drops the forward entries. */
  record(s: NavSnapshot): void {
    const n = normalize(s);
    const cur = this.current;
    if (cur && sameState(cur, n)) return;
    this.#stack.length = this.#i + 1;
    this.#stack.push(n);
    if (this.#stack.length > HISTORY_CAP) this.#stack.shift();
    this.#i = this.#stack.length - 1;
    this.rev++;
  }

  /** Moves one step and returns the state to restore. `valid` skips entries that no longer exist (a deleted profile). */
  back(valid: (s: NavSnapshot) => boolean = () => true): NavSnapshot | null { return this.#step(-1, valid); }
  forward(valid: (s: NavSnapshot) => boolean = () => true): NavSnapshot | null { return this.#step(1, valid); }

  #step(d: 1 | -1, valid: (s: NavSnapshot) => boolean): NavSnapshot | null {
    let j = this.#i + d;
    while (j >= 0 && j < this.#stack.length && !valid(this.#stack[j])) j += d;
    if (j < 0 || j >= this.#stack.length) return null;
    this.#i = j;
    this.rev++;
    return { ...this.#stack[j] };
  }

  clear(): void { this.#stack = []; this.#i = -1; this.rev++; }
}
