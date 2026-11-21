// Part 15: single info lines, background in all areas, new-instance button.
export const en15 = {
  'info.lastPlayed': 'Last played: {date}', 'info.never': 'Never played',
  'rail.newInstance': 'New instance', 'settings.onPage': 'On this page',
  'new.firstRun': 'Create your first instance',
  'home.createInstance': 'Create instance',
  'psettings.mangoUnsupported': 'Off (not supported)', 'psettings.mangoUnsupportedHint': 'There is no MangoConfig build for {version} with {loader}. It is only available for Fabric and Quilt on 1.20.5, 1.20.6 and 1.21 to 1.21.11.',
} as const;
export const de15: Record<keyof typeof en15, string> = {
  'info.lastPlayed': 'Zuletzt gespielt: {date}', 'info.never': 'Noch nie gespielt',
  'rail.newInstance': 'Neue Instanz', 'settings.onPage': 'Auf dieser Seite',
  'new.firstRun': 'Erstelle deine erste Instanz',
  'home.createInstance': 'Instanz erstellen',
  'psettings.mangoUnsupported': 'Aus (nicht unterstützt)', 'psettings.mangoUnsupportedHint': 'Für {version} mit {loader} gibt es keinen MangoConfig-Build. Er ist nur für Fabric und Quilt auf 1.20.5, 1.20.6 und 1.21 bis 1.21.11 verfügbar.',
} as const;
