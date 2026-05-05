/**
 * Activity log · full reverse-chronological event stream with text search
 * and category filter chips.
 */

import { useMemo, useState } from "react";
import { ListBody, ListRow, WidgetCard } from "@/components/widgets/Card";
import { useCategorizedEvents } from "@/hooks/useAw";
import { categoryColor, categoryRoot } from "@/lib/category-colors";
import { fmtClock, fmtDuration } from "@/lib/format";
import { cn } from "@/lib/utils";

export function ActivityLogPage() {
  const { data } = useCategorizedEvents();

  const [query, setQuery] = useState("");
  const [activeCats, setActiveCats] = useState<Set<string> | null>(null);

  // Distinct category roots in the data, in deterministic order.
  const allRoots = useMemo(() => {
    const set = new Set<string>();
    for (const ev of data ?? []) set.add(categoryRoot(ev.category));
    return Array.from(set).sort();
  }, [data]);

  const sorted = useMemo(() => {
    const all = [...(data ?? [])].sort((a, b) =>
      a.timestamp < b.timestamp ? 1 : -1,
    );
    return all
      .filter((ev) => {
        if (activeCats && !activeCats.has(categoryRoot(ev.category))) return false;
        if (!query) return true;
        const q = query.toLowerCase();
        const d = (ev.data ?? {}) as { app?: string; title?: string };
        return (
          (d.app ?? "").toLowerCase().includes(q) ||
          (d.title ?? "").toLowerCase().includes(q)
        );
      });
  }, [data, query, activeCats]);

  const toggleCat = (root: string) => {
    setActiveCats((prev) => {
      const next = new Set(prev ?? allRoots);
      if (next.has(root)) {
        next.delete(root);
      } else {
        next.add(root);
      }
      // If everything is selected, reset to "all" (null) for clarity.
      if (next.size === allRoots.length) return null;
      if (next.size === 0) return new Set();
      return next;
    });
  };

  return (
    <WidgetCard
      title="Activity log"
      description={`${sorted.length} of ${data?.length ?? 0} events`}
      action={
        <input
          type="text"
          placeholder="Search app or title…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="bg-card border border-border rounded-[var(--radius-sm)] px-[10px] py-[5px] text-[12px] w-[260px] focus:outline-none focus:ring-1 focus:ring-ring placeholder:text-muted-foreground"
        />
      }
    >
      {allRoots.length > 0 && (
        <div className="flex flex-wrap gap-[6px] mb-[8px]">
          {allRoots.map((root) => {
            const isActive = activeCats === null || activeCats.has(root);
            return (
              <button
                key={root}
                type="button"
                onClick={() => toggleCat(root)}
                className={cn(
                  "inline-flex items-center gap-[6px] px-[8px] py-[3px] rounded-[var(--radius-sm)] border text-[11px] transition-colors cursor-pointer",
                  isActive
                    ? "bg-secondary border-border text-foreground"
                    : "bg-card border-border text-muted-foreground hover:text-foreground",
                )}
              >
                <span
                  className="w-[7px] h-[7px] rounded-[2px]"
                  style={{
                    background: categoryColor([root]),
                    opacity: isActive ? 1 : 0.5,
                  }}
                />
                {root}
              </button>
            );
          })}
        </div>
      )}

      {sorted.length === 0 ? (
        <div className="text-muted-foreground text-[12px] py-[24px] text-center">
          {query || activeCats
            ? "no events match the current filters"
            : "no activity yet"}
        </div>
      ) : (
        <div className="max-h-[calc(100vh-260px)] overflow-y-auto">
          <ListBody>
            {sorted.map((ev, i) => {
              const data = (ev.data ?? {}) as { app?: string; title?: string };
              return (
                <ListRow key={i} cols="56px_9px_1fr_60px">
                  <span className="mono text-muted-foreground text-[11px]">
                    {fmtClock(ev.timestamp)}
                  </span>
                  <span
                    className="w-[8px] h-[8px] rounded-[2px]"
                    style={{ background: categoryColor(ev.category) }}
                  />
                  <span className="flex gap-[8px] min-w-0 items-baseline">
                    <span className="font-medium text-[12px]">{data.app ?? "—"}</span>
                    <span className="text-muted-foreground text-[11px] truncate min-w-0">
                      {data.title ?? ""}
                    </span>
                  </span>
                  <span className="mono text-muted-foreground text-[11px] text-right">
                    {fmtDuration(ev.duration)}
                  </span>
                </ListRow>
              );
            })}
          </ListBody>
        </div>
      )}
    </WidgetCard>
  );
}
