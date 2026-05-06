/**
 * Apps · full sortable/searchable table of every app in the active range.
 * Each row shows a per-hour sparkline + percent of total active time.
 */

import { useEffect, useMemo, useState } from "react";
import { ListBody, ListRow, WidgetCard } from "@/components/widgets/Card";
import { Sparkline } from "@/components/Sparkline";
import { Input } from "@/components/ui/input";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useTopApps, useCategorizedEvents } from "@/hooks/useAw";
import { fmtDuration, fmtPercent } from "@/lib/format";
import { categoryColor, categoryLabel } from "@/lib/category-colors";
import { usePage } from "@/context/PageContext";

type SortKey = "duration" | "name";

export function AppsPage() {
  const { data: apps } = useTopApps();
  const { data: categorized } = useCategorizedEvents();
  const { filter } = usePage();

  const [query, setQuery] = useState(filter?.app ?? "");
  const [sort, setSort] = useState<SortKey>("duration");

  useEffect(() => {
    if (filter?.app) setQuery(filter.app);
  }, [filter?.app]);

  const sparkByApp = useMemo(() => {
    const out = new Map<string, number[]>();
    for (const ev of categorized ?? []) {
      const app = (ev.data as { app?: string })?.app;
      if (!app) continue;
      const h = new Date(ev.timestamp).getHours();
      const arr = out.get(app) ?? new Array(24).fill(0);
      arr[h] += ev.duration;
      out.set(app, arr);
    }
    return out;
  }, [categorized]);

  const catByApp = useMemo(() => {
    const out = new Map<string, string[]>();
    for (const ev of categorized ?? []) {
      const app = (ev.data as { app?: string })?.app;
      if (app && !out.has(app)) out.set(app, ev.category);
    }
    return out;
  }, [categorized]);

  const total = (apps ?? []).reduce((a, c) => a + c.duration, 0);

  const rows = useMemo(() => {
    const all = (apps ?? []).map((row) => ({
      app: row.data.app,
      duration: row.duration,
      category: catByApp.get(row.data.app) ?? [],
      pct: total > 0 ? row.duration / total : 0,
      spark: sparkByApp.get(row.data.app) ?? [],
    }));
    const filtered = query
      ? all.filter((r) => r.app.toLowerCase().includes(query.toLowerCase()))
      : all;
    if (sort === "name") filtered.sort((a, b) => a.app.localeCompare(b.app));
    else filtered.sort((a, b) => b.duration - a.duration);
    return filtered;
  }, [apps, catByApp, sparkByApp, total, query, sort]);

  return (
    <WidgetCard
      title="Apps"
      description={`${apps?.length ?? 0} unique apps · ${fmtDuration(total)} active total`}
      action={
        <div className="flex items-center gap-2">
          <Input
            type="text"
            placeholder="Search apps…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="w-48"
          />
          <ToggleGroup
            type="single"
            size="sm"
            value={sort}
            onValueChange={(v) => v && setSort(v as SortKey)}
            aria-label="Sort"
          >
            <ToggleGroupItem value="duration">Time</ToggleGroupItem>
            <ToggleGroupItem value="name">Name</ToggleGroupItem>
          </ToggleGroup>
        </div>
      }
    >
      {rows.length === 0 ? (
        <div className="py-6 text-center text-muted-foreground">
          {query ? `no apps match "${query}"` : "no apps tracked yet"}
        </div>
      ) : (
        <ListBody>
          <div className="grid grid-cols-[9px_1.4fr_1fr_56px_60px_70px] gap-2.5 px-2.5 pt-0.5 pb-1.5 text-[0.625rem] font-medium uppercase tracking-wider text-muted-foreground">
            <span></span>
            <span>App</span>
            <span>Category</span>
            <span className="text-right">24h</span>
            <span className="text-right">Share</span>
            <span className="text-right">Total</span>
          </div>
          {rows.map((r) => {
            const color = categoryColor(r.category);
            return (
              <ListRow key={r.app} cols="9px_1.4fr_1fr_56px_60px_70px">
                <span
                  className="size-2 rounded-sm"
                  style={{ background: color }}
                />
                <span className="truncate font-medium">{r.app}</span>
                <span className="truncate text-muted-foreground">
                  {categoryLabel(r.category)}
                </span>
                <Sparkline values={r.spark} color={color} width={56} height={14} />
                <span className="text-right font-mono tabular-nums text-muted-foreground">
                  {fmtPercent(r.pct, 1)}
                </span>
                <span className="text-right font-mono tabular-nums text-foreground">
                  {fmtDuration(r.duration)}
                </span>
              </ListRow>
            );
          })}
        </ListBody>
      )}
    </WidgetCard>
  );
}
