/**
 * Activity log · full reverse-chronological event stream with text search
 * and category filter chips.
 */

import { useMemo, useState } from "react";
import { ListBody, ListRow, WidgetCard } from "@/components/widgets/Card";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useCategorizedEvents } from "@/hooks/useAw";
import { categoryColor, categoryRoot } from "@/lib/category-colors";
import { fmtClock, fmtDuration } from "@/lib/format";

export function ActivityLogPage() {
  const { data } = useCategorizedEvents();

  const [query, setQuery] = useState("");
  const [activeRoots, setActiveRoots] = useState<string[] | null>(null);

  const allRoots = useMemo(() => {
    const set = new Set<string>();
    for (const ev of data ?? []) set.add(categoryRoot(ev.category));
    return Array.from(set).sort();
  }, [data]);

  const sorted = useMemo(() => {
    const all = [...(data ?? [])].sort((a, b) =>
      a.timestamp < b.timestamp ? 1 : -1,
    );
    return all.filter((ev) => {
      if (activeRoots && !activeRoots.includes(categoryRoot(ev.category)))
        return false;
      if (!query) return true;
      const q = query.toLowerCase();
      const d = (ev.data ?? {}) as { app?: string; title?: string };
      return (
        (d.app ?? "").toLowerCase().includes(q) ||
        (d.title ?? "").toLowerCase().includes(q)
      );
    });
  }, [data, query, activeRoots]);

  const selected = activeRoots ?? allRoots;

  return (
    <WidgetCard
      title="Activity log"
      description={`${sorted.length} of ${data?.length ?? 0} events`}
      action={
        <Input
          type="text"
          placeholder="Search app or title…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="w-64"
        />
      }
      bodyClassName="flex flex-1 flex-col gap-2 min-h-0"
    >
      {allRoots.length > 0 && (
        <ToggleGroup
          type="multiple"
          size="sm"
          value={selected}
          onValueChange={(next) => {
            if (next.length === allRoots.length) setActiveRoots(null);
            else setActiveRoots(next);
          }}
          aria-label="Filter by category"
          className="flex-wrap"
        >
          {allRoots.map((root) => (
            <ToggleGroupItem key={root} value={root}>
              <span
                className="size-1.5 rounded-sm"
                style={{ background: categoryColor([root]) }}
              />
              {root}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      )}

      {sorted.length === 0 ? (
        <div className="py-6 text-center text-muted-foreground">
          {query || activeRoots
            ? "no events match the current filters"
            : "no activity yet"}
        </div>
      ) : (
        <ScrollArea className="flex-1 min-h-0">
          <ListBody>
            {sorted.map((ev, i) => {
              const data = (ev.data ?? {}) as { app?: string; title?: string };
              return (
                <ListRow key={i} cols="56px_9px_1fr_60px">
                  <span className="font-mono tabular-nums text-muted-foreground">
                    {fmtClock(ev.timestamp)}
                  </span>
                  <span
                    className="size-2 rounded-sm"
                    style={{ background: categoryColor(ev.category) }}
                  />
                  <span className="flex min-w-0 items-baseline gap-2">
                    <span className="font-medium">{data.app ?? "—"}</span>
                    <span className="min-w-0 truncate text-muted-foreground">
                      {data.title ?? ""}
                    </span>
                  </span>
                  <span className="text-right font-mono tabular-nums text-muted-foreground">
                    {fmtDuration(ev.duration)}
                  </span>
                </ListRow>
              );
            })}
          </ListBody>
        </ScrollArea>
      )}
    </WidgetCard>
  );
}
