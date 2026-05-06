/**
 * Active page state. The dashboard is the default view; Week and Month
 * push themselves via the topbar's view toggle. There is no router for v0.1.
 */

import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

export type PageId = "overview" | "week" | "month";

export const PAGE_TITLES: Record<PageId, string> = {
  overview: "Overview",
  week: "Week",
  month: "Month",
};

interface PageContextValue {
  page: PageId;
  push: (page: PageId) => void;
  back: () => void;
}

const PageContext = createContext<PageContextValue | null>(null);

export function PageProvider({ children }: { children: ReactNode }) {
  const [page, setPage] = useState<PageId>("overview");

  const push = useCallback((next: PageId) => setPage(next), []);
  const back = useCallback(() => setPage("overview"), []);

  const value = useMemo(() => ({ page, push, back }), [page, push, back]);

  return <PageContext.Provider value={value}>{children}</PageContext.Provider>;
}

export function usePage(): PageContextValue {
  const ctx = useContext(PageContext);
  if (!ctx) throw new Error("usePage must be used inside <PageProvider>");
  return ctx;
}
