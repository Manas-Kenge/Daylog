import { ListBody, ListRow, WidgetCard } from "./Card";
import { Sparkline } from "@/components/Sparkline";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { useTopApps, useCategorizedEvents, useAppIcons } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import { categoryColor } from "@/lib/category-colors";
import type { TimeRange } from "@/lib/aw-types";
import { useMemo, useState } from "react";

interface TopAppsProps {
  rangeOverride?: TimeRange;
  /** Defaults true on single-day ranges, false on multi-day ranges (the
   *  categorized-events query gets heavy fast, and the 24-bucket sparkline
   *  reads as noise across N days). */
  showSparklines?: boolean;
  title?: string;
  description?: string;
}

const TAKE = 12;

export function TopApps({
  rangeOverride,
  showSparklines = true,
  title = "Top apps",
  description = "By active time",
}: TopAppsProps = {}) {
  const { data: apps, isLoading } = useTopApps(rangeOverride);
  // Skip the categorized-events fetch entirely when sparklines are off
  // — saves a potentially heavy query for the multi-day Month view.
  const { data: categorized } = useCategorizedEvents(rangeOverride, {
    enabled: showSparklines,
  });

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
  const maxDuration = top.reduce((a, r) => Math.max(a, r.duration), 0);
  const { data: icons } = useAppIcons(top.map((r) => r.data.app));
  // Without sparklines we lean a "duration bar" into the row — useful
  // multi-day surface that doesn't collapse to a single number.
  const cols = showSparklines ? "16px_1fr_56px_60px" : "16px_1fr_72px_60px";

  return (
    <WidgetCard
      title={title}
      description={description}
      action={
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          {top.length} of {apps?.length ?? 0}
        </Badge>
      }
    >
      {isLoading ? (
        <SkeletonRows cols={cols} />
      ) : top.length === 0 ? (
        <Empty>no apps tracked yet</Empty>
      ) : (
        <ListBody>
          {top.map((row) => {
            const app = row.data.app;
            const color = categoryColor(catByApp.get(app) ?? []);
            const spark = sparkByApp.get(app) ?? [];
            const pct = maxDuration > 0 ? row.duration / maxDuration : 0;
            const icon = icons?.[app] ?? null;
            return (
              <ListRow key={app} cols={cols}>
                <AppGlyph icon={icon} color={color} />
                <span className="truncate font-medium">{app}</span>
                {showSparklines ? (
                  <Sparkline values={spark} color={color} width={56} height={14} />
                ) : (
                  <DurationBar pct={pct} color={color} />
                )}
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

function AppGlyph({ icon, color }: { icon: string | null; color: string }) {
  const [failed, setFailed] = useState(false);
  if (icon && !failed) {
    return (
      <img
        src={icon}
        alt=""
        className="size-6 shrink-0 rounded-sm object-contain"
        onError={() => setFailed(true)}
      />
    );
  }
  return (
    <span className="flex size-6 shrink-0 items-center justify-center">
      <span className="size-3 rounded-sm" style={{ background: color }} />
    </span>
  );
}

function DurationBar({ pct, color }: { pct: number; color: string }) {
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary">
      <div
        className="h-full rounded-full transition-[width]"
        style={{ width: `${Math.max(2, pct * 100)}%`, background: color }}
      />
    </div>
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
          <Skeleton className="size-4 rounded-sm" />
          <Skeleton className="h-3 w-3/4" />
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-3 w-12 justify-self-end" />
        </ListRow>
      ))}
    </ListBody>
  );
}
