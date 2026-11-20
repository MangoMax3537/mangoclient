// Last known skin info and library per account, kept for the session: the Skins page shows these at once when it opens (the skeleton is only
// for the very first load) and refreshes silently behind them.
import type { LibrarySkin, SkinInfo } from './types';

const infos = new Map<string, SkinInfo>();
let library: LibrarySkin[] | null = null;

export const cachedSkin = (uuid: string): SkinInfo | null => infos.get(uuid) ?? null;
export const rememberSkin = (uuid: string, i: SkinInfo): void => { infos.set(uuid, i); };
export const cachedLibrary = (): LibrarySkin[] | null => library;
export const rememberLibrary = (l: LibrarySkin[]): void => { library = l; };
export function resetSkinCache(): void { infos.clear(); library = null; }
