/**
 * Apps · full sortable/searchable table of every app in the active range.
 * Each row shows a per-hour sparkline + percent of total active time.
 */

import { useMemo, useState } from "react";
import { ListBody, ListRow, WidgetCard } from "@/components/widgets/Card";
import { Sparkline } from "@/components/Sparkline";
import { useTopApps, useCategorizedEvents } from "@/hooks/useAw";
import { fmtDuration, fmtPercent } from "@/lib/format";
import { categoryColor, categoryLabel } from "@/lib/category-colors";

type SortKey = "duration" | "name";

export function AppsPage() {
  const { data: apps } = useTopApps();
  const { data: categorized } = useCategorizedEvents();

  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortKey>("duration");

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
        <div className="flex items-center gap-[8px]">
          <input
            type="text"
            placeholder="Search apps…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="bg-card border border-border rounded-[var(--radius-sm)] px-[10px] py-[5px] text-[12px] w-[200px] focus:outline-none focus:ring-1 focus:ring-ring placeholder:text-muted-foreground"
          />
          <SortToggle value={sort} onChange={setSort} />
        </div>
      }
    >
      {rows.length === 0 ? (
        <div className="text-muted-foreground text-[12px] py-[24px] text-center">
          {query ? `no apps match "${query}"` : "no apps tracked yet"}
        </div>
      ) : (
        <ListBody>
          <div className="grid grid-cols-[9px_1.4fr_1fr_56px_60px_70px] gap-[10px] px-[10px] pt-[2px] pb-[6px] text-[10px] tracking-[0.13em] uppercase text-muted-foreground font-medium">
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
                  className="w-[8px] h-[8px] rounded-[2px]"
                  style={{ background: color }}
                />
                <span className="font-medium text-[12.5px] truncate">{r.app}</span>
                <span className="text-[11.5px] text-muted-foreground truncate">
                  {categoryLabel(r.category)}
                </span>
                <Sparkline values={r.spark} color={color} width={56} height={14} />
                <span className="mono text-muted-foreground text-[11.5px] text-right">
                  {fmtPercent(r.pct, 1)}
                </span>
                <span className="mono text-foreground text-[11.5px] text-right">
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

function SortToggle({
  value,
  onChange,
}: {
  value: SortKey;
  onChange: (v: SortKey) => void;
}) {
  const options: { id: SortKey; label: string }[] = [
    { id: "duration", label: "Time" },
    { id: "name", label: "Name" },
  ];
  return (
    <div className="inline-flex border border-border rounded-[var(--radius-sm)] overflow-hidden bg-card">
      {options.map((o, i) => (
        <button
          key={o.id}
          type="button"
          onClick={() => onChange(o.id)}
          className={
            "px-[10px] py-[5px] text-[11px] cursor-pointer " +
            (i < options.length - 1 ? "border-r border-border " : "") +
            (value === o.id
              ? "bg-secondary text-foreground"
              : "text-muted-foreground hover:text-foreground hover:bg-accent")
          }
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
