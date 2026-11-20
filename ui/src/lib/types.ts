// The IPC types are generated from the Rust definitions (./generated, see its README.txt). This file only re-exports them and adds
// a few UI-only aliases. Do not hand-write wire types here.
import type { BlockFaces, ConfigView, EventMap, InstanceIcon as WireInstanceIcon, LogLevelName, ModType, PlanConflict, UpdateStatus } from './generated';

export * from './generated';

/** The config as returned by `app_state` / `config_set` (the Rust type is `ConfigView`). The write path is `ConfigPatch`. */
export type Config = ConfigView;
export type ContentType = ModType;
export type LogLevel = LogLevelName;
export type InstallConflict = PlanConflict;
export type BlockFace = keyof BlockFaces;
export type UpdateStateName = UpdateStatus['state'];
/** Event name -> payload (the Rust side calls it `EventMap`). */
export type ApiEvents = EventMap;
/** `null` = automatic (`ProfileView.icon`). */
export type InstanceIcon = WireInstanceIcon | null;
