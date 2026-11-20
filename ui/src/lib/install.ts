// Pure install-flow logic (docs/ipc.md Part 4): instance picker state mapping, dependency plan selection,
// update previews and the sequential runner. No Svelte, no DOM, so it is unit-tested.
import type { Api } from './api';
import { toAppError } from './api';
import type { Key } from './i18n';
import type {
  ContentItem, InstallPlan, InstallPlanItem, InstallTarget, ProfileView, TargetReason, UpdateInfo,
} from './types';

/* ---------- instance picker ---------- */
export const REASON_KEY: Record<TargetReason, Key> = {
  loader: 'picker.reason.loader',
  mc_version: 'picker.reason.mc_version',
  no_compatible_version: 'picker.reason.no_compatible_version',
  content_type: 'picker.reason.content_type',
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
}
export function planGroups(plan: InstallPlan): PlanGroups {
  return {
    requested: plan.items.find((i) => i.role === 'requested') ?? null,
    required: plan.items.filter((i) => i.role === 'required'),
    optional: plan.items.filter((i) => i.role === 'optional'),
    incompatible: plan.items.filter((i) => i.role === 'incompatible'),
  };
}
/** Optional ids to send as `include`: only ids that really are optional, not-installed items of this plan. */
export function includeFor(plan: InstallPlan, chosen: ReadonlySet<string> | string[]): string[] {
  const set = new Set(chosen);
  return plan.items.filter((i) => i.role === 'optional' && !i.alreadyInstalled && set.has(i.projectId)).map((i) => i.projectId);
}
const downloads = (i: InstallPlanItem) => !i.alreadyInstalled;
/** Files that would be written: requested + required + chosen optional, minus installed ones. */
export function fileCount(plan: InstallPlan, chosen: ReadonlySet<string> | string[]): number {
  const inc = new Set(includeFor(plan, chosen));
  return plan.items.filter((i) => (i.role === 'requested' || i.role === 'required' ? downloads(i) : inc.has(i.projectId))).length;
}
export function planBytes(plan: InstallPlan, chosen: ReadonlySet<string> | string[]): number {
  const inc = new Set(includeFor(plan, chosen));
  return plan.totalBytes + plan.items.filter((i) => inc.has(i.projectId)).reduce((a, i) => a + (i.sizeBytes ?? 0), 0);
}
/* ---------- sequential runner ---------- */
export interface RunStep { profileId: string; projectId: string; versionId?: string; include: string[]; title: string }
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
        projectId: step.projectId, profileId: step.profileId, include: step.include,
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
  /** Required dependencies the new version adds (not installed yet). */
  newDeps: InstallPlanItem[];
  conflicts: InstallPlan['conflicts'];
  bytes: number;
}
export function updateRows(infos: UpdateInfo[], plans: (InstallPlan | null)[]): UpdateRow[] {
  return infos.map((info, i) => {
    const plan = plans[i] ?? null;
    const g = plan ? planGroups(plan) : null;
    return {
      info, plan,
      newDeps: g ? g.required.filter((d) => !d.alreadyInstalled) : [],
      conflicts: plan?.conflicts ?? [],
      bytes: plan?.totalBytes ?? 0,
    };
  });
}
export function updateSteps(profileId: string, rows: UpdateRow[]): RunStep[] {
  return rows.map((r) => ({ profileId, projectId: r.info.projectId, versionId: r.info.versionId, include: [], title: r.info.title }));
}
