/**
 * Top apps · pill rows in a recessed body. Each row carries a per-hour
 * sparkline derived from the categorized event stream.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
import { Sparkline } from "@/components/Sparkline";
import { useTopApps, useCategorizedEvents } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import { categoryColor } from "@/lib/category-colors";
import { useMemo } from "react";

const TAKE = 8;

export function TopApps() {
  const { data: apps } = useTopApps();
  const { data: categorized } = useCategorizedEvents();

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

  const top = (apps ?? []).slice(0, TAKE);

  return (
    <WidgetCard
      title="Top apps"
      description="By active time"
      action={
        <span className="mono text-[10.5px] text-muted-foreground tracking-[0.13em] uppercase">
          {top.length} of {apps?.length ?? 0}
        </span>
      }
    >
      {top.length === 0 ? (
        <Empty>no apps tracked yet</Empty>
      ) : (
        <ListBody>
          {top.map((row) => {
            const app = row.data.app;
            const color = categoryColor(catByApp.get(app) ?? []);
            const spark = sparkByApp.get(app) ?? [];
            return (
              <ListRow key={app} cols="9px_1fr_56px_60px">
                <span
                  className="w-[8px] h-[8px] rounded-[2px]"
                  style={{ background: color }}
                />
                <span className="font-medium text-[12.5px] truncate">{app}</span>
                <Sparkline values={spark} color={color} width={56} height={14} />
                <span className="mono text-muted-foreground text-[11.5px] text-right">
                  {fmtDuration(row.duration)}
                </span>
              </ListRow>
            );
          })}
        </ListBody>
      )}
    </WidgetCard>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-muted-foreground text-[12px] py-[16px] text-center">
      {children}
    </div>
  );
}
