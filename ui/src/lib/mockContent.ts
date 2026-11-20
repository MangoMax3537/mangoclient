// Part 2 of the mock backend: content, discover, servers, media, logs, stats, storage, updates (docs/ipc.md Part 2).
import type { Api } from './api';
import { fallbackImage } from './fallbackImage';
import { CONTENT, DEPS, FULL_PROFILE, rng, rowOf, TABLES, titleOf } from './mockData';
import type {
  ApiEvents, ContentItem, ContentType, LogEntry, ProfileView, ProjectDetail, ProjectHit, ProjectType, ProjectVersion, Screenshot,
  SearchParams, ServerEntry, ServerStatus, StatsSummary, StorageKey, StorageUsage, UpdateInfo, UpdateStatus,
} from './types';

export interface MockCtx {
  now: number;
  scale: number;
  emit: <K extends keyof ApiEvents>(name: K, payload: ApiEvents[K]) => void;
  profiles: () => ProfileView[];
  addProfile: (p: ProfileView) => void;
  find: (id: string) => ProfileView;
  gameActive: () => boolean;
}

export type Part2 = Pick<Api,
  'contentList' | 'contentToggle' | 'contentUninstall' | 'contentCheckUpdates' | 'contentOpenFolder' | 'modrinthSearch' | 'modrinthProject'
  | 'modrinthVersions' | 'modrinthInstall' | 'modrinthInstallModpack' | 'serversList' | 'serversPingAll' | 'serversPing' | 'screenshotsList'
  | 'screenshotAction' | 'logsList' | 'logsRead' | 'logsUpload' | 'logsDelete' | 'statsSummary' | 'storageUsage' | 'storageClear'
  | 'updateStatus' | 'updateCheck' | 'updateInstall'>;

const DAY = 86400000;
const fail = (code: string, message = code) => ({ code, message, needsRelogin: false });
const hash = (s: string) => { let h = 2166136261; for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619); return h >>> 0; };

/** Mod-like catalog entry -> ProjectHit (deterministic metadata). */
function toHit(id: string, now: number): ProjectHit {
  const r = rowOf(id);
  if (!r) throw fail('project_not_found', `no project ${id}`);
  const [, title, author, description, cats] = r.row;
  const h = hash(id);
  const type = r.type;
  const downloads = Math.floor(48_000_000 / Math.pow(r.index + 1, 0.95) + (h % 9000));
  const gv = ['1.21.4', '1.21.3', '1.21.1', '1.21', '1.20.6', '1.20.4', '1.20.1'];
  const gameVersions = gv.filter((_, i) => i === 0 ? h % 10 < 8 : (h >> i) % 3 !== 0);
  const loaders = type === 'shader' ? ['iris', 'optifine'] : type === 'resourcepack' ? ['minecraft']
    : ['fabric', ...(h % 3 ? ['quilt'] : []), ...(h % 4 === 0 ? ['forge', 'neoforge'] : [])];
  return {
    projectId: id, slug: id, title, author, description, type, iconUrl: null, downloads, follows: Math.floor(downloads / 38),
    categories: cats.split(','), gameVersions: gameVersions.length ? gameVersions : ['1.21.1'], loaders,
    dateModified: new Date(now - (1 + (h % 160)) * DAY).toISOString(), installedIn: [],
  };
}

const SERVERS: ServerEntry[] = [
  { id: 'srv-mango', name: 'MangoMC Network', address: 'play.mangomc.example', partner: true, starred: true, iconUrl: null },
  { id: 'srv-bedwars', name: 'Bedwars Arena', address: 'bw.arena.example', partner: true, starred: false, iconUrl: null },
  { id: 'srv-hollow', name: 'Iron Hollow SMP', address: 'smp.ironhollow.example:25570', partner: false, starred: true, iconUrl: null },
  { id: 'srv-skyblock', name: 'Skyblock Realms', address: 'sky.realms.example', partner: false, starred: false, iconUrl: null },
  { id: 'srv-creative', name: 'Creative Plots', address: 'plots.example.net', partner: false, starred: false, iconUrl: null },
  { id: 'srv-old', name: 'Old Survival 1.8', address: '192.168.1.40', partner: false, starred: false, iconUrl: null },
];
const STATUS: Record<string, ServerStatus> = {
  'srv-mango': { id: 'srv-mango', online: true, ping: 38, motd: '§6§lMangoMC §r§7Bedwars, Skyblock, Survival', players: { online: 1284, max: 5000 }, version: '1.21.4' },
  'srv-bedwars': { id: 'srv-bedwars', online: true, ping: 61, motd: '§aSeason 14 started! Queue now.', players: { online: 342, max: 1000 }, version: '1.8.9 - 1.21.4' },
  'srv-hollow': { id: 'srv-hollow', online: true, ping: 14, motd: 'Iron Hollow, whitelisted friends only', players: { online: 3, max: 12 }, version: '1.21.4' },
  'srv-skyblock': { id: 'srv-skyblock', online: false, ping: null, motd: '', players: null, version: null },
  'srv-creative': { id: 'srv-creative', online: true, ping: 112, motd: 'Free plots, no ranks needed.', players: { online: 57, max: 200 }, version: '1.21.1' },
  'srv-old': { id: 'srv-old', online: false, ping: null, motd: '', players: null, version: null },
};

const LOG_LINES = [
  '[Datafixer Bootstrap/INFO]: 238 Datafixer optimizations took 164 milliseconds',
  '[main/INFO]: Loading Minecraft 1.21.4 with Fabric Loader 0.16.10',
  '[main/INFO]: Loading {n} mods',
  '[main/WARN]: Config file sodium-options.json was outdated, rewriting',
  '[Render thread/INFO]: Setting user: MangoMax',
  '[Render thread/INFO]: Backend library: LWJGL version 3.3.3-snapshot',
  '[Render thread/WARN]: Missing sound for event: minecraft:item.goat_horn.play',
  '[Worker-Main-3/INFO]: Preparing spawn area: 84%',
  '[Server thread/INFO]: MangoMax joined the game',
  '[Render thread/INFO]: [CHAT] <Gast_Steve> gg',
  '[Render thread/INFO]: Saving screenshot 2026-11-12_21.04.33.png',
  '[Render thread/ERROR]: Failed to load texture minecraft:textures/entity/missing.png',
  '[IO-Worker-2/INFO]: Chunk system saved 128 chunks',
];
const CRASH = (mc: string) => `---- Minecraft Crash Report ----
// Why did you do that?

Time: 2026-11-16 22:14:07
Description: Rendering screen

java.lang.NullPointerException: Cannot invoke "net.minecraft.class_1058.method_4577()" because "sprite" is null
\tat net.minecraft.class_332.method_52707(DrawContext.java:512)
\tat net.minecraft.class_437.method_25394(Screen.java:208)
\tat net.minecraft.class_757.method_3192(GameRenderer.java:861)
\tat net.minecraft.class_310.method_1523(MinecraftClient.java:1262)
Caused by: java.lang.IllegalStateException: Atlas not stitched
\tat net.minecraft.class_1059.method_4608(SpriteAtlasTexture.java:91)
\t... 4 more

-- Head --
Thread: Render thread
Stacktrace:
\tat net.minecraft.class_332.method_52707(DrawContext.java:512)

-- System Details --
Minecraft Version: ${mc}
Operating System: Linux (amd64) version 6.12
Java Version: 21.0.5, Eclipse Adoptium
Memory: 1480291216 bytes (1411 MiB) / 6442450944 bytes (6144 MiB) up to 6442450944 bytes (6144 MiB)
Fabric Mods:
\tfabric-api: Fabric API 0.119.4+1.21.4
\tsodium: Sodium 0.6.9
`;

export function createContentApi(ctx: MockCtx): Part2 {
  const { now } = ctx;
  const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms * ctx.scale));

  /* ---------- content per profile ---------- */
  const lists = new Map<string, ContentItem[]>();
  function build(id: string, ids: string[]): ContentItem[] {
    return recompute(ids.map((pid, i) => {
      const m = CONTENT[pid];
      const type: ContentType = m?.type ?? 'mod';
      const jar = type === 'mod' ? '.jar' : '.zip';
      return {
        projectId: pid, slug: pid, title: titleOf(pid), type, versionId: `${pid}-v1`, versionNumber: m?.ver ?? '1.0.0',
        filename: `${pid}-${m?.ver ?? '1.0.0'}${jar}`, enabled: pid !== 'dynamic-fps',
        local: false, iconUrl: null, requiredBy: [], dependency: !!m?.dep, updateAvailable: m?.upd ?? null,
        installedAt: now - (i * 3 + 1) * DAY,
      } satisfies ContentItem;
    }));
  }
  function recompute(list: ContentItem[]): ContentItem[] {
    for (const it of list) it.requiredBy = list.filter((o) => (DEPS[o.projectId] ?? []).includes(it.projectId)).map((o) => o.projectId);
    return list;
  }
  function listFor(profileId: string): ContentItem[] {
    ctx.find(profileId);
    let l = lists.get(profileId);
    if (!l) {
      const p = ctx.find(profileId);
      const seeded = p.name === 'Iron Hollow' ? FULL_PROFILE
        : p.name === 'Shader Showcase' ? ['iris', 'sodium', 'complementary-reimagined', 'bsl-shaders', 'faithful-32x', 'fresh-animations', 'lithium', 'ferrite-core', 'modmenu', 'xaeros-minimap']
        : p.name === 'Bedwars Practice' ? ['lithium', 'entityculling', 'dynamic-fps', 'appleskin', 'continuity', 'modmenu', 'ferrite-core']
        : p.name === 'Vanilla Survival' ? ['default-dark-mode'] : [];
      l = build(profileId, seeded);
      lists.set(profileId, l);
    }
    return l;
  }
  const snap = (l: ContentItem[]) => structuredClone(l);
  const syncCount = (profileId: string) => { ctx.find(profileId).modCount = listFor(profileId).filter((i) => i.type === 'mod').length; };

  /* ---------- screenshots / logs ---------- */
  const shots = new Map<string, Screenshot[]>();
  function shotsFor(profileId: string): Screenshot[] {
    let l = shots.get(profileId);
    if (!l) {
      const n = ctx.find(profileId).name === 'Iron Hollow' ? 12 : ctx.find(profileId).playTimeMs > 0 ? 4 : 0;
      l = Array.from({ length: n }, (_, i) => {
        const d = new Date(now - i * 1.7 * DAY);
        const pad = (x: number) => String(x).padStart(2, '0');
        return {
          name: `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}_${pad(20 - (i % 5))}.${pad(4 + i * 3)}.${pad(10 + i)}.png`,
          size: 1_400_000 + i * 91_000, taken: d.getTime(), url: fallbackImage('profile', `shot-${i}`, 256),
        };
      });
      shots.set(profileId, l);
    }
    return l;
  }
  const logs = new Map<string, LogEntry[]>();
  function logsFor(profileId: string): LogEntry[] {
    let l = logs.get(profileId);
    if (!l) {
      const p = ctx.find(profileId);
      l = p.playTimeMs === 0 ? [] : [
        { id: 'latest.log', name: 'latest.log', size: 184_320, modified: now - 2 * 3600000, kind: 'game' },
        { id: 'crash-2026-11-16_22.14.07-client.txt', name: 'crash-2026-11-16_22.14.07-client.txt', size: 9_410, modified: now - 4 * DAY, kind: 'crash' },
        { id: '2026-11-17-2.log', name: '2026-11-17-2.log', size: 96_400, modified: now - 3 * DAY, kind: 'game' },
        { id: '2026-11-15-1.log', name: '2026-11-15-1.log', size: 51_020, modified: now - 5 * DAY, kind: 'game' },
        { id: 'launcher.log', name: 'launcher.log', size: 7_800, modified: now - 2 * 3600000, kind: 'launcher' },
      ];
      logs.set(profileId, l);
    }
    return l;
  }

  /* ---------- servers ---------- */
  const pingStatus = (id: string): ServerStatus => {
    const s = STATUS[id];
    if (!s) return { id, online: false, ping: null, motd: '', players: null, version: null };
    return s.online ? { ...s, ping: (s.ping ?? 40) + Math.floor(Math.random() * 6) } : { ...s };
  };

  /* ---------- storage ---------- */
  let storage: StorageUsage[] = [
    { key: 'instances', bytes: 4_510_000_000, clearable: false }, { key: 'assets', bytes: 1_180_000_000, clearable: true },
    { key: 'runtimes', bytes: 684_000_000, clearable: true }, { key: 'versions', bytes: 641_000_000, clearable: true },
    { key: 'libraries', bytes: 402_000_000, clearable: true }, { key: 'cache', bytes: 231_000_000, clearable: true },
    { key: 'logs', bytes: 67_000_000, clearable: true }, { key: 'natives', bytes: 38_000_000, clearable: true },
  ];

  /* ---------- updates ---------- */
  let update: UpdateStatus = { state: 'idle' };
  const setUpdate = (u: UpdateStatus) => { update = u; ctx.emit('update:status', { ...u }); };

  /* ---------- install ---------- */
  async function runInstall(profileId: string, projectId: string, label: string, steps = 5) {
    for (let i = 1; i <= steps; i++) {
      await sleep(180);
      ctx.emit('install:progress', { profileId, projectId, done: i, total: steps, label });
    }
  }
  const versionsOf = (projectId: string): { id: string; number: string; gv: string[]; type: ProjectVersion['type'] }[] => {
    const base = CONTENT[projectId]?.ver ?? '1.4.0';
    const upd = CONTENT[projectId]?.upd;
    const [maj, min] = (/^\D*(\d+)\.(\d+)/.exec(base) ?? [, '1', '4']).slice(1).map(Number);
    return [
      ...(upd ? [{ id: `${projectId}-vu`, number: upd, gv: ['1.21.4', '1.21.3'], type: 'release' as const }] : []),
      { id: `${projectId}-v1`, number: base, gv: ['1.21.4', '1.21.3'], type: 'release' },
      { id: `${projectId}-v2`, number: `${maj}.${min}.0-beta.2`, gv: ['1.21.4'], type: 'beta' },
      { id: `${projectId}-v3`, number: `${maj}.${Math.max(0, min - 1)}.5`, gv: ['1.21.1', '1.21'], type: 'release' },
      { id: `${projectId}-v4`, number: `${maj}.${Math.max(0, min - 2)}.2`, gv: ['1.20.6', '1.20.4'], type: 'release' },
      { id: `${projectId}-v5`, number: `${maj}.${Math.max(0, min - 3)}.0-alpha.1`, gv: ['1.20.1'], type: 'alpha' },
    ];
  };

  return {
    async contentList({ profileId }) { await sleep(60); return snap(listFor(profileId)); },
    async contentToggle({ profileId, projectId, enabled }) {
      const l = listFor(profileId);
      const it = l.find((i) => i.projectId === projectId);
      if (!it) throw fail('content_not_found');
      const affected: string[] = [];
      const visit = (id: string) => {
        const x = l.find((i) => i.projectId === id);
        if (!x || x.enabled === enabled) return;
        x.enabled = enabled;
        x.filename = enabled ? x.filename.replace(/\.disabled$/, '') : `${x.filename}.disabled`;
        if (id !== projectId) affected.push(id);
        // disabling cascades to dependents, enabling cascades to required dependencies
        if (enabled) for (const d of DEPS[id] ?? []) visit(d);
        else for (const o of l) if ((DEPS[o.projectId] ?? []).includes(id)) visit(o.projectId);
      };
      visit(projectId);
      return { items: snap(l), affected };
    },
    async contentUninstall({ profileId, projectId }) {
      const l = listFor(profileId);
      const dependents = l.filter((o) => (DEPS[o.projectId] ?? []).includes(projectId)).map((o) => o.projectId);
      if (dependents.length) return { blocked: true, dependents, items: snap(l) };
      const next = recompute(l.filter((i) => i.projectId !== projectId));
      lists.set(profileId, next);
      syncCount(profileId);
      return { blocked: false, dependents: [], items: snap(next) };
    },
    async contentCheckUpdates({ profileId }) {
      await sleep(250);
      const l = listFor(profileId);
      return l.filter((i) => i.updateAvailable).map((i): UpdateInfo => ({
        projectId: i.projectId, title: i.title, type: i.type, from: i.versionNumber, to: i.updateAvailable as string,
        versionId: `${i.projectId}-vu`, compatibility: true, conflict: false, requiredBy: i.requiredBy,
      }));
    },
    async contentOpenFolder({ profileId, type }) { return `~/.mangoclient/instances/${profileId}/${type === 'mod' ? 'mods' : type === 'shader' ? 'shaderpacks' : type === 'resourcepack' ? 'resourcepacks' : 'datapacks'}`; },

    async modrinthSearch({ params }: { params: SearchParams }) {
      await sleep(220);
      const q = params.query.trim().toLowerCase();
      let hits = (TABLES[params.projectType] ?? []).map((r) => toHit(r[0], now));
      if (q) hits = hits.filter((h) => `${h.title} ${h.description} ${h.author}`.toLowerCase().includes(q));
      if (params.loader && (params.projectType === 'mod' || params.projectType === 'modpack')) hits = hits.filter((h) => h.loaders.includes(params.loader as string));
      if (params.mcVersion) hits = hits.filter((h) => h.gameVersions.includes(params.mcVersion as string));
      if (params.categories?.length) hits = hits.filter((h) => params.categories?.some((c) => h.categories.includes(c)));
      const key = { downloads: (h: ProjectHit) => -h.downloads, follows: (h: ProjectHit) => -h.follows, newest: (h: ProjectHit) => -hash(h.slug), updated: (h: ProjectHit) => -Date.parse(h.dateModified) }[params.sort as string];
      if (key) hits = [...hits].sort((a, b) => key(a) - key(b));
      const total = hits.length;
      const page = hits.slice(params.offset, params.offset + params.limit);
      for (const h of page) h.installedIn = ctx.profiles().filter((p) => listFor(p.id).some((i) => i.projectId === h.projectId)).map((p) => p.id);
      return { hits: page, total, offset: params.offset };
    },
    async modrinthProject({ projectId }): Promise<ProjectDetail> {
      await sleep(180);
      const hit = toHit(projectId, now);
      const body = `# ${hit.title}\n\n${hit.description}\n\n## Features\n\n- Works with **Fabric** and Quilt\n- Tuned for \`${hit.gameVersions[0]}\`\n- Configurable in game\n\n## Notes\n\nThis is mock data. HTML such as <script>alert(1)</script> or <img src=x onerror=alert(1)> and [links](https://example.com) are shown as plain text.\n\n\`\`\`\nrequires: java >= 21\n\`\`\`\n`;
      return {
        ...hit, body, gallery: [fallbackImage('profile', `${projectId}-g`, 256), fallbackImage('profile', `${projectId}-a`), fallbackImage('profile', `${projectId}-b`)],
        license: hit.type === 'shader' ? 'Custom' : 'LGPL-3.0-only', sourceUrl: `https://github.com/example/${projectId}`, issuesUrl: `https://github.com/example/${projectId}/issues`,
      };
    },
    async modrinthVersions({ projectId, profileId }): Promise<ProjectVersion[]> {
      await sleep(120);
      toHit(projectId, now);
      const p = profileId ? ctx.find(profileId) : null;
      return versionsOf(projectId).map((v, i) => ({
        versionId: v.id, versionNumber: v.number, name: `${titleOf(projectId)} ${v.number}`, type: v.type, gameVersions: v.gv,
        loaders: rowOf(projectId)?.type === 'resourcepack' ? ['minecraft'] : rowOf(projectId)?.type === 'shader' ? ['iris'] : ['fabric', 'quilt'],
        datePublished: new Date(now - (4 + i * 31) * DAY).toISOString(), downloads: Math.floor(900_000 / (i + 1.4)),
        compatible: p ? v.gv.includes(p.mcVersion) && (p.loader !== 'forge' && p.loader !== 'neoforge') : true,
      }));
    },
    async modrinthInstall({ profileId, projectId, versionId }) {
      const l = listFor(profileId);
      const hit = toHit(projectId, now);
      if (hit.type === 'modpack') throw fail('invalid_project_type');
      const kind: ContentType = hit.type;
      const ver = versionsOf(projectId).find((v) => v.id === versionId) ?? versionsOf(projectId)[0];
      await runInstall(profileId, projectId, hit.title);
      const make = (pid: string, dependency: boolean, number: string): ContentItem => ({
        projectId: pid, slug: pid, title: titleOf(pid), type: rowOf(pid)?.type === 'modpack' ? 'mod' : ((rowOf(pid)?.type ?? 'mod') as ContentType), versionId: `${pid}-v1`,
        versionNumber: number, filename: `${pid}-${number}${kind === 'mod' ? '.jar' : '.zip'}`, enabled: true, local: false, iconUrl: null,
        requiredBy: [], dependency, updateAvailable: null, installedAt: Date.now(),
      });
      const existing = l.findIndex((i) => i.projectId === projectId);
      if (existing >= 0) { l[existing] = { ...make(projectId, l[existing].dependency, ver.number), enabled: l[existing].enabled }; }
      else l.push(make(projectId, false, ver.number));
      for (const d of DEPS[projectId] ?? []) if (!l.some((i) => i.projectId === d)) l.push(make(d, true, CONTENT[d]?.ver ?? '1.0.0'));
      recompute(l);
      syncCount(profileId);
      return snap(l);
    },
    async modrinthInstallModpack({ projectId, name }) {
      const hit = toHit(projectId, now);
      if (hit.type !== 'modpack') throw fail('invalid_project_type');
      const id = 'mp-' + Math.floor(Math.random() * 1e9).toString(16).padStart(8, '0') + '-0000-4000-8000-000000000000';
      await runInstall(id, projectId, hit.title, 8);
      const p: ProfileView = {
        id, name: (name?.trim() || hit.title), mcVersion: '1.21.1', loader: 'fabric', loaderVersion: '0.16.10', color: null, coverUrl: null, ram: 4096,
        javaArgs: '', mangoConfig: null, lastPlayed: null, playTimeMs: 0, created: Date.now(), modCount: 0,
      };
      ctx.addProfile(p);
      lists.set(id, build(id, ['fabric-api', 'sodium', 'lithium', 'ferrite-core', 'modmenu', 'entityculling', 'immediatelyfast', 'moreculling']));
      syncCount(id);
      return structuredClone(p);
    },

    async serversList() { return structuredClone(SERVERS); },
    async serversPingAll() {
      SERVERS.forEach((s, i) => { setTimeout(() => ctx.emit('servers:pinged', pingStatus(s.id)), (260 + i * 330) * ctx.scale); });
    },
    async serversPing({ address }) {
      await sleep(300);
      const s = SERVERS.find((x) => x.address === address);
      return s ? pingStatus(s.id) : { id: address, online: false, ping: null, motd: '', players: null, version: null };
    },

    async screenshotsList({ profileId }) { return structuredClone(shotsFor(profileId)); },
    async screenshotAction({ profileId, name, action }) {
      if (action === 'delete') shots.set(profileId, shotsFor(profileId).filter((s) => s.name !== name));
    },
    async logsList({ profileId }) { return structuredClone(logsFor(profileId)); },
    async logsRead({ profileId, id }) {
      const e = logsFor(profileId).find((x) => x.id === id);
      if (!e) throw fail('log_not_found');
      if (e.kind === 'crash') return { text: CRASH(ctx.find(profileId).mcVersion), truncated: false };
      const n = id === 'latest.log' ? 3200 : 260;
      const r = rng(hash(id));
      const out: string[] = [];
      for (let i = 0; i < n; i++) {
        const s = Math.floor(i * 0.9);
        const ts = `[${String(20 + Math.floor(s / 3600) % 4).padStart(2, '0')}:${String(Math.floor(s / 60) % 60).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}]`;
        out.push(`${ts} ${LOG_LINES[Math.floor(r() * LOG_LINES.length)].replace('{n}', String(ctx.find(profileId).modCount))}`);
      }
      return { text: out.join('\n'), truncated: id === 'latest.log' };
    },
    async logsUpload({ profileId, id }) {
      if (!logsFor(profileId).some((x) => x.id === id)) throw fail('log_not_found');
      await sleep(700);
      return { url: `https://mclo.gs/${hash(profileId + id).toString(36).slice(0, 7)}` };
    },
    async logsDelete({ profileId, id }) { logs.set(profileId, logsFor(profileId).filter((x) => x.id !== id)); },

    async statsSummary({ days }): Promise<StatsSummary> {
      await sleep(80);
      const r = rng(7);
      const out: { date: string; ms: number }[] = [];
      for (let i = days - 1; i >= 0; i--) {
        const d = new Date(now - i * DAY);
        const weekend = d.getDay() === 0 || d.getDay() === 6;
        const roll = r();
        const ms = roll < (weekend ? 0.12 : 0.34) ? 0 : Math.round((weekend ? 2.6 : 1.1) * (0.3 + r()) * 3600000);
        out.push({ date: `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`, ms });
      }
      const totalMs = out.reduce((a, d) => a + d.ms, 0);
      const weights = [0.52, 0.22, 0.17, 0.09];
      const active = out.filter((d) => d.ms > 0).length;
      const perProfile = ctx.profiles().slice(0, 4).map((p, i) => ({
        profileId: p.id, name: p.name, ms: Math.round(totalMs * weights[i]), sessions: Math.max(1, Math.round(active * weights[i] * 1.8)),
      })).sort((a, b) => b.ms - a.ms);
      return { days: out, perProfile, totalMs, sessions: perProfile.reduce((a, p) => a + p.sessions, 0), longestMs: Math.round(Math.max(...out.map((d) => d.ms), 0) * 0.8) };
    },

    async storageUsage() { return structuredClone(storage); },
    async storageClear({ key }: { key: StorageKey }) {
      if (ctx.gameActive()) throw fail('game_running');
      await sleep(300);
      storage = storage.map((s) => (s.key === key && s.clearable ? { ...s, bytes: 0 } : s));
      return structuredClone(storage);
    },

    async updateStatus() { return { ...update }; },
    async updateCheck() {
      if (update.state === 'checking' || update.state === 'downloading') return { ...update };
      setUpdate({ state: 'checking' });
      void (async () => {
        await sleep(600);
        for (let p = 0; p <= 100; p += 12.5) { setUpdate({ state: 'downloading', percent: p, version: '2.0.1' }); await sleep(260); }
        setUpdate({ state: 'ready', version: '2.0.1', notes: 'Faster startup, content tab update badges, fixes for server quick join.' });
      })();
      return { state: 'checking' };
    },
    async updateInstall() { return update.state === 'ready'; },
  };
}
