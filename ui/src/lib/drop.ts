// `files:dropped` (docs/ipc.md Part 10): the shell registers a one-shot path grant for every dropped path, then emits `{ paths }`.
import type { Api, Unlisten } from './api';
import type { FilesDroppedEvent } from './generated';

/** Subscribes; malformed payloads (no array, non-string entries) are reduced to what is usable. */
export const listenDropped = (api: Api, cb: (e: FilesDroppedEvent) => void): Promise<Unlisten> =>
  api.listen('files:dropped', (e) => cb({ paths: Array.isArray(e?.paths) ? e.paths.filter((p) => typeof p === 'string') : [] }));
