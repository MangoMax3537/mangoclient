// Small pure rules of the Skins page, mirroring the Rust side (docs/ipc.md Part 13).
import type { CapeInfo, LibrarySkin, SkinInfo } from './types';

/** `skin_use_username`: 3..16 of `A-Za-z0-9_` (same as Minecraft names). */
export const USERNAME_RE = /^[A-Za-z0-9_]{3,16}$/;
export const checkUsername = (s: string): boolean => USERNAME_RE.test(s);

/** Most recently added first. */
export const sortLibrary = (l: readonly LibrarySkin[]): LibrarySkin[] => [...l].sort((a, b) => b.addedAt - a.addedAt);

export const activeCape = (i: Pick<SkinInfo, 'capes'> | null | undefined): CapeInfo | null => i?.capes.find((c) => c.active) ?? null;

/** Skin edits exist only when the service says so (offline accounts: `canEdit: false`). */
export const canEditSkin = (i: Pick<SkinInfo, 'canEdit'> | null | undefined): boolean => i?.canEdit === true;

/** Error codes of Part 13 that the page maps to a dedicated sentence; everything else goes through the generic `err.<family>`. */
export const SKIN_ERROR_CODES = ['skin.offline_account', 'skin.invalid_png', 'skin.too_large', 'skin.not_found', 'skin.upload_failed', 'skin.library_full', 'auth.invalid_username'] as const;

/** A default library name from a file name: `my_skin.png` -> `my skin`. */
export function nameFromFile(file: string): string {
  const base = (file.split(/[\\/]/).pop() ?? file).replace(/\.[a-z0-9]+$/i, '').replace(/[_-]+/g, ' ').trim();
  return (base || 'Skin').slice(0, 48);
}
