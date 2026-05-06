/**
 * TanStack Query wrappers around the Tauri commands. Each is keyed by
 * command name + serialized range so flipping the range refetches all widgets.
 */

import { useQuery } from "@tanstack/react-query";
import { useRange } from "@/context/RangeContext";
import * as aw from "@/lib/aw";
import type { TimeRange } from "@/lib/aw-types";
import { Today, Yesterday } from "@/lib/aw-types";

const REFRESH_MS = 5_000;

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

export function useCategorizedEvents(rangeOverride?: TimeRange) {
  const { range: ctxRange } = useRange();
  const range = rangeOverride ?? ctxRange;
  return useQuery({
    queryKey: ["aw_categorized_events", ...rangeKey(range)],
    queryFn: () => aw.awCategorizedEvents(range),
    refetchInterval: REFRESH_MS,
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
