<script lang="ts">
  import InstanceIcon from './InstanceIcon.svelte';
  import TimerIcon from 'phosphor-svelte/lib/TimerIcon';
  import GameControllerIcon from 'phosphor-svelte/lib/GameControllerIcon';
  import TrophyIcon from 'phosphor-svelte/lib/TrophyIcon';
  import ChartLineIcon from 'phosphor-svelte/lib/ChartLineIcon';
  import CalendarCheckIcon from 'phosphor-svelte/lib/CalendarCheckIcon';
  import Seg from './Seg.svelte';
  import Skeleton from './skeleton/Skeleton.svelte';
  import StatTileSkeleton from './skeleton/StatTileSkeleton.svelte';
  import ListRowSkeleton from './skeleton/ListRowSkeleton.svelte';
  import LoadError from './LoadError.svelte';
  import EmptyState from './EmptyState.svelte';
  import { createLoadState } from '../lib/loadState.svelte';
  import { fmtDuration } from '../lib/format';
  import { app, errorText, getApi, lang, nav, t, toast, tp } from '../lib/stores.svelte';
  import type { StatsSummary } from '../lib/types';

  const RANGES = ['14', '30', '90'];
  let range = $state('30');
  const load = createLoadState<StatsSummary>({ isEmpty: () => false });
  const data = $derived(load.data);
  let hover = $state<number | null>(null);

  $effect(() => {
    const d = Number(range);
    hover = null;
    load.run(() => getApi().statsSummary({ days: d }));
    return () => load.cancel();
  });

  const max = $derived(Math.max(1, ...(data?.days ?? []).map((d) => d.ms)));
  const activeDays = $derived((data?.days ?? []).filter((d) => d.ms > 0).length);
  // nothing played in the range: one empty state instead of zero tiles, a flat chart and a list of zeros
  const empty = $derived(!!data && data.totalMs === 0);
  const avg = $derived(data && data.days.length ? data.totalMs / data.days.length : 0);
  const top = $derived(data?.perProfile[0] ?? null);
  const ticks = $derived([max, max / 2].map((v) => Math.round(v / 3600000 * 10) / 10));
  const label = (iso: string) => new Intl.DateTimeFormat(lang(), { day: 'numeric', month: 'short' }).format(new Date(`${iso}T12:00:00`));
  const hrs = (ms: number) => `${(ms / 3600000).toFixed(1)} h`;
  const summary = $derived(data ? t('stats.chartLabel', { days: range, total: fmtDuration(data.totalMs) }) : '');
</script>

<div class="st">
  <header>
    <h1 class="display">{t('nav.stats')}</h1>
    <Seg label={t('stats.range')} idPrefix="sr" value={range} onchange={(v) => (range = v)} options={RANGES.map((r) => ({ value: r, label: t('stats.days', { n: r }) }))} />
  </header>

  {#if empty}
    <EmptyState title={t('stats.emptyTitle', { n: range })} hint={t('stats.empty')}>
      <button class="btn" type="button" onclick={() => (nav.view = 'library')}>{t('home.continue')}</button>
    </EmptyState>
  {:else}
  {#if data}
    <dl class="tiles nb" style:--n="16px" style:--fill="var(--ink-1)">
      <div><dt><TimerIcon weight="bold" size={14} aria-hidden="true" />{t('stats.total')}</dt><dd class="big mono">{fmtDuration(data.totalMs)}</dd></div>
      <div><dt><GameControllerIcon weight="bold" size={14} aria-hidden="true" />{t('stats.sessions')}</dt><dd class="mono">{data.sessions}</dd></div>
      <div><dt><TrophyIcon weight="bold" size={14} aria-hidden="true" />{t('stats.longest')}</dt><dd class="mono">{fmtDuration(data.longestMs)}</dd></div>
      <div><dt><ChartLineIcon weight="bold" size={14} aria-hidden="true" />{t('stats.avg')}</dt><dd class="mono">{fmtDuration(avg)}</dd></div>
      <div><dt><CalendarCheckIcon weight="bold" size={14} aria-hidden="true" />{t('stats.activeDays')}</dt><dd class="mono">{`${activeDays} / ${data.days.length}`}</dd></div>
    </dl>
  {:else if load.phase === 'error'}
    <LoadError error={load.error} onretry={() => load.retry()} />
  {:else if load.showSkeleton}
    <StatTileSkeleton count={5} />
  {/if}

  <section aria-labelledby="st-daily">
    <h2 id="st-daily" class="eyebrow">{t('stats.daily')}</h2>
    {#if !data}
      {#if load.showSkeleton}
        <div class="chart" aria-busy="true">
          <span class="sr-only" role="status">{t('skel.loading')}</span>
          <div class="axis mono" aria-hidden="true"></div>
          <div class="bars" aria-hidden="true">{#each Array(Number(range)) as _, i (i)}<div class="col"><Skeleton variant="box" w="100%" h="{20 + ((i * 37) % 70)}%" radius={2} /></div>{/each}</div>
        </div>
      {/if}
    {:else}
      <div class="chart" role="img" aria-label={summary}>
        <div class="axis mono" aria-hidden="true"><span>{ticks[0]} h</span><span>{ticks[1]} h</span><span>0</span></div>
        <div class="bars" onmouseleave={() => (hover = null)} role="presentation">
          {#each data.days as d, i (d.date)}
            <div class="col" onmouseenter={() => (hover = i)} role="presentation">
              <div class="bar" class:zero={d.ms === 0} style:height="{Math.max(d.ms > 0 ? 3 : 1, (d.ms / max) * 100)}%" title="{label(d.date)}: {hrs(d.ms)}"></div>
            </div>
          {/each}
        </div>
        <div class="tip mono" aria-hidden="true">{#if hover !== null && data.days[hover]}{label(data.days[hover].date)} &middot; {hrs(data.days[hover].ms)}{:else}&nbsp;{/if}</div>
        <div class="xl mono" aria-hidden="true"><span>{label(data.days[0]?.date ?? '1970-01-01')}</span><span>{label(data.days[data.days.length - 1]?.date ?? '1970-01-01')}</span></div>
      </div>
    {/if}
  </section>

  <section aria-labelledby="st-prof">
    <h2 id="st-prof" class="eyebrow">{t('stats.perProfile')}</h2>
    {#if !data}
      {#if load.showSkeleton}<ListRowSkeleton count={4} img={36} padding="8px 0" gap={14} bars={[{ w: '38%', h: 13 }, { w: '22%', h: 10 }]} tail={100} announce={false} />{/if}
    {:else}
      <ul class="pp">
        {#each data.perProfile as p (p.profileId)}
          {@const prof = app.profiles.find((x) => x.id === p.profileId)}
          <li>
            <InstanceIcon profile={prof} id={p.profileId} size={36} radius={6} />
            <div class="nm"><b>{p.name}</b><span class="hint mono">{tp('stats.sessionsN', p.sessions)}</span></div>
            <div class="meter step-end" role="presentation"><span style:transform="scaleX({top && top.ms ? p.ms / top.ms : 0})"></span></div>
            <span class="mono val">{fmtDuration(p.ms)}</span>
            <span class="mono pct">{data.totalMs ? Math.round((p.ms / data.totalMs) * 100) : 0}%</span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
  {/if}
</div>

<style>
  .st { height: 100%; overflow: auto; padding: var(--s-5) 32px 32px; display: flex; flex-direction: column; gap: var(--s-4); }
  header { display: flex; justify-content: space-between; align-items: flex-end; gap: 12px; flex-wrap: wrap; }
  h1 { font-size: var(--fs-3xl); }
  .tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); margin: 0; padding: 1px; overflow: hidden; }
  .tiles > div { padding: 12px 16px; border-right: 1px solid var(--line-soft); }
  .tiles > div:last-child { border-right: 0; }
  dt { display: flex; align-items: flex-start; gap: 6px; font-size: 12px; letter-spacing: 0.06em; text-transform: uppercase; color: var(--paper-mute); font-weight: 600; }
  dt :global(svg) { flex: none; margin-top: 1px; }
  dd { margin: 4px 0 0; font-size: 18px; }
  .big { color: var(--mango); font-size: 22px; }
  h2 { margin-bottom: 8px; }
  .chart { position: relative; display: grid; grid-template-columns: 44px minmax(0, 1fr); grid-template-rows: 180px 22px 18px; column-gap: 8px; padding: 14px 14px 8px; background: var(--ink-1); border: 1px solid var(--line); }
  .axis { display: flex; flex-direction: column; justify-content: space-between; align-items: flex-end; font-size: 12px; color: var(--paper-mute); }
  .bars { display: flex; align-items: stretch; gap: 2px; border-bottom: 1px solid var(--line); background: linear-gradient(var(--line-soft) 1px, transparent 1px) 0 0 / 100% 50%; }
  .col { flex: 1; min-width: 0; display: flex; align-items: flex-end; }
  .col:hover { background: rgba(255, 173, 50, 0.07); }
  .bar { width: 100%; background: var(--accent-fill); min-height: 1px; }
  .bar.zero { background: var(--ink-4); }
  .tip { grid-column: 2; font-size: 12.5px; color: var(--paper-dim); padding-top: 4px; }
  .xl { grid-column: 2; display: flex; justify-content: space-between; font-size: 12px; color: var(--paper-mute); }
  .pp { list-style: none; margin: 0; padding: 0; }
  .pp li { display: grid; grid-template-columns: 36px minmax(120px, 220px) minmax(0, 1fr) 100px 48px; gap: 14px; align-items: center; padding: 8px 0; border-bottom: 1px solid var(--line-soft); }
  .nm { display: grid; line-height: 1.25; min-width: 0; }
  .nm b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nm .hint { font-size: 12px; }
  .meter { height: 8px; background: var(--ink-3); overflow: hidden; }
  .meter span { display: block; height: 100%; background: var(--accent-fill); transform-origin: left; }
  .val, .pct { text-align: right; font-size: 13.5px; }
  .pct { color: var(--paper-mute); }
</style>
