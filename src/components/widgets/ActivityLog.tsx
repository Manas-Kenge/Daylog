/**
 * Reverse-chronological tail of categorized events as pill rows.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
import { useCategorizedEvents } from "@/hooks/useAw";
import { categoryColor } from "@/lib/category-colors";
import { fmtClock, fmtDuration } from "@/lib/format";

const TAKE = 12;

export function ActivityLog() {
  const { data } = useCategorizedEvents();
  const sorted = [...(data ?? [])]
    .sort((a, b) => (a.timestamp < b.timestamp ? 1 : -1))
    .slice(0, TAKE);

  return (
    <WidgetCard
      title="Recent activity"
      description="Most recent window events"
      action={
        <span className="mono text-[10.5px] text-muted-foreground tracking-[0.13em] uppercase">
          last {TAKE}
        </span>
      }
    >
      {sorted.length === 0 ? (
        <div className="text-muted-foreground text-[12px] py-[16px] text-center">
          no activity yet
        </div>
      ) : (
        <div className="max-h-[260px] overflow-y-auto">
          <ListBody>
            {sorted.map((ev, i) => {
              const data = (ev.data ?? {}) as { app?: string; title?: string };
              return (
                <ListRow key={i} cols="50px_9px_1fr_56px">
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
