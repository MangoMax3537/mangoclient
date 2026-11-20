// The settings index: one typed list per page. Every entry names its section, the i18n keys of its label and description, the de + en
// synonyms ("meta keywords") the search also matches, and the id of the row element it scrolls to and focuses (`set-<id>`, set by SettingRow).
// Aliases are search data, not UI text, so they are not translated and may mix German and English on purpose.
import type { Key } from './i18n';
import { SHORTCUT_ALIASES, SHORTCUT_GROUPS, comboText } from './keybinds';

export type SettingsScope = 'app' | 'instance';
export interface SectionDef { id: string; scope: SettingsScope; label: Key }
export interface SettingEntry {
  id: string;
  scope: SettingsScope;
  section: string;
  label: Key;
  desc?: Key;
  aliases: readonly string[];
  /** DOM id of the row to scroll to, flash and focus. */
  target: string;
}

export const SECTIONS: readonly SectionDef[] = [
  { id: 'look', scope: 'app', label: 'settings.group.appearance' },
  { id: 'launcher', scope: 'app', label: 'settings.group.general' },
  { id: 'accounts', scope: 'app', label: 'nav.accounts' },
  { id: 'game', scope: 'app', label: 'settings.group.game' },
  { id: 'backups', scope: 'app', label: 'settings.group.backups' },
  { id: 'keys', scope: 'app', label: 'keys.sectionLabel' },
  { id: 'storage', scope: 'app', label: 'storage.title' },
  { id: 'updates', scope: 'app', label: 'update.title' },
  { id: 'general', scope: 'instance', label: 'psettings.group.general' },
  { id: 'game', scope: 'instance', label: 'settings.group.game' },
  { id: 'danger', scope: 'instance', label: 'psettings.danger' },
];

const e = (scope: SettingsScope, section: string, id: string, label: Key, aliases: string[], desc?: Key): SettingEntry =>
  ({ id, scope, section, label, aliases, target: `set-${id}`, ...(desc ? { desc } : {}) });

/** One entry per shortcut, from the same data as the shortcut sheet; found by its name, its key combination and the words tastenkürzel / shortcut / hotkey / keybinds / tastatur / keyboard. */
const SHORTCUT_ENTRIES: SettingEntry[] = SHORTCUT_GROUPS.flatMap((g) => g.items.map((it) =>
  e('app', 'keys', `key-${g.id}-${it.id}`, it.label, [...SHORTCUT_ALIASES, ...(it.aliases ?? []), ...it.keys(false).map((c) => comboText(c, false)), ...it.keys(true).map((c) => comboText(c, true))])));

export const SETTINGS_INDEX: readonly SettingEntry[] = [
  /* ---- launcher settings ---- */
  e('app', 'look', 'theme', 'settings.theme', ['theme', 'design', 'darkmode', 'dark mode', 'dunkel', 'dunkelmodus', 'hell', 'light', 'light mode', 'farbschema', 'appearance', 'darstellung', 'color scheme', 'nachtmodus'], 'settings.theme.hint'),
  e('app', 'look', 'cycleBg', 'settings.cycleBg', ['hintergrund', 'wallpaper', 'mango client', 'mango-client', 'wechseln', 'cycle', 'slideshow', 'diashow', 'background', 'home', 'panorama', 'rotate', 'überblenden', 'crossfade', 'animation'], 'settings.cycleBgHint'),
  e('app', 'look', 'bgAll', 'settings.bgAll', ['hintergrund', 'background', 'alle bereiche', 'all areas', 'überall', 'everywhere', 'unscharf', 'blur', 'abdunkeln', 'dim', 'bibliothek', 'library', 'wallpaper']),
  e('app', 'launcher', 'language', 'settings.language', ['language', 'sprache', 'locale', 'deutsch', 'english', 'englisch', 'german', 'translation', 'übersetzung']),
  e('app', 'launcher', 'keepOpen', 'settings.keepOpen', ['launcher', 'fenster', 'window', 'hide', 'ausblenden', 'minimieren', 'close', 'schließen', 'tray'], 'settings.keepOpenHint'),
  e('app', 'launcher', 'telemetry', 'settings.telemetry', ['telemetry', 'telemetrie', 'ping', 'analytics', 'statistik', 'privacy', 'datenschutz', 'tracking', 'anonym', 'anonymous', 'nutzungsdaten'], 'settings.telemetryHint'),
  e('app', 'accounts', 'accounts', 'nav.accounts', ['account', 'accounts', 'konto', 'konten', 'login', 'anmelden', 'microsoft', 'offline', 'skin'], 'settings.accounts.hint'),
  e('app', 'game', 'ram', 'settings.ram', ['ram', 'memory', 'speicher', 'arbeitsspeicher', 'heap', 'xmx', 'gb', 'gigabyte'], 'settings.ramHint'),
  e('app', 'game', 'javaArgs', 'settings.javaArgs', ['java', 'jvm', 'args', 'arguments', 'argumente', 'flags', 'runtime', 'parameter', 'xx'], 'settings.javaArgsHint'),
  e('app', 'game', 'fullscreen', 'settings.fullscreen', ['fullscreen', 'vollbild', 'fenster', 'window', 'resolution', 'auflösung', 'bildschirm', 'display'], 'settings.fullscreenHint'),
  e('app', 'game', 'windowSize', 'settings.windowSize', ['resolution', 'auflösung', 'size', 'größe', 'width', 'height', 'breite', 'höhe', 'fenster', 'window', 'pixel'], 'settings.windowSizeHint'),
  e('app', 'backups', 'autoSnapshot', 'settings.autoSnapshot', ['backup', 'snapshot', 'sicherung', 'restore', 'wiederherstellen', 'rollback', 'zurück'], 'settings.autoSnapshotHint'),
  e('app', 'backups', 'backupKeep', 'settings.backupKeep', ['backup', 'backups', 'sicherung', 'keep', 'behalten', 'aufbewahren', 'anzahl', 'rotation'], 'settings.backupKeepHint'),
  ...SHORTCUT_ENTRIES,
  e('app', 'storage', 'storage', 'storage.title', ['storage', 'speicherplatz', 'cache', 'festplatte', 'disk', 'aufräumen', 'cleanup', 'clean', 'platz', 'größe', 'leeren']),
  e('app', 'updates', 'updates', 'update.title', ['update', 'updates', 'aktualisieren', 'aktualisierung', 'version', 'release', 'upgrade', 'neue version', 'check', 'suchen']),

  /* ---- instance settings ---- */
  e('instance', 'general', 'name', 'new.name', ['name', 'umbenennen', 'rename', 'titel', 'title']),
  e('instance', 'general', 'grouptags', 'group.label', ['group', 'gruppe', 'tag', 'tags', 'kategorie', 'ordnen', 'sortieren', 'organize', 'library', 'bibliothek'], 'group.hint'),
  e('instance', 'general', 'icon', 'psettings.icon', ['icon', 'symbol', 'block', 'bild', 'image', 'logo', 'modpack', 'mrpack', 'avatar', 'würfel', 'cube'], 'psettings.iconHint'),
  e('instance', 'general', 'background', 'psettings.background', ['background', 'hintergrund', 'wallpaper', 'mango client', 'panorama', 'screenshot', 'blur', 'unschärfe', 'dim', 'abdunkeln']),
  e('instance', 'game', 'version', 'psettings.version', ['loader', 'fabric', 'forge', 'quilt', 'neoforge', 'version', 'minecraft', 'mc', 'snapshot', 'release', 'wechseln', 'change', 'modpack', 'mrpack']),
  e('instance', 'game', 'ram', 'new.ram', ['ram', 'memory', 'speicher', 'arbeitsspeicher', 'heap', 'xmx', 'gb', 'gigabyte'], 'ram.hint.instance'),
  e('instance', 'game', 'javaArgs', 'settings.javaArgs', ['java', 'jvm', 'args', 'arguments', 'argumente', 'flags', 'runtime', 'parameter', 'xx'], 'psettings.argsHint'),
  e('instance', 'game', 'java', 'psettings.java', ['java', 'jre', 'jdk', 'runtime', 'laufzeit', 'temurin', 'pfad', 'path', 'installieren', 'install', 'automatisch', 'automatic', 'auto', 'version', 'custom', 'eigener pfad', 'oracle', 'openjdk'], 'psettings.java.hint'),
  e('instance', 'game', 'window', 'psettings.window', ['fenster', 'window', 'fullscreen', 'vollbild', 'auflösung', 'resolution', 'größe', 'size', 'width', 'height', 'breite', 'höhe', 'pixel', 'maximiert', 'maximized', '1920', '1280', '854', 'bildschirm', 'display'], 'psettings.window.hint'),
  e('instance', 'game', 'mango', 'psettings.mango', ['mangoconfig', 'mango', 'config', 'mod', 'ingame', 'in game', 'einstellungen im spiel']),
  e('instance', 'danger', 'delete', 'psettings.deleteTitle', ['delete', 'löschen', 'entfernen', 'remove', 'danger', 'gefahr', 'gefahrenbereich', 'papierkorb', 'trash'], 'psettings.deleteHint'),
];

export const entriesFor = (scope: SettingsScope): SettingEntry[] => SETTINGS_INDEX.filter((x) => x.scope === scope);
export const sectionOf = (e: SettingEntry): SectionDef => SECTIONS.find((s) => s.id === e.section && s.scope === e.scope) as SectionDef;
