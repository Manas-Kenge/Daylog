/**
 * TanStack Query wrappers around the Tauri commands. Each is keyed by
 * command name + serialized range so flipping the range refetches all widgets.
 */

import { useQueries, useQuery } from "@tanstack/react-query";
import { useRange } from "@/context/RangeContext";
import * as aw from "@/lib/aw";
import type { CategorizedEvent, TimeRange } from "@/lib/aw-types";
import { DaysAgo, Today, Yesterday } from "@/lib/aw-types";

const REFRESH_MS = 5_000;
/** Past days don't change until the wall clock crosses midnight, so a
 *  5-minute staleTime is more than enough — it's effectively cached for
 *  the rest of the day after the first paint. */
const PAST_DAY_STALE_MS = 5 * 60_000;

function rangeKey(r: TimeRange): unknown[] {
  return [r.kind, ...Object.values(r).slice(1)];
}

export function useInfo() {
  return useQuery({
    queryKey: ["aw_info"],
    queryFn: aw.awInfo,
    staleTime: 60_000,
  });
}

export function useBuckets() {
  return useQuery({
    queryKey: ["aw_buckets"],
    queryFn: aw.awBuckets,
    staleTime: 30_000,
  });
}

export function useTopApps(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_top_apps", ...rangeKey(range)],
    queryFn: () => aw.awTopApps(range),
    refetchInterval: REFRESH_MS,
  });
}

export function useTimeline(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_timeline", ...rangeKey(range)],
    queryFn: () => aw.awTimeline(range),
    refetchInterval: REFRESH_MS,
  });
}

export function useTopCategories(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_top_categories", ...rangeKey(range)],
    queryFn: () => aw.awTopCategories(range),
    refetchInterval: REFRESH_MS,
  });
}

export function useHourly(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_hourly", ...rangeKey(range)],
    queryFn: () => aw.awHourly(range),
    refetchInterval: REFRESH_MS,
  });
}

export function useCategorizedEvents(
  rangeOverride?: TimeRange,
  options?: { enabled?: boolean },
) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_categorized_events", ...rangeKey(range)],
    queryFn: () => aw.awCategorizedEvents(range),
    refetchInterval: REFRESH_MS,
    enabled: options?.enabled ?? true,
  });
}

export function useAfkSummary(includeIntervals = false, rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_afk_summary", includeIntervals, ...rangeKey(range)],
    queryFn: () => aw.awAfkSummary(range, includeIntervals),
    refetchInterval: REFRESH_MS,
  });
}

export function useHasWebWatcher() {
  return useQuery({
    queryKey: ["aw_has_web_watcher"],
    queryFn: aw.awHasWebWatcher,
    staleTime: 60_000,
  });
}

export function useTopDomains(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  const { data: hasWatcher } = useHasWebWatcher();
  return useQuery({
    queryKey: ["aw_top_domains", ...rangeKey(range)],
    queryFn: () => aw.awTopDomains(range),
    enabled: hasWatcher === true,
    refetchInterval: REFRESH_MS,
  });
}

export function useTopUrls(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  const { data: hasWatcher } = useHasWebWatcher();
  return useQuery({
    queryKey: ["aw_top_urls", ...rangeKey(range)],
    queryFn: () => aw.awTopUrls(range),
    enabled: hasWatcher === true,
    refetchInterval: REFRESH_MS,
  });
}

/**
 * Scoped active-total for the Topbar.
 *
 * - Overview and most pages: today's active total (paired with yesterday for delta).
 * - Week page: sum of trailing 7 days' active seconds.
 * - Month page: sum of trailing 30 days' active seconds.
 *
 * Uses `enabled` flags so off-page queries don't fire — flipping pages
 * doesn't pay for the data the topbar isn't going to show. Past-day
 * queries dedupe with whatever WeekPage / MonthPage already cached, so
 * navigating from Overview → Week is instant after the first paint.
 */
export interface ScopedActive {
  activeSec: number;
  /** "today" | "7-day" | "30-day" — drives the label suffix in the Topbar. */
  label: "today" | "7-day" | "30-day";
  /** Today-vs-yesterday delta. Only set on the today scope. */
  delta?: number;
  isLoading: boolean;
}

export function useScopedActive(scope: "today" | "week" | "month"): ScopedActive {
  // Today + yesterday always run — they back the Overview default and
  // the topbar shows them whenever the user is somewhere "today-ish."
  const todayQ = useQuery({
    queryKey: ["aw_afk_summary", false, ...rangeKey(Today)],
    queryFn: () => aw.awAfkSummary(Today, false),
    refetchInterval: REFRESH_MS,
    enabled: scope === "today",
  });
  const yestQ = useQuery({
    queryKey: ["aw_afk_summary", false, ...rangeKey(Yesterday)],
    queryFn: () => aw.awAfkSummary(Yesterday, false),
    staleTime: 5 * 60_000,
    enabled: scope === "today",
  });

  const weekQs = useQueries({
    queries: Array.from({ length: 7 }, (_, n) => ({
      queryKey: ["aw_afk_summary_daysago", false, n],
      queryFn: () => aw.awAfkSummary(DaysAgo(n), false),
      enabled: scope === "week",
      refetchInterval: (n === 0 && scope === "week"
        ? REFRESH_MS
        : false) as number | false,
      staleTime: n === 0 ? 0 : PAST_DAY_STALE_MS,
    })),
  });
  const monthQs = useQueries({
    queries: Array.from({ length: 30 }, (_, n) => ({
      queryKey: ["aw_afk_summary_daysago", false, n],
      queryFn: () => aw.awAfkSummary(DaysAgo(n), false),
      enabled: scope === "month",
      refetchInterval: (n === 0 && scope === "month"
        ? REFRESH_MS
        : false) as number | false,
      staleTime: n === 0 ? 0 : PAST_DAY_STALE_MS,
    })),
  });

  if (scope === "week") {
    const sec = weekQs.reduce(
      (a, q) => a + (q.data?.active_seconds ?? 0),
      0,
    );
    return {
      activeSec: sec,
      label: "7-day",
      isLoading: weekQs.some((q) => q.isLoading),
    };
  }
  if (scope === "month") {
    const sec = monthQs.reduce(
      (a, q) => a + (q.data?.active_seconds ?? 0),
      0,
    );
    return {
      activeSec: sec,
      label: "30-day",
      isLoading: monthQs.some((q) => q.isLoading),
    };
  }
  const todaySec = todayQ.data?.active_seconds ?? 0;
  const yestSec = yestQ.data?.active_seconds;
  return {
    activeSec: todaySec,
    label: "today",
    delta: yestSec !== undefined ? todaySec - yestSec : undefined,
    isLoading: todayQ.isLoading,
  };
}

/**
 * Trailing N days of categorized events + AFK summary, indexed by `daysAgo`
 * (0 = today, 1 = yesterday, ..., N-1). Drives the Pattern Shift KPI sub-line
 * and the Notable Today widget.
 *
 * Today's slot uses 5s refetchInterval so live deltas track real-time;
 * past days use a 5min staleTime since they don't change inside one day.
 *
 * Returns null entries for days that are still loading.
 */
export interface TrailingDay {
  daysAgo: number;
  events: CategorizedEvent[] | null;
  activeSec: number | null;
}

export function useTrailingDays(days = 7): {
  data: TrailingDay[];
  isLoading: boolean;
} {
  const eventQueries = useQueries({
    queries: Array.from({ length: days }, (_, n) => ({
      queryKey: ["aw_categorized_events_daysago", n],
      queryFn: () => aw.awCategorizedEvents(DaysAgo(n)),
      refetchInterval: (n === 0 ? REFRESH_MS : false) as number | false,
      staleTime: n === 0 ? 0 : PAST_DAY_STALE_MS,
    })),
  });
  const afkQueries = useQueries({
    queries: Array.from({ length: days }, (_, n) => ({
      queryKey: ["aw_afk_summary_daysago", false, n],
      queryFn: () => aw.awAfkSummary(DaysAgo(n), false),
      refetchInterval: (n === 0 ? REFRESH_MS : false) as number | false,
      staleTime: n === 0 ? 0 : PAST_DAY_STALE_MS,
    })),
  });

  const data: TrailingDay[] = Array.from({ length: days }, (_, n) => ({
    daysAgo: n,
    events: eventQueries[n].data ?? null,
    activeSec: afkQueries[n].data?.active_seconds ?? null,
  }));

  const isLoading =
    eventQueries.some((q) => q.isLoading) || afkQueries.some((q) => q.isLoading);

  return { data, isLoading };
}

/**
 * Convenience: paired Today + Yesterday hourly queries for the
 * "compare to yesterday" overlay on the hourly chart.
 */
export function useHourlyTodayVsYesterday() {
  const today = useQuery({
    queryKey: ["aw_hourly", ...rangeKey(Today)],
    queryFn: () => aw.awHourly(Today),
    refetchInterval: REFRESH_MS,
  });
  const yesterday = useQuery({
    queryKey: ["aw_hourly", ...rangeKey(Yesterday)],
    queryFn: () => aw.awHourly(Yesterday),
    staleTime: 5 * 60_000,
  });
  return { today, yesterday };
}

/**
 * Convenience: pair active-time totals for today and yesterday so KPI cells
 * can render a delta without two ad-hoc queries.
 */
export function useAfkTodayVsYesterday() {
  const today = useQuery({
    queryKey: ["aw_afk_summary", false, ...rangeKey(Today)],
    queryFn: () => aw.awAfkSummary(Today, false),
    refetchInterval: REFRESH_MS,
  });
  const yesterday = useQuery({
    queryKey: ["aw_afk_summary", false, ...rangeKey(Yesterday)],
    queryFn: () => aw.awAfkSummary(Yesterday, false),
    staleTime: 5 * 60_000,
  });
  return { today, yesterday };
}
