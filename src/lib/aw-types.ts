/**
 * TS mirror of the types returned by the Rust commands.
 * Source of truth: src-tauri/src/{aw_client,categories,aggregate,time}.rs
 */

export interface ServerInfo {
  hostname: string;
  version: string;
  testing: boolean;
  device_id?: string | null;
}

export interface Bucket {
  id: string;
  type: string;
  client: string;
  hostname: string;
  created: string;
  last_updated?: string | null;
}

export interface AwEvent<T = unknown> {
  id?: number | null;
  timestamp: string;
  duration: number;
  data: T;
}

export interface WindowData { app: string; title: string; }
export interface AppOnlyData { app: string; }
export interface DomainData { $domain: string; }
export interface UrlData { url: string; }
export interface AfkData { status: "afk" | "not-afk"; }

export interface HourBucket {
  hour: number;
  duration: number;
}

export interface AfkInterval {
  timestamp: string;
  duration: number;
  status: string;
}

export interface AfkSummary {
  active_seconds: number;
  afk_seconds: number;
  active_ratio: number;
  intervals: AfkInterval[];
}

export interface CategorySummary {
  name: string[];
  duration: number;
}

export interface CategorizedEvent {
  timestamp: string;
  duration: number;
  data: WindowData | Record<string, unknown>;
  category: string[];
}

export type Rule =
  | { type: "regex"; regex: string; ignore_case?: boolean }
  | { type: "none" };

export interface Category {
  name: string[];
  rule: Rule;
}

export interface CategoryConfig {
  categories: Category[];
}

/**
 * Mirror of TimeRange in src-tauri/src/time.rs.
 * Serde tag = "kind", rename_all = "snake_case".
 */
export type TimeRange =
  | { kind: "today" }
  | { kind: "yesterday" }
  | { kind: "last_n_days"; days: number }
  | { kind: "days_ago"; days: number }
  | { kind: "custom"; start: string; end: string };

export const Today: TimeRange = { kind: "today" };
export const Yesterday: TimeRange = { kind: "yesterday" };
export const LastNDays = (days: number): TimeRange => ({ kind: "last_n_days", days });
export const DaysAgo = (days: number): TimeRange => ({ kind: "days_ago", days });
