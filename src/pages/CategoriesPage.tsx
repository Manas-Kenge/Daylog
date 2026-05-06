/**
 * Categories · grouped by category root, each row drillable to its apps.
 * Honors `filter.category` from the page context.
 */

import { useMemo, useState } from "react";
import { ListBody, ListRow, WidgetCard } from "@/components/widgets/Card";
import { useCategorizedEvents, useTopCategories } from "@/hooks/useAw";
import { categoryColor, categoryRoot } from "@/lib/category-colors";
import { fmtDuration, fmtPercent } from "@/lib/format";
import { usePage } from "@/context/PageContext";
import { cn } from "@/lib/utils";

interface AppBreakdown {
  app: string;
  duration: number;
}

export function CategoriesPage() {
  const { data: cats } = useTopCategories();
  const { data: events } = useCategorizedEvents();
  const { filter } = usePage();

  const [expanded, setExpanded] = useState<string | null>(filter?.category ?? null);

  const total = (cats ?? []).reduce((a, c) => a + c.duration, 0);

  const breakdownByRoot = useMemo(() => {
    const out = new Map<string, { subTotals: Map<string, number>; appTotals: Map<string, number> }>();
    for (const ev of events ?? []) {
      const root = categoryRoot(ev.category);
      const sub = ev.category[1] ?? "—";
      const app = (ev.data as { app?: string })?.app;
      const cur =
        out.get(root) ?? { subTotals: new Map(), appTotals: new Map() };
      cur.subTotals.set(sub, (cur.subTotals.get(sub) ?? 0) + ev.duration);
      if (app)
        cur.appTotals.set(app, (cur.appTotals.get(app) ?? 0) + ev.duration);
      out.set(root, cur);
    }
    return out;
  }, [events]);

  const rolled = useMemo(() => {
    const totals = new Map<string, number>();
    for (const c of cats ?? []) {
      const root = categoryRoot(c.name);
      totals.set(root, (totals.get(root) ?? 0) + c.duration);
    }
    return Array.from(totals.entries())
      .map(([root, duration]) => ({ root, duration }))
      .sort((a, b) => b.duration - a.duration);
  }, [cats]);

  return (
    <WidgetCard
      title="Categories"
      description={`${rolled.length} category roots · ${fmtDuration(total)} total`}
    >
      {rolled.length === 0 ? (
        <div className="py-6 text-center text-muted-foreground">
          no categorized activity yet
        </div>
      ) : (
        <ListBody>
          {rolled.map((row) => {
            const breakdown = breakdownByRoot.get(row.root);
            const apps: AppBreakdown[] = breakdown
              ? Array.from(breakdown.appTotals.entries())
                  .map(([app, duration]) => ({ app, duration }))
                  .sort((a, b) => b.duration - a.duration)
                  .slice(0, 8)
              : [];
            const pct = total > 0 ? row.duration / total : 0;
            const isOpen = expanded === row.root;
            const color = categoryColor([row.root]);

            return (
              <div key={row.root}>
                <ListRow
                  cols="9px_1fr_60px_70px_60px"
                  className={cn("cursor-pointer", isOpen && "bg-muted/60")}
                >
                  <button
                    type="button"
                    onClick={() => setExpanded(isOpen ? null : row.root)}
                    className="contents"
                  >
                    <span
                      className="size-2 rounded-sm"
                      style={{ background: color }}
                    />
                    <span className="truncate text-left font-medium">
                      {row.root}
                    </span>
                    <span className="text-right font-mono tabular-nums text-muted-foreground">
                      {fmtPercent(pct, 1)}
                    </span>
                    <span className="block h-[3px] overflow-hidden rounded-sm bg-background/50">
                      <span
                        className="block h-full"
                        style={{
                          width: `${(pct * 100).toFixed(1)}%`,
                          background: color,
                        }}
                      />
                    </span>
                    <span className="text-right font-mono tabular-nums text-foreground">
                      {fmtDuration(row.duration)}
                    </span>
                  </button>
                </ListRow>

                {isOpen && (
                  <div className="ml-5 mt-0.5 mb-1.5 flex flex-col gap-0.5">
                    {apps.length === 0 ? (
                      <div className="px-2.5 py-1.5 text-muted-foreground">
                        no per-app breakdown for this range
                      </div>
                    ) : (
                      apps.map((a) => (
                        <div
                          key={a.app}
                          className="grid grid-cols-[1fr_auto] items-center gap-2.5 rounded-sm bg-muted/20 px-2.5 py-1.5"
                        >
                          <span>{a.app}</span>
                          <span className="font-mono tabular-nums text-muted-foreground">
                            {fmtDuration(a.duration)}
                          </span>
                        </div>
                      ))
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </ListBody>
      )}
    </WidgetCard>
  );
}
