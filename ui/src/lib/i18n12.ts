// Part 12: CurseForge removed (import message), content split into Mods / Shaders / Resource packs / Data packs tabs, the settings search and
// the window-size / accounts rows it can find.
export const en12 = {
  'err.import.unsupported_curseforge': 'CurseForge packs are not supported. Use a Modrinth pack (.mrpack) or a Prism / MultiMC instance.',
  'discover.type.datapack': 'Data packs',

  /* instance tabs */
  'profile.tab.mods': 'Mods', 'profile.tab.shaders': 'Shaders', 'profile.tab.resourcepacks': 'Resource packs', 'profile.tab.datapacks': 'Data packs',
  'content.search.mod': 'Search mods', 'content.search.shader': 'Search shaders', 'content.search.resourcepack': 'Search resource packs', 'content.search.datapack': 'Search data packs',
  'content.add.mod': 'Add mods', 'content.add.shader': 'Add shaders', 'content.add.resourcepack': 'Add resource packs', 'content.add.datapack': 'Add data packs',
  'content.emptyTitle.mod': 'No mods yet', 'content.emptyTitle.shader': 'No shaders yet', 'content.emptyTitle.resourcepack': 'No resource packs yet', 'content.emptyTitle.datapack': 'No data packs yet',
  'content.empty.mod': 'Add mods from Modrinth. Required dependencies are shown before anything is installed.',
  'content.empty.shader': 'Add shader packs from Modrinth. They need a shader mod such as Iris to run.',
  'content.empty.resourcepack': 'Add resource packs from Modrinth to change textures, sounds and the look of the game.',
  'content.empty.datapack': 'Add data packs from Modrinth. They apply to the worlds of this instance.',
  'content.updatesAvailable.mod.one': '{n} mod update available', 'content.updatesAvailable.mod.other': '{n} mod updates available',
  'content.updatesAvailable.shader.one': '{n} shader update available', 'content.updatesAvailable.shader.other': '{n} shader updates available',
  'content.updatesAvailable.resourcepack.one': '{n} resource pack update available', 'content.updatesAvailable.resourcepack.other': '{n} resource pack updates available',
  'content.updatesAvailable.datapack.one': '{n} data pack update available', 'content.updatesAvailable.datapack.other': '{n} data pack updates available',

  /* settings */
  'settings.search.label': 'Search settings', 'settings.search.placeholder': 'Search settings, for example "dark mode", "ram" or "backup"',
  'settings.search.clear': 'Clear search', 'settings.search.none': 'Nothing found for "{q}"', 'settings.search.matched': 'matched: {alias}',
  'settings.fullscreen': 'Start in fullscreen', 'settings.fullscreenHint': 'The game window fills the screen when it starts.',
  'settings.windowSize': 'Window size', 'settings.windowSizeHint': 'Width and height of the game window in pixels. Not used in fullscreen.',
  'settings.width': 'Width', 'settings.height': 'Height', 'settings.windowSizeInvalid': 'Enter whole numbers from 320 to 7680.',
  'settings.accounts.hint': 'Sign in with Microsoft, add an offline account or switch the account used to play.', 'settings.accounts.manage': 'Manage accounts',
  'psettings.group.general': 'General',
  'stats.emptyTitle': 'No playtime in the last {n} days', 'stats.empty': 'Play an instance and your sessions show up here.',
  'media.bothEmptyTitle': 'No worlds or screenshots yet', 'media.bothEmpty': 'Worlds appear after you play this instance once. Press F2 in game to take screenshots.',
} as const;

export const de12: Record<keyof typeof en12, string> = {
  'err.import.unsupported_curseforge': 'CurseForge-Packs werden nicht unterstützt. Nutze ein Modrinth-Pack (.mrpack) oder eine Prism- / MultiMC-Instanz.',
  'discover.type.datapack': 'Datenpakete',

  'profile.tab.mods': 'Mods', 'profile.tab.shaders': 'Shader', 'profile.tab.resourcepacks': 'Ressourcenpakete', 'profile.tab.datapacks': 'Datenpakete',
  'content.search.mod': 'Mods suchen', 'content.search.shader': 'Shader suchen', 'content.search.resourcepack': 'Ressourcenpakete suchen', 'content.search.datapack': 'Datenpakete suchen',
  'content.add.mod': 'Mods hinzufügen', 'content.add.shader': 'Shader hinzufügen', 'content.add.resourcepack': 'Ressourcenpakete hinzufügen', 'content.add.datapack': 'Datenpakete hinzufügen',
  'content.emptyTitle.mod': 'Noch keine Mods', 'content.emptyTitle.shader': 'Noch keine Shader', 'content.emptyTitle.resourcepack': 'Noch keine Ressourcenpakete', 'content.emptyTitle.datapack': 'Noch keine Datenpakete',
  'content.empty.mod': 'Füge Mods von Modrinth hinzu. Benötigte Abhängigkeiten siehst du, bevor etwas installiert wird.',
  'content.empty.shader': 'Füge Shader-Pakete von Modrinth hinzu. Sie brauchen eine Shader-Mod wie Iris, um zu laufen.',
  'content.empty.resourcepack': 'Füge Ressourcenpakete von Modrinth hinzu, um Texturen, Sounds und das Aussehen des Spiels zu ändern.',
  'content.empty.datapack': 'Füge Datenpakete von Modrinth hinzu. Sie gelten für die Welten dieser Instanz.',
  'content.updatesAvailable.mod.one': '{n} Mod-Update verfügbar', 'content.updatesAvailable.mod.other': '{n} Mod-Updates verfügbar',
  'content.updatesAvailable.shader.one': '{n} Shader-Update verfügbar', 'content.updatesAvailable.shader.other': '{n} Shader-Updates verfügbar',
  'content.updatesAvailable.resourcepack.one': '{n} Ressourcenpaket-Update verfügbar', 'content.updatesAvailable.resourcepack.other': '{n} Ressourcenpaket-Updates verfügbar',
  'content.updatesAvailable.datapack.one': '{n} Datenpaket-Update verfügbar', 'content.updatesAvailable.datapack.other': '{n} Datenpaket-Updates verfügbar',

  'settings.search.label': 'Einstellungen durchsuchen', 'settings.search.placeholder': 'Einstellungen suchen, zum Beispiel „Dark Mode“, „RAM“ oder „Backup“',
  'settings.search.clear': 'Suche leeren', 'settings.search.none': 'Nichts gefunden für „{q}“', 'settings.search.matched': 'Treffer: {alias}',
  'settings.fullscreen': 'Im Vollbild starten', 'settings.fullscreenHint': 'Das Spielfenster füllt beim Start den ganzen Bildschirm.',
  'settings.windowSize': 'Fenstergröße', 'settings.windowSizeHint': 'Breite und Höhe des Spielfensters in Pixeln. Im Vollbild nicht verwendet.',
  'settings.width': 'Breite', 'settings.height': 'Höhe', 'settings.windowSizeInvalid': 'Gib ganze Zahlen von 320 bis 7680 ein.',
  'settings.accounts.hint': 'Mit Microsoft anmelden, ein Offline-Konto hinzufügen oder das Konto zum Spielen wechseln.', 'settings.accounts.manage': 'Konten verwalten',
  'psettings.group.general': 'Allgemein',
  'stats.emptyTitle': 'Keine Spielzeit in den letzten {n} Tagen', 'stats.empty': 'Spiele eine Instanz, dann erscheinen deine Sitzungen hier.',
  'media.bothEmptyTitle': 'Noch keine Welten oder Screenshots', 'media.bothEmpty': 'Welten erscheinen, sobald du diese Instanz einmal gespielt hast. Drücke im Spiel F2 für Screenshots.',
};
