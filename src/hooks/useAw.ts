// Each query is keyed by command name + serialized range so flipping the
// range refetches all widgets.

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

export function useTopApps(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_top_apps", ...rangeKey(range)],
    queryFn: () => aw.awTopApps(range),
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

/**
 * Resolve `aw-watcher-window` app names to data:URL icons via the XDG
 * application/icon-theme cascade. Sorted-key memoization keeps the cache
 * stable across re-renders that produce the same set of apps.
 *
 * Icons don't change within a session (theme drift requires app restart),
 * so we set `staleTime: Infinity` and let the Rust-side cache do the work.
 */
export function useAppIcons(names: string[]) {
  const sorted = [...names].sort();
  return useQuery({
    queryKey: ["app_icons", sorted],
    queryFn: () => aw.appIcons(sorted),
    staleTime: Infinity,
    enabled: sorted.length > 0,
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
 * Two queries, not 14:
 *   - Today's slot piggybacks on the same query keys other widgets
 *     (`useCategorizedEvents(Today)`, `useAfkSummary(Today, false)`) use,
 *     so it's deduped from cache and refetches every 5s in lockstep with
 *     the rest of the live UI.
 *   - Days 1..N-1 collapse into one bundled IPC call (`aw_trailing_days_past`)
 *     with a 5min staleTime — past days don't change within a day.
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
  const todayEvents = useQuery({
    queryKey: ["aw_categorized_events", ...rangeKey(Today)],
    queryFn: () => aw.awCategorizedEvents(Today),
    refetchInterval: REFRESH_MS,
  });
  const todayAfk = useQuery({
    queryKey: ["aw_afk_summary", false, ...rangeKey(Today)],
    queryFn: () => aw.awAfkSummary(Today, false),
    refetchInterval: REFRESH_MS,
  });

  const pastCount = Math.max(0, days - 1);
  const past = useQuery({
    queryKey: ["aw_trailing_days_past", pastCount],
    queryFn: () => aw.awTrailingDaysPast(pastCount),
    enabled: pastCount > 0,
    staleTime: PAST_DAY_STALE_MS,
  });

  const data: TrailingDay[] = Array.from({ length: days }, (_, n) => {
    if (n === 0) {
      return {
        daysAgo: 0,
        events: todayEvents.data ?? null,
        activeSec: todayAfk.data?.active_seconds ?? null,
      };
    }
    const slot = past.data?.find((d) => d.days_ago === n);
    return {
      daysAgo: n,
      events: slot?.events ?? null,
      activeSec: slot?.afk.active_seconds ?? null,
    };
  });

  const isLoading =
    todayEvents.isLoading ||
    todayAfk.isLoading ||
    (pastCount > 0 && past.isLoading);

  return { data, isLoading };
}

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
