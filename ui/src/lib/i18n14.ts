// Part 14: context menus, rename, double-click to open, the keyboard section in Settings, the reworked Skins page.
export const en14 = {
  /* context menus */
  'menu.open': 'Open', 'menu.rename': 'Rename', 'menu.openFolder': 'Open folder', 'menu.deleteRunning': 'Stop the game first.',
  'menu.duplicated': '{name} created.', 'menu.copied': 'Copied: {what}', 'menu.copyFailed': 'Could not copy to the clipboard.',
  'menu.enable': 'Enable', 'menu.disable': 'Disable', 'menu.updateTo': 'Update to {v}', 'menu.projectPage': 'Open project page',
  'menu.copyName': 'Copy name', 'menu.copyAddress': 'Copy address', 'menu.pingNow': 'Ping now',
  'menu.requiredReason': 'Other items need it. Disable those first.', 'menu.localNoPage': 'A local file cannot be updated.',
  'menu.busyReason': 'Another action is still running.', 'menu.gameRunningReason': 'Stop the game first.',
  'menu.instance': 'Options for {name}',

  /* rename */
  'rename.title': 'Rename instance', 'rename.label': 'Name', 'rename.save': 'Rename', 'rename.done': 'Renamed to {name}.',
  'rename.err.empty': 'Enter a name.', 'rename.err.long': 'At most 128 characters.', 'rename.err.control': 'The name contains characters that are not allowed.',
  'rename.inline': 'Rename {name}', 'rename.hint': 'Double-click the title or press F2 to rename.',

  /* keyboard */
  'keys.group.instances': 'Instances', 'keys.group.search': 'Search and dialogs', 'keys.group.window': 'Window',
  'keys.rename': 'Rename the open or selected instance', 'keys.openCard': 'Open the focused instance', 'keys.contextMenu': 'Open the context menu',
  'keys.maximize': 'Maximize or restore the window', 'keys.settingsHint': 'Every shortcut of the launcher. Shortcuts with Ctrl or Cmd also work while you type.',
  'keys.sectionLabel': 'Keyboard shortcuts', 'keys.openSheet': 'Show as overlay',

  /* skins page */
  'skins.toolbar': 'Viewer options', 'skins.capeAnim': 'Cape animation', 'skins.resetView': 'Reset view', 'skins.saving': 'Saving...',
  'skins.capeFailed': 'The cape was not changed.', 'skins.capeLoading': 'Loading capes...', 'skins.skinCached': 'Showing the saved skin. Updating...',
  'skins.capesTimeout': 'The capes did not load in time.', 'skins.pose.label': 'Pose',
  'err.skin.cape_not_applied': 'Mojang did not apply the cape. The previous cape is back. Try again in a moment.',
  'err.skin.rate_limited': 'Too many changes in a row. Try again in {s} seconds.', 'err.skin.rate_limited_nosec': 'Too many changes in a row. Try again in a moment.',
  'skins.capeRelogin': 'Sign in again to change your cape.', 'err.skin.timeout': 'The skin service did not answer in time. Check your connection and try again.',
} as const;

export const de14: Record<keyof typeof en14, string> = {
  'menu.open': 'Öffnen', 'menu.rename': 'Umbenennen', 'menu.openFolder': 'Ordner öffnen', 'menu.deleteRunning': 'Beende zuerst das Spiel.',
  'menu.duplicated': '{name} erstellt.', 'menu.copied': 'Kopiert: {what}', 'menu.copyFailed': 'Konnte nicht in die Zwischenablage kopieren.',
  'menu.enable': 'Aktivieren', 'menu.disable': 'Deaktivieren', 'menu.updateTo': 'Auf {v} aktualisieren', 'menu.projectPage': 'Projektseite öffnen',
  'menu.copyName': 'Namen kopieren', 'menu.copyAddress': 'Adresse kopieren', 'menu.pingNow': 'Jetzt anpingen',
  'menu.requiredReason': 'Andere Einträge brauchen es. Deaktiviere zuerst diese.', 'menu.localNoPage': 'Eine lokale Datei lässt sich nicht aktualisieren.',
  'menu.busyReason': 'Eine andere Aktion läuft noch.', 'menu.gameRunningReason': 'Beende zuerst das Spiel.',
  'menu.instance': 'Optionen für {name}',

  'rename.title': 'Instanz umbenennen', 'rename.label': 'Name', 'rename.save': 'Umbenennen', 'rename.done': 'Umbenannt in {name}.',
  'rename.err.empty': 'Gib einen Namen ein.', 'rename.err.long': 'Höchstens 128 Zeichen.', 'rename.err.control': 'Der Name enthält Zeichen, die nicht erlaubt sind.',
  'rename.inline': '{name} umbenennen', 'rename.hint': 'Doppelklick auf den Titel oder F2 zum Umbenennen.',

  'keys.group.instances': 'Instanzen', 'keys.group.search': 'Suche und Dialoge', 'keys.group.window': 'Fenster',
  'keys.rename': 'Geöffnete oder ausgewählte Instanz umbenennen', 'keys.openCard': 'Fokussierte Instanz öffnen', 'keys.contextMenu': 'Kontextmenü öffnen',
  'keys.maximize': 'Fenster maximieren oder wiederherstellen', 'keys.settingsHint': 'Alle Tastenkürzel des Launchers. Kürzel mit Strg oder Cmd funktionieren auch beim Tippen.',
  'keys.sectionLabel': 'Tastenkürzel', 'keys.openSheet': 'Als Übersicht anzeigen',

  'skins.toolbar': 'Anzeige-Optionen', 'skins.capeAnim': 'Umhang-Animation', 'skins.resetView': 'Ansicht zurücksetzen', 'skins.saving': 'Wird gespeichert...',
  'skins.capeFailed': 'Der Umhang wurde nicht geändert.', 'skins.capeLoading': 'Umhänge werden geladen...', 'skins.skinCached': 'Zeigt den gespeicherten Skin. Wird aktualisiert...',
  'skins.capesTimeout': 'Die Umhänge haben nicht rechtzeitig geladen.', 'skins.pose.label': 'Pose',
  'err.skin.cape_not_applied': 'Mojang hat den Umhang nicht übernommen. Der vorherige ist wieder aktiv. Versuche es gleich noch einmal.',
  'err.skin.rate_limited': 'Zu viele Änderungen hintereinander. Versuche es in {s} Sekunden erneut.', 'err.skin.rate_limited_nosec': 'Zu viele Änderungen hintereinander. Versuche es gleich noch einmal.',
  'skins.capeRelogin': 'Melde dich erneut an, um deinen Umhang zu ändern.', 'err.skin.timeout': 'Der Skin-Dienst hat nicht rechtzeitig geantwortet. Prüfe die Verbindung und versuche es erneut.',
};
