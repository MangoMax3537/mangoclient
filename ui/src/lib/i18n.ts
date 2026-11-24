import { de2, en2 } from './i18n2';
import { de3, en3 } from './i18n3';
import { de4, en4 } from './i18n4';
import { de5, en5 } from './i18n5';
import { de7, en7 } from './i18n7';
import { de8, en8 } from './i18n8';
import { de9, en9 } from './i18n9';
import { de10, en10 } from './i18n10';
import { de11, en11 } from './i18n11';
import { de12, en12 } from './i18n12';
import { de13, en13 } from './i18n13';
import { de14, en14 } from './i18n14';
import { de15, en15 } from './i18n15';
import { de16, en16 } from './i18n16';

export type Lang = 'de' | 'en';

const en = {
  'app.name': 'Mango Client',
  'nav.home': 'Home', 'nav.library': 'Library', 'nav.discover': 'Discover', 'nav.servers': 'Servers',
  'nav.settings': 'Settings', 'nav.soon': 'Coming soon', 'nav.recent': 'Recent profiles', 'nav.main': 'Main navigation',
  'nav.accounts': 'Accounts', 'nav.noAccount': 'No account',
  'home.play': 'Play', 'home.stop': 'Stop', 'home.stopping': 'Stopping…', 'home.switch': 'Switch profile', 'home.lastPlayed': 'Last played',
  'home.playtime': 'Playtime', 'home.never': 'Never', 'home.noProfiles': 'No profiles yet',
  'home.noProfilesHint': 'Create a profile to start playing.', 'home.continue': 'All profiles',
  'home.running': 'Running', 'home.mods': 'Mods',
  'chip.java': 'Java', 'chip.ram': 'RAM', 'chip.auto': 'Auto', 'chip.default': 'Default',
  'launch.preparing': 'Preparing', 'launch.java': 'Java', 'launch.loader': 'Loader', 'launch.downloading': 'Downloading',
  'launch.account': 'Account', 'launch.launching': 'Launching', 'launch.running': 'Running',
  'launch.stopped': 'Stopped', 'launch.crashed': 'Crashed', 'launch.progress': 'Launch progress',
  'phase.client': 'Game client', 'phase.libraries': 'Libraries', 'phase.assets': 'Assets', 'phase.java': 'Java runtime',
  'phase.modpack': 'Modpack', 'phase.done': 'Done',
  'console.title': 'Console', 'console.empty': 'No output yet.', 'console.show': 'Show console', 'console.hide': 'Hide console',
  'console.lines.one': '{n} line', 'console.lines.other': '{n} lines', 'console.clear': 'Clear', 'console.copyAll': 'Copy all', 'console.copied': 'Copied',
  'library.title': 'Library', 'library.search': 'Search profiles', 'library.sort': 'Sort',
  'library.sort.recent': 'Last played', 'library.sort.name': 'Name', 'library.sort.playtime': 'Playtime',
  'library.grid': 'Grid view', 'library.list': 'List view', 'library.new': 'New profile',
  'library.count.one': '{n} profile', 'library.count.other': '{n} profiles', 'library.empty': 'No profile matches.',
  'profile.mods.one': '{n} mod', 'profile.mods.other': '{n} mods', 'profile.vanilla': 'Vanilla',
  'profile.delete': 'Delete', 'profile.duplicate': 'Duplicate',
  'col.name': 'Profile', 'col.version': 'Version', 'col.loader': 'Loader', 'col.last': 'Last played', 'col.time': 'Playtime',
  'new.title': 'New profile', 'new.step.name': 'Name & look', 'new.step.version': 'Version & loader', 'new.step.resources': 'Resources',
  'new.name': 'Name', 'new.namePlaceholder': 'e.g. Iron Hollow', 'new.nameRequired': 'Enter a name.',
  'new.color': 'Accent colour', 'new.search': 'Search versions', 'new.releases': 'Releases', 'new.snapshots': 'Snapshots',
  'new.all': 'All', 'new.loader': 'Loader', 'new.loaderVersion': 'Loader version', 'new.loadingVersions': 'Loading versions...',
  'new.noVersions': 'No version matches.', 'new.showMore': 'Show more', 'new.latest': 'latest', 'new.installed': 'installed',
  'new.recommended': 'Recommended', 'new.ram': 'Memory',
  'new.summary': 'Summary', 'new.next': 'Next', 'new.back': 'Back', 'new.create': 'Create profile', 'new.cancel': 'Cancel',
  'new.creating': 'Creating...', 'new.pickVersion': 'Pick a version.', 'new.noLoaderVersion': 'No loader version for this game version.',
  'accounts.title': 'Accounts', 'accounts.addOffline': 'Add offline account', 'accounts.offlineName': 'Player name',
  'accounts.add': 'Add', 'accounts.microsoft': 'Sign in with Microsoft', 'accounts.offline': 'Offline',
  'accounts.msa': 'Microsoft', 'accounts.expired': 'Session expired', 'accounts.active': 'Active', 'accounts.select': 'Use account',
  'accounts.remove': 'Remove', 'accounts.empty': 'No accounts yet.', 'accounts.nameInvalid': '3 to 16 letters, digits or underscores.',
  'settings.title': 'Settings', 'settings.language': 'Language', 'settings.language.de': 'Deutsch', 'settings.language.en': 'English',
  'settings.ramOf': 'of {total} installed', 'settings.javaArgs': 'Java arguments',
  'settings.javaArgsHint': 'Added to every launch. Per-profile arguments are appended.', 'settings.keepOpen': 'Keep launcher open while playing',
  'settings.keepOpenHint': 'Otherwise the window hides until the game closes.', 'settings.telemetry': 'Send anonymous usage ping',
  'settings.telemetryHint': 'Random install id, version and platform. Nothing else.', 'settings.group.general': 'Launcher',
  'settings.group.game': 'Game', 'settings.saved': 'Saved', 'settings.version': 'Version', 'settings.dataDir': 'Data folder',
  'common.close': 'Close', 'common.loading': 'Loading', 'common.retry': 'Retry', 'common.mb': '{n} MB', 'common.gb': '{n} GB',
  'common.dismiss': 'Dismiss',
  'err.not_implemented': 'Not available yet.', 'err.unknown': 'Something went wrong.', 'err.offline': 'No connection.',
  'err.invalid_name': 'That name is not valid.', 'err.launch_failed': 'The game could not be started.',
  'err.relogin': 'Please sign in again.',
  'time.justNow': 'just now',
  ...en2,
  ...en3,
  ...en4,
  ...en5,
  ...en7,
  ...en8,
  ...en9,
  ...en10,
  ...en11,
  ...en12,
  ...en13,
  ...en14,
  ...en15,
  ...en16,
} as const;

export type Key = keyof typeof en;
type PluralBase<K> = K extends `${infer B}.other` ? B : never;
export type PluralKey = PluralBase<Key>;

const de: Record<Key, string> = {
  'app.name': 'Mango Client',
  'nav.home': 'Start', 'nav.library': 'Bibliothek', 'nav.discover': 'Entdecken', 'nav.servers': 'Server',
  'nav.settings': 'Einstellungen', 'nav.soon': 'Bald verfügbar', 'nav.recent': 'Zuletzt gespielt', 'nav.main': 'Hauptnavigation',
  'nav.accounts': 'Konten', 'nav.noAccount': 'Kein Konto',
  'home.play': 'Spielen', 'home.stop': 'Beenden', 'home.stopping': 'Wird beendet…', 'home.switch': 'Profil wechseln', 'home.lastPlayed': 'Zuletzt gespielt',
  'home.playtime': 'Spielzeit', 'home.never': 'Nie', 'home.noProfiles': 'Noch keine Profile',
  'home.noProfilesHint': 'Lege ein Profil an, um loszuspielen.', 'home.continue': 'Alle Profile',
  'home.running': 'Läuft', 'home.mods': 'Mods',
  'chip.java': 'Java', 'chip.ram': 'RAM', 'chip.auto': 'Automatisch', 'chip.default': 'Standard',
  'launch.preparing': 'Vorbereiten', 'launch.java': 'Java', 'launch.loader': 'Loader', 'launch.downloading': 'Herunterladen',
  'launch.account': 'Konto', 'launch.launching': 'Starten', 'launch.running': 'Läuft',
  'launch.stopped': 'Beendet', 'launch.crashed': 'Abgestürzt', 'launch.progress': 'Startfortschritt',
  'phase.client': 'Spielclient', 'phase.libraries': 'Bibliotheken', 'phase.assets': 'Assets', 'phase.java': 'Java-Laufzeit',
  'phase.modpack': 'Modpack', 'phase.done': 'Fertig',
  'console.title': 'Konsole', 'console.empty': 'Noch keine Ausgabe.', 'console.show': 'Konsole anzeigen', 'console.hide': 'Konsole ausblenden',
  'console.lines.one': '{n} Zeile', 'console.lines.other': '{n} Zeilen', 'console.clear': 'Leeren', 'console.copyAll': 'Alles kopieren', 'console.copied': 'Kopiert',
  'library.title': 'Bibliothek', 'library.search': 'Profile durchsuchen', 'library.sort': 'Sortieren',
  'library.sort.recent': 'Zuletzt gespielt', 'library.sort.name': 'Name', 'library.sort.playtime': 'Spielzeit',
  'library.grid': 'Rasteransicht', 'library.list': 'Listenansicht', 'library.new': 'Neues Profil',
  'library.count.one': '{n} Profil', 'library.count.other': '{n} Profile', 'library.empty': 'Kein Profil gefunden.',
  'profile.mods.one': '{n} Mod', 'profile.mods.other': '{n} Mods', 'profile.vanilla': 'Vanilla',
  'profile.delete': 'Löschen', 'profile.duplicate': 'Duplizieren',
  'col.name': 'Profil', 'col.version': 'Version', 'col.loader': 'Loader', 'col.last': 'Zuletzt gespielt', 'col.time': 'Spielzeit',
  'new.title': 'Neues Profil', 'new.step.name': 'Name & Look', 'new.step.version': 'Version & Loader', 'new.step.resources': 'Ressourcen',
  'new.name': 'Name', 'new.namePlaceholder': 'z. B. Iron Hollow', 'new.nameRequired': 'Gib einen Namen ein.',
  'new.color': 'Akzentfarbe', 'new.search': 'Versionen suchen', 'new.releases': 'Releases', 'new.snapshots': 'Snapshots',
  'new.all': 'Alle', 'new.loader': 'Loader', 'new.loaderVersion': 'Loader-Version', 'new.loadingVersions': 'Versionen werden geladen...',
  'new.noVersions': 'Keine passende Version.', 'new.showMore': 'Mehr anzeigen', 'new.latest': 'aktuell', 'new.installed': 'installiert',
  'new.recommended': 'Empfohlen', 'new.ram': 'Arbeitsspeicher',
  'new.summary': 'Zusammenfassung', 'new.next': 'Weiter', 'new.back': 'Zurück', 'new.create': 'Profil erstellen', 'new.cancel': 'Abbrechen',
  'new.creating': 'Wird erstellt...', 'new.pickVersion': 'Wähle eine Version.', 'new.noLoaderVersion': 'Keine Loader-Version für diese Spielversion.',
  'accounts.title': 'Konten', 'accounts.addOffline': 'Offline-Konto hinzufügen', 'accounts.offlineName': 'Spielername',
  'accounts.add': 'Hinzufügen', 'accounts.microsoft': 'Mit Microsoft anmelden', 'accounts.offline': 'Offline',
  'accounts.msa': 'Microsoft', 'accounts.expired': 'Sitzung abgelaufen', 'accounts.active': 'Aktiv', 'accounts.select': 'Konto verwenden',
  'accounts.remove': 'Entfernen', 'accounts.empty': 'Noch keine Konten.', 'accounts.nameInvalid': '3 bis 16 Buchstaben, Ziffern oder Unterstriche.',
  'settings.title': 'Einstellungen', 'settings.language': 'Sprache', 'settings.language.de': 'Deutsch', 'settings.language.en': 'English',
  'settings.ramOf': 'von {total} installiert', 'settings.javaArgs': 'Java-Argumente',
  'settings.javaArgsHint': 'Werden bei jedem Start übergeben. Profil-Argumente folgen danach.', 'settings.keepOpen': 'Launcher beim Spielen geöffnet lassen',
  'settings.keepOpenHint': 'Sonst wird das Fenster bis zum Spielende ausgeblendet.', 'settings.telemetry': 'Anonymen Nutzungs-Ping senden',
  'settings.telemetryHint': 'Zufällige Installations-ID, Version und Plattform. Sonst nichts.', 'settings.group.general': 'Launcher',
  'settings.group.game': 'Spiel', 'settings.saved': 'Gespeichert', 'settings.version': 'Version', 'settings.dataDir': 'Datenordner',
  'common.close': 'Schließen', 'common.loading': 'Lädt', 'common.retry': 'Erneut versuchen', 'common.mb': '{n} MB', 'common.gb': '{n} GB',
  'common.dismiss': 'Ausblenden',
  'err.not_implemented': 'Noch nicht verfügbar.', 'err.unknown': 'Etwas ist schiefgelaufen.', 'err.offline': 'Keine Verbindung.',
  'err.invalid_name': 'Dieser Name ist ungültig.', 'err.launch_failed': 'Das Spiel konnte nicht gestartet werden.',
  'err.relogin': 'Bitte melde dich erneut an.',
  'time.justNow': 'gerade eben',
  ...de2,
  ...de3,
  ...de4,
  ...de5,
  ...de7,
  ...de8,
  ...de9,
  ...de10,
  ...de11,
  ...de12,
  ...de13,
  ...de14,
  ...de15,
  ...de16,
};

export const dictionaries: Record<Lang, Partial<Record<string, string>>> = { de, en };
export const DEFAULT_LANG: Lang = 'de';

export type Vars = Record<string, string | number>;

function interpolate(s: string, vars?: Vars): string {
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

/** Look up `key` in `lang`, fall back to English, then to the key itself. */
export function translate(lang: Lang, key: string, vars?: Vars): string {
  const s = dictionaries[lang]?.[key] ?? dictionaries.en[key] ?? key;
  return interpolate(s, vars);
}

/** Plural helper: picks `<key>.one|other` by Intl.PluralRules; `{n}` is filled automatically. */
export function plural(lang: Lang, key: PluralKey | (string & {}), n: number, vars?: Vars): string {
  const cat = new Intl.PluralRules(lang).select(n);
  const dict = dictionaries[lang];
  const k = dict[`${key}.${cat}`] !== undefined ? `${key}.${cat}` : `${key}.other`;
  return translate(lang, k, { n, ...vars });
}

export function hasKey(lang: Lang, key: string): boolean {
  return dictionaries[lang]?.[key] !== undefined;
}

export function isLang(v: unknown): v is Lang {
  return v === 'de' || v === 'en';
}
