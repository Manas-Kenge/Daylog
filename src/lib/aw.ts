/**
 * Typed wrappers around Tauri invoke() calls. Names match the Rust commands
 * registered in src-tauri/src/lib.rs. Tauri 2 converts JS camelCase keys to
 * snake_case Rust parameters automatically, so use camelCase here.
 */

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

export const awAfkSummary = (range: TimeRange, includeIntervals = false) =>
  invoke<AfkSummary>("aw_afk_summary", { range, includeIntervals });

export const awHasWebWatcher = () => invoke<boolean>("aw_has_web_watcher");

export const awTopDomains = (range: TimeRange) =>
  invoke<AwEvent<DomainData>[]>("aw_top_domains", { range });

export const appIcons = (names: string[]) =>
  invoke<Record<string, string | null>>("app_icons", { names });
