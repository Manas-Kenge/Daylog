/**
 * Last 7 days, stacked bars by category root. Uses 7 parallel queries
 * keyed on DaysAgo{n} for n=0..6.
 */

import { useQueries } from "@tanstack/react-query";
import { WidgetCard } from "./Card";
import { awTopCategories } from "@/lib/aw";
import { DaysAgo } from "@/lib/aw-types";
import type { CategorySummary } from "@/lib/aw-types";
import { categoryColor, categoryRoot } from "@/lib/category-colors";
import { fmtDuration } from "@/lib/format";
import { format, subDays } from "date-fns";

const SEG_ORDER = ["Programming", "Documents", "Browsing", "Comms", "Media", "Uncategorized"];

function rollup(cats: CategorySummary[] | undefined): Record<string, number> {
  const acc: Record<string, number> = {};
  for (const cat of cats ?? []) {
    const root = categoryRoot(cat.name);
    acc[root] = (acc[root] ?? 0) + cat.duration;
  }
  return acc;
}

export function WeekChart() {
  const today = new Date();
  // Newest day on the right (n=0 today, n=6 six days ago).
  const queries = useQueries({
    queries: Array.from({ length: 7 }, (_, n) => ({
      queryKey: ["aw_top_categories_week", n] as const,
      queryFn: () => awTopCategories(DaysAgo(n)),
      staleTime: 60_000,
    })),
  });

  // Build days array in chronological order (oldest first → today last).
  const days = Array.from({ length: 7 }, (_, i) => {
    const ago = 6 - i;
    const date = subDays(today, ago);
    const cats = queries[ago]?.data;
    return {
      date,
      label: format(date, "EEE"),
      isToday: ago === 0,
      segs: rollup(cats),
    };
  });

  const totals = days.map((d) =>
    SEG_ORDER.reduce((acc, k) => acc + (d.segs[k] ?? 0), 0),
  );
  const max = Math.max(1, ...totals);

  return (
    <WidgetCard
      title="Last 7 days"
      description="Stacked by category"
      action={<span className="mono text-[10.5px] text-muted-foreground tracking-[0.13em] uppercase">7-day window</span>}
    >
      <div className="grid grid-cols-7 gap-[6px] h-[110px] items-end">
        {days.map((d, i) => {
          const total = totals[i];
          const height = (total / max) * 100;
          return (
            <div
              key={i}
              className={
                "relative flex flex-col-reverse gap-px min-h-[2px] rounded-t-[2px] overflow-hidden " +
                (d.isToday ? "shadow-[0_0_0_1px_var(--brand-coral)]" : "")
              }
              style={{ height: `${height}%` }}
              title={`${d.label} ${format(d.date, "MMM d")} — ${fmtDuration(total)} total`}
            >
              {SEG_ORDER.map((k) => {
                const sec = d.segs[k] ?? 0;
                if (sec === 0) return null;
                return (
                  <div
                    key={k}
                    className="w-full"
                    style={{
                      flex: sec,
                      background: categoryColor([k]),
                    }}
                    title={`${d.label} — ${k}: ${fmtDuration(sec)}`}
                  />
                );
              })}
            </div>
          );
        })}
      </div>
      <div className="grid grid-cols-7 gap-[6px] mt-[6px] text-muted-foreground text-[10.5px] text-center">
        {days.map((d, i) => (
          <div key={i}>
            {d.label}
            <div className="mono text-[10px] mt-[2px] opacity-80">{fmtDuration(totals[i])}</div>
          </div>
        ))}
      </div>
    </WidgetCard>
  );
}
