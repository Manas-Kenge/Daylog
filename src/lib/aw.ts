// Tauri 2 converts JS camelCase keys to snake_case Rust params automatically
// — use camelCase here.

import { invoke } from "@tauri-apps/api/core";
import type {
  AfkSummary,
  AwEvent,
  AppOnlyData,
  CategorizedEvent,
  CategorySummary,
  DomainData,
  HourBucket,
  TimeRange,
} from "./aw-types";

export const awTopApps = (range: TimeRange) =>
  invoke<AwEvent<AppOnlyData>[]>("aw_top_apps", { range });

export const awTopCategories = (range: TimeRange) =>
  invoke<CategorySummary[]>("aw_top_categories", { range });

export const awHourly = (range: TimeRange) =>
  invoke<HourBucket[]>("aw_hourly", { range });

export const awCategorizedEvents = (range: TimeRange) =>
  invoke<CategorizedEvent[]>("aw_categorized_events", { range });

export interface TrailingDayPayload {
  days_ago: number;
  events: CategorizedEvent[];
  afk: AfkSummary;
}

/**
 * Past N days of categorized events + AFK summaries in one IPC call,
 * dispatched concurrently inside Rust. `days` is the count of past
 * days, where 1 = yesterday only. Today is intentionally excluded
 * (the dashboard refreshes today on a 5s tick; bundling would force
 * past-day AQL to re-run on every tick).
 */
export const awTrailingDaysPast = (days: number) =>
  invoke<TrailingDayPayload[]>("aw_trailing_days_past", { days });

export const awAfkSummary = (range: TimeRange, includeIntervals = false) =>
  invoke<AfkSummary>("aw_afk_summary", { range, includeIntervals });

export const awHasWebWatcher = () => invoke<boolean>("aw_has_web_watcher");

export const awTopDomains = (range: TimeRange) =>
  invoke<AwEvent<DomainData>[]>("aw_top_domains", { range });

export const appIcons = (names: string[]) =>
  invoke<Record<string, string | null>>("app_icons", { names });
