// What the global activity indicator lists: running installs (from install:progress and background runs),
// launches in progress and running games. Pure so it can be tested.
import type { Key } from './i18n';
import type { LaunchInfo, InstallRun } from './stores.svelte';
import type { InstallProgressEvent, ProfileView } from './types';

export interface ActivityItem {
  key: string;
  kind: 'install' | 'launch' | 'running';
  title: string;
  detail: string;
  /** 0..1 or null for indeterminate / not applicable. */
  fraction: number | null;
  profileId?: string;
  runId?: number;
}
interface Input {
  profiles: ProfileView[];
  launches: Record<string, LaunchInfo>;
  installs: Record<string, InstallProgressEvent>;
  runs: InstallRun[];
  label: (key: Key, vars?: Record<string, string | number>) => string;
}
const STAGES = ['preparing', 'java', 'loader', 'downloading', 'account', 'launching'] as const;

export function activityItems({ profiles, launches, installs, runs, label }: Input): ActivityItem[] {
  const out: ActivityItem[] = [];
  const name = (id: string) => profiles.find((p) => p.id === id)?.name ?? id;
  const seen = new Set<string>();
  for (const r of runs) {
    if (r.finished) continue;
    const job = r.jobs.find((j) => j.status === 'running') ?? r.jobs[0];
    const ev = installs[job.projectId];
    const done = r.jobs.filter((j) => j.status === 'done').length;
    seen.add(job.projectId);
    out.push({
      key: `run:${r.id}`, kind: 'install', title: r.title,
      detail: r.kind === 'modpack' ? label('activity.creating') : r.jobs.length > 1 ? label('activity.step', { done: done + 1, total: r.jobs.length }) : label('activity.installingTo', { name: name(job.profileId) }),
      fraction: ev && ev.total ? ev.done / ev.total : null, profileId: job.profileId || undefined, runId: r.id,
    });
  }
  for (const ev of Object.values(installs)) {
    if (seen.has(ev.projectId)) continue; // an install without a tracked run (started elsewhere)
    if (ev.projectId === 'import') { // a modpack import (Part 9): the new instance is not in the list yet
      out.push({ key: 'inst:import', kind: 'install', title: label('import.title'), detail: ev.label, fraction: ev.total ? ev.done / ev.total : null, profileId: undefined });
      continue;
    }
    out.push({ key: `inst:${ev.projectId}`, kind: 'install', title: ev.label || ev.projectId, detail: label('activity.installingTo', { name: name(ev.profileId) }), fraction: ev.total ? ev.done / ev.total : null, profileId: ev.profileId });
  }
  for (const [id, info] of Object.entries(launches)) {
    if (info.state === 'running') out.push({ key: `run-g:${id}`, kind: 'running', title: name(id), detail: label('launch.running'), fraction: null, profileId: id });
    else if ((STAGES as readonly string[]).includes(info.state)) {
      const i = STAGES.indexOf(info.state as (typeof STAGES)[number]);
      out.push({ key: `launch:${id}`, kind: 'launch', title: name(id), detail: label(`launch.${info.state}` as Key), fraction: (i + (info.state === 'downloading' ? info.downloadFraction : 0)) / STAGES.length, profileId: id });
    }
  }
  return out;
}
