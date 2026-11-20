# Ideen und Lücken: MangoClient im Vergleich zu anderen Launchern

Stand: 2026-11-20. Basis: CLAUDE.md (Feature scope, Network hosts, Design principles) und docs/ipc.md (Teil 1 bis 4). Die Recherche stützt sich überwiegend auf Sekundärquellen (Blogs, Vergleichsseiten, Foren). Nichts davon ist offiziell bestätigt, solange nicht "verifiziert" dasteht. Einträge mit (?) sind unsicher und vor einer Entscheidung gegen die offiziellen Changelogs bzw. Doku zu prüfen. Texte und Assets der Launcher werden nicht übernommen, alles ist eigene Zusammenfassung.

## 1. Kurzfassung

- MangoClient deckt den Kern bereits ab: Profile, Modrinth und CurseForge mit Install-Plan und Abhängigkeitsbestätigung, Modpack-Installation, Konten, Server, Logs, Stats, Auto-Java.
- Die größten Lücken gegenüber Prism, Modrinth App und HMCL/PCL2 liegen bei Import/Export fremder Instanzformate, Welten-Backups mit Snapshots vor Updates, Absturzanalyse, Profil-Gruppen/Tags und Skin-Bibliothek.
- Differenzierung ohne Social-Overkill: Sicherheit und Transparenz (Hashes, Vertrauensabzeichen, Diagnosepaket) sowie ein ruhiger, bildzentrierter Installationsfluss.
- Rechtlich kritisch sind drei Punkte: CurseForge-Key und Konkurrenzklausel, Microsoft-/Minecraft-Services-Freigabe der App-Registrierung, Mojang-Branding (siehe Abschnitt 5).

## 2. Vergleichstabelle

Legende: have = vorhanden laut Feature-Scope/IPC, planned = in IPC-Vertrag oder CLAUDE.md genannt, missing = nirgends erwähnt. Spalte "Launcher" nennt nur Quellen-gestützte oder plausible Angaben, (?) = nicht sicher belegt.

| Feature | MangoClient | Launcher mit Feature |
|---|---|---|
| Modrinth- und CurseForge-Suche, Install | have/planned (Teil 4) | Prism, Modrinth App, CurseForge, GDLauncher (?) |
| Abhängigkeitsdialog vor Install | planned (`install_plan`) | Modrinth App, Prism (?) |
| Instanz-Auswahl mit gesperrten Einträgen | planned (`install_targets`) | Modrinth App (?) |
| Direkte Modpack-Installation | planned (`modpack_install`) | Prism, FTB, ATLauncher, Technic, CurseForge |
| .mrpack/CurseForge-Zip Import | missing | Prism, Modrinth App (verifiziert über Sekundärquellen) |
| Export als .mrpack/Zip | missing | Prism, Modrinth App |
| Import fremder Instanzen (Prism, MultiMC, ATL, GDL) | missing | Modrinth App |
| Teilbare Instanz-Links/Codes | missing | keine verlässliche Quelle (Marktlücke) |
| Instanz duplizieren | have (`profile_duplicate`) | Prism, MultiMC |
| Gruppen/Tags/Icons | missing (Cover vorhanden) | Prism, Modrinth App (?) |
| Welten-Manager und Backup | missing | wenige, meist per Mod/Skript |
| Update-all mit Rollback | missing (Updates ja) | kaum ein Launcher |
| Absturzanalyse | missing | PCL2 (stark), HMCL (?) |
| Mod-Konflikterkennung | teilweise (`conflicts` im Plan) | PCL2 (?) |
| Mehrere Konten, Offline | have | Prism, HMCL, PCL2, TLauncher |
| Skin-Bibliothek/Garderobe, Cape wählen | missing (Skin-Wechsel ja) | Lunar (Cosmetics), Modrinth App (?) |
| Server-Liste, Ping, Quick Join | have | Lunar, Modrinth App (?) |
| Optionen/Server/Packs zwischen Instanzen syncen | missing | Lunar, Modrinth App (?) |
| JVM-Preset, RAM-Berater | teilweise (Preset potato/balanced/quality) | HMCL (?) |
| Freunde/Aktivität | missing | Lunar, Dawn/Feather |
| Discord Presence | missing | mehrere Clients |
| Command Palette, Shortcuts | missing | keine Quelle gefunden |
| Tray/Mini-Modus | teilweise (Tray geplant) | Lunar, Modrinth App (?) |
| Release-Kanäle stable/beta | missing | Prism, HMCL (?) |
| Diagnosepaket | teilweise (mclo.gs-Share) | Prism (Log-Upload) |
| Parental Controls | missing | keiner nennenswert |
| Hash/Malware-Hinweise | teilweise (SHA-512) | Modrinth (serverseitige Prüfung, ?) |
| News/Changelog-Feed | missing | Lunar, CurseForge, FTB |
| i18n | have (EN/DE) | PCL2, HMCL (viele Sprachen) |

## 3. Lücken nach Wirkung und Aufwand, Top-15-Roadmap

Wirkung H/M/N, Aufwand S/M/L.

| # | Idee | Wirkung | Aufwand | Begründung |
|---|---|---|---|---|
| 1 | Import .mrpack und CurseForge-Zip per Drag-and-Drop | H | M | Wichtigster Umstiegspfad, Format bereits durch Modpack-Plan weitgehend abgedeckt |
| 2 | Export als .mrpack (Overrides, Hashes) | H | M | Gegenstück zu 1, ermöglicht Teilen ohne Cloud |
| 3 | Snapshot vor jedem Update/Install, Rollback mit einem Klick | H | M | Löst die größte Angst bei "Update all"; echtes Alleinstellungsmerkmal |
| 4 | Welten-Backup (automatisch vor Start/Update, manuelle Wiederherstellung) | H | M | Datenverlust ist der teuerste Fehler, fast ohne Konkurrenz |
| 5 | Absturzanalyse (Muster für Mod-Konflikt, falsche Java, zu wenig RAM) | H | M | PCL2 gilt als Referenz (?), baut auf vorhandener Log-Pipeline auf |
| 6 | Diagnosepaket exportieren (Logs, Mod-Liste, Config ohne Tokens) | M | S | Support-Aufwand sinkt, Privacy-Regeln sind klar definierbar |
| 7 | Gruppen, Tags und Suche in der Bibliothek | M | S | Ab etwa 10 Profilen nötig, Prism-Nutzer erwarten es |
| 8 | Command Palette (Strg+K) und Tastenkürzel | M | S | Passt zur Zielgruppe "Power-User", wenig Backend |
| 9 | Skin-Bibliothek und Cape-Auswahl | M | M | 3D-Viewer ist schon da, Mojang-Endpunkte für Skin/Cape bekannt |
| 10 | Vertrauensabzeichen und Quellenanzeige (Modrinth verifiziert, Hash geprüft, lokale Jar unbekannt) | M | S | Sicherheit sichtbar machen, nutzt vorhandene Daten |
| 11 | Import fremder Instanzen (Prism/MultiMC) | M | M | Erleichtert Wechsel; Prism-Format ist dokumentiert |
| 12 | Release-Kanal stable/beta im Updater | M | S | Sinnvoll vor dem ersten Rust-Release (offene Entscheidung 1) |
| 13 | Speicher-Berater (RAM-Vorschlag je Modanzahl und System) | M | S | Ergänzt Preset-Feld, ohne neue Hosts |
| 14 | Optionen/Server/Resource-Packs zwischen Instanzen synchronisieren (lokal, symlink-frei) | M | M | Mehrere Quellen berichten es für Lunar und Modrinth App (?), lokal ohne Cloud lösbar |
| 15 | Teilbarer Instanz-Link (`mangoclient://` mit Modrinth-/CF-Projekt-IDs, keine Dateien) | M | L | Marktlücke; erst nach 1, 2 und einer Entscheidung zum Hosting |

Bewusst zurückgestellt: Freunde und Aktivität (braucht Server-Ausbau, `server/` ist eingefroren), Cloud-Sync (Hosting, DSGVO), Parental Controls (geringer Bedarf, hoher Aufwand), Discord Presence (nur zusätzliche Abhängigkeit, später optional).

## 4. UX-Muster zum Übernehmen oder Vermeiden

**Installationsfluss (Wunsch des Nutzers)**

1. **Abhängigkeitsdialog.** Beim Klick auf "Installieren" erst `install_plan` zeigen: Hauptmod, Pflicht-Abhängigkeiten (vorangekreuzt, nicht abwählbar), optionale (abwählbar, standardmäßig aus), bereits installierte (ausgegraut, mit Version), Gesamtgröße. Ein Button "Installieren (n Dateien, x MB)". Nie still nachladen (entspricht der Regel in Teil 4).
2. **Instanz-Popup.** Liste aller Profile mit Cover. Nicht nutzbare Profile bleiben sichtbar, aber dunkel und deaktiviert, mit einer Zeile Grund ("braucht Fabric, Profil nutzt Forge", "bereits installiert 1.2.3"). Daten liefert `install_targets`. Zusätzlich "Neues Profil erstellen" mit voreingestellter Version und Loader. Grund als Tooltip und für Screenreader als Text, nicht nur Farbe.
3. **Direkter Modpack-Install.** Von der Detailseite (Modrinth und CurseForge) ein Knopf "Als neues Profil installieren". Vorschau (`modpack_plan`): Name, MC-Version, Loader, Dateizahl, Größe, blockierte Dateien. Blockierte CurseForge-Dateien als Liste mit Website-Link und Fortschrittsanzeige "2 von 3 manuell geladen"; Installation erst fortsetzen, wenn die Dateien im Ordner erkannt werden (Dateiwächter, Hash-Prüfung).
4. **Drag-and-drop.** Eine Datei (.jar, .mrpack, .zip) auf das Fenster ziehen öffnet denselben Dialog wie 1 oder 3. Jar ohne Metadaten: Hash gegen Modrinth nachschlagen, sonst als "lokal, unbekannte Quelle" markieren.
5. **Update-all.** Vorher Zusammenfassung (Version alt zu neu, Änderungsnotiz, Größe), automatischer Snapshot, nach Start-Fehler Angebot "Rückgängig".

**Allgemein übernehmen:** Tray-Modus mit Statuspille; Konsole mit Filter nach Level; Absturz-Karte mit einer Hauptursache statt Logwand; leere Zustände mit einer klaren Aktion.

**Vermeiden:** Werbung und Premium-Währung im Launcher (Dawn/Feather, laut Sekundärquelle); Cosmetics nur für eigene Nutzer; vermischte Suchergebnisse aus zwei Quellen (bereits ausgeschlossen); stille Bundles/Affiliate-Installer wie bei Drittanbieter-Launchern; Telemetrie ohne Schalter; Buchstaben-Platzhalter statt Bildern (Designregel).

## 5. Rechtliches und ToS

| Punkt | Stand | Sicherheit |
|---|---|---|
| CurseForge-API-Key | Schlüssel ist laut Bedingungen an den Entwickler gebunden und nicht weitergebbar; Antrag wird von Overwolf geprüft. Ein im Build eingebetteter Key für alle Nutzer ist daher rechtlich zu klären. | Quelle ist eine Support-Seite, Aktualität (?) |
| CurseForge Konkurrenzklausel | Bedingungen untersagen laut Kopie Produkte, die CurseForge direkt oder indirekt Konkurrenz machen. Ein Launcher mit CF-Browser kann darunter fallen. Vor Einbau schriftlich klären. | unsicher, Rechtsprüfung nötig |
| Blockierte Mods | Autoren können API-Verteilung abschalten (`allowModDistribution=false`), Download-URL dann leer (?). Konsequenz wie im Vertrag: nur Website-Link, nie umgehen. | Verhalten verifiziert über Doku und Sekundärquellen |
| Caching/Proxy der CF-API | Verbot von Caching und Proxy wird von Dritten berichtet, nicht gegen die aktuellen Bedingungen geprüft. Betrifft den lokalen Metadaten-Cache. | unsicher (?) |
| Microsoft-Login | Entra-App-Registrierung allein reicht oft nicht: neue Apps erhalten laut Community-Berichten beim Minecraft-Login HTTP 403 "Invalid app registration", bis eine Freigabe erfolgt. Offizieller Prozess nicht gefunden; Mojang/Microsoft-Support direkt fragen. | Berichte verifiziert, Prozess unsicher |
| Mojang-Branding | Rechte an Name, Marke und Assets liegen bei Mojang/Microsoft. Produkte dürfen keine Zugehörigkeit oder Billigung nahelegen und sollen einen Disclaimer tragen. Passt zur bestehenden Regel "keine Mojang-Texturen". Name "MangoClient" und Icon sind unkritisch, "Minecraft" im Namen vermeiden. | verifiziert (Usage Guidelines) |
| EULA | Kein Weiterverkauf, kein Umgehen des Kaufs. Offline-Konten sind Funktion, aber kein Werbeargument für Raubkopien; Hinweistext im Onboarding erwägen. | allgemein bekannt, Wortlaut nicht geprüft |
| Modrinth-API | Eindeutiger User-Agent mit Kontakt nötig; keine Weiterverteilung von Cache-Inhalten Dritter. | Praxis, (?) |

## 6. Quellen

- Vergleich Modrinth App, CurseForge, Prism: https://minecrafthub.io/articles/minecraft-mod-launchers-compared
- Launcher-Vergleich 2026: https://space-node.net/blog/curseforge-vs-modrinth-vs-prism-launcher-2026
- .mrpack öffnen: https://minecrafthub.io/guides/open-mrpack-file-minecraft
- Modpack teilen (CurseForge, Modrinth, Prism): https://www.savesync.games/blog/how-to-share-a-modpack-with-friends/
- Prism, Kopieren und Exportieren (Code-Doku): https://deepwiki.com/PrismLauncher/PrismLauncher/3.4-copying-and-exporting-instances
- Prism Überblick: https://deepwiki.com/PrismLauncher/PrismLauncher
- PCL2/HMCL/Prism (chinesisch): https://pcl2.ijinshan.com/pages/concepts/minecraft-launchers/
- HMCL Alternativen: https://alternativeto.net/software/hmcl/
- Lunar Client Recap Februar 2026: https://www.lunarclient.com/news/lunar-client-monthly-recap-february-2026
- Clients 2026 (Lunar, Feather/Dawn, Badlion): https://blog.sparkedhost.com/minecraft/best-minecraft-clients-2026
- CurseForge 3rd Party API Terms: https://support.curseforge.com/support/solutions/articles/9000207405-curse-forge-3rd-party-api-terms-and-conditions
- CurseForge API Key beantragen: https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key
- CurseForge REST-Doku: https://docs.curseforge.com/rest-api/
- Minecraft Services 403 "Invalid app registration": https://learn.microsoft.com/en-us/answers/questions/5989335/minecraft-services-returns-http-403-invalid-app-re
- Weitere Berichte dazu: https://learn.microsoft.com/en-us/answers/questions/5945628/minecraft-services-returns-invalid-app-registratio
- Minecraft Usage Guidelines: https://www.minecraft.net/en-us/usage-guidelines
- Mojang API (Wiki): https://minecraft.wiki/w/Mojang_API

Nicht abgedeckt, da Suche ohne belastbare Treffer: GDLauncher Carbon, FTB App, ATLauncher, Technic, SKLauncher/TLauncher im Detail. Vor Planung dieser Vergleichszeilen nachrecherchieren.
