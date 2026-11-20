// Pure install-flow logic (docs/ipc.md Part 4): instance picker state mapping, dependency plan selection,
// provider switch rules, update previews and the sequential runner. No Svelte, no DOM, so it is unit-tested.
import type { Api } from './api';
import { toAppError } from './api';
import type { Key } from './i18n';
import type {
  ContentItem, InstallPlan, InstallPlanItem, InstallTarget, ModpackPlan, ProfileView, Provider, ProviderStatus, TargetReason, UpdateInfo,
} from './types';

/* ---------- provider switch ---------- */
export const PROVIDERS: Provider[] = ['modrinth', 'curseforge'];
export interface SourceOption { provider: Provider; disabled: boolean; reason: NonNullable<ProviderStatus['reason']> | null }

/** Both sources are always offered; an unavailable one is disabled with its reason (never silently hidden). */
export function sourceOptions(status: ProviderStatus[] | null): SourceOption[] {
  return PROVIDERS.map((provider) => {
    const s = status?.find((x) => x.provider === provider);
    // before providers_status answers, assume available so the switch does not flicker
    if (!s) return { provider, disabled: false, reason: null };
    return { provider, disabled: !s.available, reason: s.available ? null : (s.reason ?? 'offline') };
  });
}
/** The wanted provider if usable, else the first usable one (Modrinth). */
export function effectiveProvider(want: Provider, opts: SourceOption[]): Provider {
  return opts.find((o) => o.provider === want && !o.disabled)?.provider ?? opts.find((o) => !o.disabled)?.provider ?? 'modrinth';
}
export const providerLabel = (p: Provider | null): string => (p === 'modrinth' ? 'Modrinth' : p === 'curseforge' ? 'CurseForge' : '');

/* ---------- instance picker ---------- */
export const REASON_KEY: Record<TargetReason, Key> = {
  loader: 'picker.reason.loader',
  mc_version: 'picker.reason.mc_version',
  no_compatible_version: 'picker.reason.no_compatible_version',
  content_type: 'picker.reason.content_type',
  distribution_blocked: 'picker.reason.distribution_blocked',
};
export interface TargetRow {
  target: InstallTarget;
  profile: ProfileView;
  selectable: boolean;
  /** Shown dark: installed or incompatible. */
  dark: boolean;
}
const ORDER = { available: 0, installed: 1, incompatible: 2 } as const;

/** Join targets with profiles (targets without a profile are dropped) and order: available, installed, incompatible. */
export function targetRows(targets: InstallTarget[], profiles: ProfileView[]): TargetRow[] {
  const rows: TargetRow[] = [];
  for (const target of targets) {
    const profile = profiles.find((p) => p.id === target.profileId);
    if (!profile) continue;
    rows.push({ target, profile, selectable: target.state === 'available', dark: target.state !== 'available' });
  }
  return rows.map((r, i) => ({ r, i })).sort((a, b) => ORDER[a.r.target.state] - ORDER[b.r.target.state] || a.i - b.i).map((x) => x.r);
}
/** First preferred profile id that is selectable (open instance, then last used, then the selected one). */
export function preselect(rows: TargetRow[], preferred: (string | null | undefined)[]): string[] {
  for (const id of preferred) if (id && rows.some((r) => r.selectable && r.profile.id === id)) return [id];
  return [];
}
export function toggleSelection(selected: string[], id: string, rows: TargetRow[]): string[] {
  const row = rows.find((r) => r.profile.id === id);
  if (!row?.selectable) return selected;
  return selected.includes(id) ? selected.filter((x) => x !== id) : [...selected, id];
}
export function selectAll(rows: TargetRow[]): string[] { return rows.filter((r) => r.selectable).map((r) => r.profile.id); }
export function selectionSummary(rows: TargetRow[], selected: string[]): { count: number; requiredDeps: number; available: number } {
  const chosen = rows.filter((r) => selected.includes(r.profile.id) && r.selectable);
  return { count: chosen.length, requiredDeps: chosen.reduce((a, r) => a + r.target.requiredDeps, 0), available: rows.filter((r) => r.selectable).length };
}
/** Arrow-key navigation over a list; Home/End jump. Returns the new index or null for other keys. */
export function nextIndex(i: number, key: string, len: number): number | null {
  if (len <= 0) return null;
  if (key === 'ArrowDown') return Math.min(len - 1, i + 1);
  if (key === 'ArrowUp') return Math.max(0, i - 1);
  if (key === 'Home') return 0;
  if (key === 'End') return len - 1;
  return null;
}
/** Every instance is blocked for the same reason: the project itself cannot be downloaded by the app. */
export const allBlocked = (rows: TargetRow[]): boolean => rows.length > 0 && rows.every((r) => r.target.reason === 'distribution_blocked');
/** A loader and version that would make the project installable, for "Create new instance for this". */
export function suggestInstance(hit: { loaders: string[]; gameVersions: string[] }): { loader: ProfileView['loader']; mcVersion: string } {
  const order = ['fabric', 'neoforge', 'forge', 'quilt'];
  const loader = (order.find((l) => hit.loaders.includes(l)) ?? 'vanilla') as ProfileView['loader'];
  return { loader, mcVersion: hit.gameVersions[0] ?? '' };
}

/* ---------- dependency plan ---------- */
export interface PlanGroups {
  requested: InstallPlanItem | null;
  required: InstallPlanItem[];
  optional: InstallPlanItem[];
  incompatible: InstallPlanItem[];
  blocked: InstallPlanItem[];
}
export function planGroups(plan: InstallPlan): PlanGroups {
  return {
    requested: plan.items.find((i) => i.role === 'requested') ?? null,
    required: plan.items.filter((i) => i.role === 'required'),
    optional: plan.items.filter((i) => i.role === 'optional'),
    incompatible: plan.items.filter((i) => i.role === 'incompatible'),
    blocked: plan.items.filter((i) => i.distributionBlocked),
  };
}
/** Optional ids to send as `include`: only ids that really are optional, not-installed items of this plan. */
export function includeFor(plan: InstallPlan, chosen: ReadonlySet<string> | string[]): string[] {
  const set = new Set(chosen);
  return plan.items.filter((i) => i.role === 'optional' && !i.alreadyInstalled && !i.distributionBlocked && set.has(i.projectId)).map((i) => i.projectId);
}
const downloads = (i: InstallPlanItem) => !i.alreadyInstalled && !i.distributionBlocked;
/** Files that would be written: requested + required + chosen optional, minus installed and blocked ones. */
export function fileCount(plan: InstallPlan, chosen: ReadonlySet<string> | string[]): number {
  const inc = new Set(includeFor(plan, chosen));
  return plan.items.filter((i) => (i.role === 'requested' || i.role === 'required' ? downloads(i) : inc.has(i.projectId))).length;
}
export function planBytes(plan: InstallPlan, chosen: ReadonlySet<string> | string[]): number {
  const inc = new Set(includeFor(plan, chosen));
  return plan.totalBytes + plan.items.filter((i) => inc.has(i.projectId)).reduce((a, i) => a + (i.sizeBytes ?? 0), 0);
}
/** Plan summary for the confirm button: the requested item cannot be downloaded at all. */
export const requestedBlocked = (plan: InstallPlan): boolean => !!planGroups(plan).requested?.distributionBlocked;

/* ---------- sequential runner ---------- */
export interface RunStep { profileId: string; provider: Provider; projectId: string; versionId?: string; include: string[]; title: string }
export interface StepResult { step: RunStep; ok: boolean; code?: string; items?: ContentItem[] }

/** Runs install_apply one step after another; a failed step is recorded and the rest still run. */
export async function runSteps(
  api: Pick<Api, 'installApply'>, steps: RunStep[], hooks: { onStart?: (i: number) => void; onResult?: (i: number, r: StepResult) => void } = {},
): Promise<StepResult[]> {
  const out: StepResult[] = [];
  for (let i = 0; i < steps.length; i++) {
    const step = steps[i];
    hooks.onStart?.(i);
    let r: StepResult;
    try {
      const items = await api.installApply({
        provider: step.provider, projectId: step.projectId, profileId: step.profileId, include: step.include,
        ...(step.versionId ? { versionId: step.versionId } : {}),
      });
      r = { step, ok: true, items };
    } catch (e) { r = { step, ok: false, code: toAppError(e).code }; }
    out.push(r);
    hooks.onResult?.(i, r);
  }
  return out;
}

/* ---------- update-all preview ---------- */
export interface UpdateRow {
  info: UpdateInfo;
  plan: InstallPlan | null;
  /** Required dependencies the new version adds (not installed yet, downloadable). */
  newDeps: InstallPlanItem[];
  blocked: InstallPlanItem[];
  conflicts: InstallPlan['conflicts'];
  bytes: number;
}
export function updateRows(infos: UpdateInfo[], plans: (InstallPlan | null)[]): UpdateRow[] {
  return infos.map((info, i) => {
    const plan = plans[i] ?? null;
    const g = plan ? planGroups(plan) : null;
    return {
      info, plan,
      newDeps: g ? g.required.filter((d) => !d.alreadyInstalled && !d.distributionBlocked) : [],
      blocked: g ? g.blocked.filter((d) => d.role !== 'requested') : [],
      conflicts: plan?.conflicts ?? [],
      bytes: plan?.totalBytes ?? 0,
    };
  });
}
/** Updates that can be applied: not blocked on the requested item. */
export const updatable = (rows: UpdateRow[]): UpdateRow[] => rows.filter((r) => !(r.plan && requestedBlocked(r.plan)));
export function updateSteps(profileId: string, rows: UpdateRow[]): RunStep[] {
  return updatable(rows).map((r) => ({
    profileId, provider: (r.info.provider ?? 'modrinth') as Provider, projectId: r.info.projectId, versionId: r.info.versionId, include: [], title: r.info.title,
  }));
}

/* ---------- modpack ---------- */
export const modpackBlocked = (p: ModpackPlan): boolean => p.blockedFiles.length > 0;
