/**
 * Active page + optional filter, mutated by the command palette. There is
 * no router for v0.1 — the dashboard is the default view, and detail pages
 * are pushed by palette commands and dismissed back to Overview.
 */

import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

export type PageId =
  | "overview"
  | "apps"
  | "activity"
  | "hourly"
  | "web"
  | "categories"
  | "week"
  | "month"
  | "settings";

export const PAGE_TITLES: Record<PageId, string> = {
  overview: "Overview",
  apps: "Apps",
  activity: "Activity log",
  hourly: "Hourly patterns",
  web: "Web",
  categories: "Categories",
  week: "Week",
  month: "Month",
  settings: "Settings",
};

/** Optional filter passed when navigating to a detail page (e.g., palette
 * "kitty" jumps to Apps with `{ app: 'kitty' }`). Cleared on each push. */
export interface PageFilter {
  app?: string;
  category?: string;
}

interface PageContextValue {
  page: PageId;
  filter: PageFilter | null;
  push: (page: PageId, filter?: PageFilter) => void;
  back: () => void;
}

const PageContext = createContext<PageContextValue | null>(null);

export function PageProvider({ children }: { children: ReactNode }) {
  const [page, setPage] = useState<PageId>("overview");
  const [filter, setFilter] = useState<PageFilter | null>(null);

  const push = useCallback((next: PageId, nextFilter?: PageFilter) => {
    setPage(next);
    setFilter(nextFilter ?? null);
  }, []);

  const back = useCallback(() => {
    setPage("overview");
    setFilter(null);
  }, []);

  const value = useMemo(() => ({ page, filter, push, back }), [page, filter, push, back]);

  return <PageContext.Provider value={value}>{children}</PageContext.Provider>;
}

export function usePage(): PageContextValue {
  const ctx = useContext(PageContext);
  if (!ctx) throw new Error("usePage must be used inside <PageProvider>");
  return ctx;
}
