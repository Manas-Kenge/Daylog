/**
 * Reverse-chronological tail of categorized events as pill rows.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { useCategorizedEvents } from "@/hooks/useAw";
import { categoryColor } from "@/lib/category-colors";
import { fmtClock, fmtDuration } from "@/lib/format";

const TAKE = 12;

export function ActivityLog() {
  const { data, isLoading } = useCategorizedEvents();
  const sorted = [...(data ?? [])]
    .sort((a, b) => (a.timestamp < b.timestamp ? 1 : -1))
    .slice(0, TAKE);

  return (
    <WidgetCard
      title="Recent activity"
      description="Most recent window events"
      action={
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          last {TAKE}
        </Badge>
      }
    >
      {isLoading ? (
        <ListBody>
          {Array.from({ length: 8 }, (_, i) => (
            <ListRow key={i} cols="50px_9px_1fr_56px">
              <Skeleton className="h-3 w-10" />
              <Skeleton className="size-2 rounded-sm" />
              <Skeleton className="h-3 w-3/4" />
              <Skeleton className="h-3 w-12 justify-self-end" />
            </ListRow>
          ))}
        </ListBody>
      ) : sorted.length === 0 ? (
        <div className="py-4 text-center text-muted-foreground">
          no activity yet
        </div>
      ) : (
        <ScrollArea className="h-[260px]">
          <ListBody>
            {sorted.map((ev, i) => {
              const data = (ev.data ?? {}) as { app?: string; title?: string };
              return (
                <ListRow key={i} cols="50px_9px_1fr_56px">
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
