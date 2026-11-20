// The library skeleton cannot know whether the user's instances have tags or groups (they change the card and section heights), so the
// Library remembers what it last showed and the skeleton starts from that: the content then replaces it without a jump.
const KEY = 'mango.libraryLayout';
export interface LibraryLayout { tags: boolean; grouped: boolean }

export function readLibraryLayout(): LibraryLayout {
  try {
    const v = JSON.parse(globalThis.localStorage?.getItem(KEY) ?? '{}') as Partial<LibraryLayout>;
    return { tags: !!v.tags, grouped: !!v.grouped };
  } catch { return { tags: false, grouped: false }; }
}
export function writeLibraryLayout(l: LibraryLayout): void {
  try { globalThis.localStorage?.setItem(KEY, JSON.stringify(l)); } catch { /* storage may be unavailable */ }
}
