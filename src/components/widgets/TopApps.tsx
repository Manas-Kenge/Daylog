/**
 * Top apps · pill rows in a recessed body. Each row carries a per-hour
 * sparkline derived from the categorized event stream.
 */

import { ListBody, ListRow, WidgetCard } from "./Card";
import { Sparkline } from "@/components/Sparkline";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { useTopApps, useCategorizedEvents } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import { categoryColor } from "@/lib/category-colors";
import { useMemo } from "react";

const TAKE = 12;

export function TopApps() {
  const { data: apps, isLoading } = useTopApps();
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
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          {top.length} of {apps?.length ?? 0}
        </Badge>
      }
    >
      {isLoading ? (
        <SkeletonRows cols="9px_1fr_56px_60px" />
      ) : top.length === 0 ? (
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
                  className="size-2 rounded-sm"
                  style={{ background: color }}
                />
                <span className="truncate font-medium">{app}</span>
                <Sparkline values={spark} color={color} width={56} height={14} />
                <span className="text-right font-mono tabular-nums text-muted-foreground">
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
    <div className="py-4 text-center text-muted-foreground">{children}</div>
  );
}

function SkeletonRows({ cols, rows = 8 }: { cols: string; rows?: number }) {
  return (
    <ListBody>
      {Array.from({ length: rows }, (_, i) => (
        <ListRow key={i} cols={cols}>
          <Skeleton className="size-2 rounded-sm" />
          <Skeleton className="h-3 w-3/4" />
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-3 w-12 justify-self-end" />
        </ListRow>
      ))}
    </ListBody>
  );
}
