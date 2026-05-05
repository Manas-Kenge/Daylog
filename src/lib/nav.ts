/**
 * Sidebar navigation identifiers + display titles. Shared by Sidebar, Topbar,
 * and App so the active view stays in sync.
 */

export type NavId =
  | "overview"
  | "apps"
  | "categories"
  | "web"
  | "activity"
  | "hourly"
  | "weekly"
  | "compare"
  | "settings-tracking"
  | "settings-categories"
  | "settings-general";

export const PAGE_TITLES: Record<NavId, string> = {
  overview:               "Overview",
  apps:                   "Apps",
  categories:             "Categories",
  web:                    "Web activity",
  activity:               "Activity log",
  hourly:                 "Hourly patterns",
  weekly:                 "Weekly trends",
  compare:                "Compare days",
  "settings-tracking":    "Tracking",
  "settings-categories":  "Categories",
  "settings-general":     "General",
};
