// Mock of the Part 9 import / export / crash analysis / diagnostics commands: two import kinds, a rejected CurseForge pack, export warnings,
// three crash scenarios (`?mockCrash=java|oom|dependency|none`), forced import errors (`?mockImportError=<code>`) and a drop simulator.
import type { Api } from './api';
import { baseName, stem } from './portability';
import type { ApiEvents, CrashReport, ImportPlan, Loader, ProfileView } from './types';

export type PartPortability = Pick<Api, 'pickImportFile' | 'pickSaveFile' | 'instanceExport' | 'instanceImportPlan' | 'instanceImport' | 'crashAnalyze' | 'diagnosticsExport'>;
export type MockCrash = 'java' | 'oom' | 'dependency' | 'none';
export const MOCK_CRASHES: readonly MockCrash[] = ['java', 'oom', 'dependency', 'none'];

export interface PortabilityDev {
  /** Plays `files:dropped` (the shell emits it with real paths). */
  drop(paths: string[]): void;
  /** Sample files: a Modrinth pack, a (rejected) CurseForge pack and a Prism instance. */
  samples: string[];
}

export interface PortabilityCtx {
  scale: number;
  emit: <K extends keyof ApiEvents>(name: K, payload: ApiEvents[K]) => void;
  emitDrop: (paths: string[]) => void;
  profiles: () => ProfileView[];
  find: (id: string) => ProfileView;
  addProfile: (p: ProfileView) => void;
  crash?: MockCrash;
  /** `?mockImportError=<code>`: every plan fails with this code. */
  importError?: string;
  /** `?mockExportError=<code>` */
  exportError?: string;
}

const MB = 1048576;
const err = (code: string, message = code) => ({ code, message, needsRelogin: false });
const uuid = () => globalThis.crypto?.randomUUID?.() ?? 'xxxxxxxx-xxxx-4xxx-8xxx-xxxxxxxxxxxx'.replace(/x/g, () => Math.floor(Math.random() * 16).toString(16));

export const SAMPLES = [
  '/home/mango/Downloads/Fabulously Optimized.mrpack',
  '/home/mango/Downloads/All the Mods 9 (CurseForge).zip',
  '/home/mango/Downloads/Prism Survival Instance.zip',
];

/** The plan a path stands for, or the error the service would raise. */
export function planFor(path: string): ImportPlan {
  const file = baseName(path);
  const low = file.toLowerCase();
  const name = stem(path);
  if (low.includes('huge')) throw err('import.too_large');
  if (low.includes('broken')) throw err('import.not_an_archive');
  if (low.includes('unsafe')) throw err('import.unsafe_archive');
  if (low.includes('curseforge') || low.includes('(cf)')) throw err('import.unsupported_curseforge');
  if (low.endsWith('.mrpack')) {
    return { kind: 'mrpack', name, mcVersion: '1.21.1', loader: 'fabric' as Loader, loaderVersion: '0.16.10', fileCount: 142, sizeBytes: 218 * MB, ram: null, appliedRamMb: null };
  }
  if (low.includes('prism') || low.includes('multimc') || low.includes('instance')) {
    return { kind: 'prism', name, mcVersion: '1.20.1', loader: 'forge', loaderVersion: '47.3.0', fileCount: 96, sizeBytes: null, ram: { recommendedMb: 6144, minMb: 2048, maxMb: null, source: 'prism' }, appliedRamMb: 6144 };
  }
  throw err('import.unsupported_format');
}

function crashReport(kind: MockCrash, logId: string | null): CrashReport {
  const base = { sourceLog: logId };
  if (kind === 'none') {
    return { ...base, summary: 'The log does not show a known problem.', causes: [], javaVersion: '21.0.5', mcVersion: '1.21.4', excerpt: '[22:14:03] [Render thread/ERROR]: Unexpected shutdown\n[22:14:03] [Render thread/INFO]: Stopping!' };
  }
  if (kind === 'java') {
    return {
      ...base, summary: 'The game could not start: this version needs Java 21 but Java 17 was used.', javaVersion: '17.0.9', mcVersion: '1.21.4',
      causes: [
        { code: 'java_version', title: 'Wrong Java version', detail: 'java.lang.UnsupportedClassVersionError: class file version 65.0, this runtime only recognizes up to 61.0', confidence: 'high',
          suggestions: ['Set the Java path back to automatic.', 'Use Java 21 or newer.'], relatedMods: [] },
        { code: 'java_path', title: 'The Java path does not work', detail: 'A custom Java path is set for this instance.', confidence: 'low', suggestions: ['Clear the Java path in the instance settings.'], relatedMods: [] },
      ],
      excerpt: 'Error: LinkageError occurred while loading main class net.fabricmc.loader.impl.launch.knot.KnotClient\n\tjava.lang.UnsupportedClassVersionError: net/fabricmc/loader/impl/launch/knot/KnotClient has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0',
    };
  }
  if (kind === 'oom') {
    return {
      ...base, summary: 'Minecraft ran out of memory while loading chunks.', javaVersion: '21.0.5', mcVersion: '1.21.4',
      causes: [
        { code: 'out_of_memory', title: 'The game ran out of memory', detail: 'java.lang.OutOfMemoryError: Java heap space (1024 MB allowed)', confidence: 'high',
          suggestions: ['Raise the memory of this instance.', 'Lower the render distance.'], relatedMods: ['Distant Horizons'] },
        { code: 'jvm_args', title: 'Invalid Java arguments', detail: '-Xmx1G set in the Java arguments overrides the memory setting.', confidence: 'medium', suggestions: ['Remove -Xmx from the Java arguments.'], relatedMods: [] },
      ],
      excerpt: '[22:14:07] [Render thread/ERROR]: Unhandled game exception\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.class_2818.method_12206(LevelChunk.java:211)\n\tat dev.hhs.dh.render.LodRenderer.render(LodRenderer.java:344)',
    };
  }
  return {
    ...base, summary: 'A mod could not start because a required mod is missing.', javaVersion: '21.0.5', mcVersion: '1.21.4',
    causes: [
      { code: 'mod_dependency', title: 'A required mod is missing', detail: 'Mod "Xaero\'s Minimap" requires Cloth Config API 15.0.0 or later, which is not installed.', confidence: 'high',
        suggestions: ['Install Cloth Config API.', 'Update the mod and its dependency together.'], relatedMods: ["Xaero's Minimap", 'Cloth Config API'] },
      { code: 'duplicate_mods', title: 'The same mod is installed twice', detail: 'sodium-fabric-0.6.1 and sodium-fabric-0.6.0 both provide "sodium".', confidence: 'medium',
        suggestions: ['Remove the older Sodium jar.'], relatedMods: ['Sodium'] },
      { code: 'wrong_mc_version', title: 'A mod is built for another Minecraft version', detail: 'Mod "Zoomify" targets Minecraft 1.20.6.', confidence: 'low', suggestions: ['Update Zoomify.'], relatedMods: ['Zoomify'] },
    ],
    excerpt: 'net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!\nA potential solution has been determined:\n\t - Install cloth-config, version 15.0.0 or later.\nMore details:\n\t - Mod \'Xaero\'s Minimap\' (xaerominimap) 24.5.0 requires version 15.0.0 or later of cloth-config, which is missing!',
  };
}

export function createPortabilityMock(c: PortabilityCtx): { api: PartPortability; dev: PortabilityDev } {
  const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms * c.scale));
  let picked = 0;
  const absolute = (p: string) => /^\//.test(p) || /^[A-Za-z]:[\\/]/.test(p);

  const api: PartPortability = {
    async pickImportFile() { return SAMPLES[picked++ % SAMPLES.length]; }, // the browser has no native dialog: cycle the three kinds
    async pickSaveFile({ defaultName }) { return `/home/mango/Documents/${defaultName}`; },
    async instanceImportPlan({ filePath }) {
      await sleep(700);
      if (c.importError) throw err(c.importError);
      if (!absolute(filePath)) throw err('path.not_granted');
      return planFor(filePath);
    },
    async instanceImport({ filePath, name }) {
      const plan = planFor(filePath);
      if (c.importError) throw err(c.importError);
      const id = uuid();
      const label = baseName(filePath);
      const steps = 8;
      for (let i = 0; i <= steps; i++) {
        c.emit('install:progress', { profileId: id, projectId: 'import', done: i, total: steps + 1, label });
        await sleep(380);
      }
      const p: ProfileView = {
        id, name: (name ?? plan.name).trim() || plan.name, mcVersion: plan.mcVersion, loader: plan.loader, loaderVersion: plan.loaderVersion, color: null, coverUrl: null, ram: plan.appliedRamMb, ramSource: plan.appliedRamMb === null ? 'default' : 'modpack', ramHint: plan.ram, effectiveRamMb: plan.appliedRamMb ?? 4096,
        javaArgs: '', javaPath: null, window: { mode: 'default', width: null, height: null }, mangoConfig: null, group: null, tags: [], lastPlayed: null, playTimeMs: 0, created: Date.now(),
        modCount: plan.kind === 'prism' ? 31 : Math.max(0, plan.fileCount - 12), icon: null, iconSource: null, autoBlock: null, background: { kind: 'auto' },
      };
      c.addProfile(p);
      c.emit('install:progress', { profileId: id, projectId: 'import', done: steps + 1, total: steps + 1, label });
      return structuredClone(p);
    },
    async instanceExport({ profileId, includeConfig, outPath }) {
      const p = c.find(profileId);
      if (c.exportError) throw err(c.exportError);
      if (!absolute(outPath)) throw err('export.invalid_path');
      await sleep(1800);
      const warnings: string[] = [];
      if (p.modCount > 0) {
        warnings.push('Packed into the file because Modrinth does not host them: custom-hud-1.2.jar, my-keybinds.jar.');
      }
      if (includeConfig) warnings.push('options.txt: removed the "lastServer" entry.');
      return { path: outPath, sizeBytes: Math.round((p.modCount * 5.2 + 3 + (includeConfig ? 1.4 : 0)) * MB), warnings };
    },
    async crashAnalyze({ profileId, logId }) {
      const p = c.find(profileId);
      await sleep(900);
      if (p.playTimeMs === 0) return { summary: '', causes: [], javaVersion: null, mcVersion: null, excerpt: '', sourceLog: null };
      return structuredClone(crashReport(c.crash ?? 'java', logId ?? 'crash-2026-11-16_22.14.07-client.txt'));
    },
    async diagnosticsExport({ profileId, outPath }) {
      const p = c.find(profileId);
      await sleep(1400);
      if (!absolute(outPath)) throw err('export.invalid_path');
      return {
        path: outPath, sizeBytes: Math.round((0.18 + p.modCount * 0.004) * MB),
        warnings: ['The latest log was cut to its last 5 MB.', 'No crash report found, the latest log was used.'].slice(0, p.playTimeMs ? 2 : 1),
      };
    },
  };
  return { api, dev: { drop: (paths) => c.emitDrop(paths), samples: [...SAMPLES, '/home/mango/Downloads/broken.zip', '/home/mango/Downloads/notes.txt'] } };
}
