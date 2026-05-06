/**
 * "Notable today" widget — Pulse's pattern-discovery surface.
 *
 * Surfaces 0-2 anomalies per day computed against the trailing-7-day
 * baseline. Empty state reads "No notable patterns today" — Pulse never
 * fakes interest. Suppressed entirely until the baseline has ≥1
 * effective day of history.
 */

import { useMemo } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowDown01Icon, ArrowUp01Icon } from "@hugeicons/core-free-icons";
import { WidgetCard } from "./Card";
import { Skeleton } from "@/components/ui/skeleton";
import { useCategorizedEvents, useTrailingDays } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import { categoryRoot, categoryColor } from "@/lib/category-colors";
import { notableToday, type Anomaly } from "@/lib/anomaly";
import type { CategorizedEvent } from "@/lib/aw-types";

function rootTotals(events: readonly CategorizedEvent[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const ev of events) {
    const root = categoryRoot(ev.category);
    out[root] = (out[root] ?? 0) + ev.duration;
  }
  return out;
}

export function NotableToday() {
  const { data: today, isLoading: todayLoading } = useCategorizedEvents();
  const { data: trailing, isLoading: trailingLoading } = useTrailingDays(8);

  const past = useMemo(
    () => (trailing ?? []).filter((d) => d.daysAgo > 0 && d.events != null),
    [trailing],
  );

  const anomalies = useMemo<Anomaly[]>(() => {
    if (!today || past.length === 0) return [];
    return notableToday({
      today: rootTotals(today),
      trailing: past.map((d) => ({
        totals: rootTotals(d.events ?? []),
        activeSec: d.activeSec ?? 0,
      })),
    });
  }, [today, past]);

  const loading = todayLoading || trailingLoading;
  const effectiveDaysReady = past.filter(
    (d) => (d.activeSec ?? 0) >= 30 * 60,
  ).length;

  return (
    <WidgetCard
      title="Notable today"
      description="Patterns vs your trailing 7 days"
    >
      {loading ? (
        <div className="flex flex-col gap-2.5">
          {Array.from({ length: 2 }, (_, i) => (
            <div key={i} className="flex items-start gap-2.5">
              <Skeleton className="size-6 shrink-0 rounded-md" />
              <div className="flex-1 space-y-1.5">
                <Skeleton className="h-3 w-3/4" />
                <Skeleton className="h-3 w-1/2" />
              </div>
            </div>
          ))}
        </div>
      ) : effectiveDaysReady === 0 ? (
        <div className="flex h-full items-center justify-center py-6 text-center text-muted-foreground">
          Building baseline ({past.length}/7 days)
          <br />
          <span className="text-[0.625rem]">
            Pattern shifts surface after a few days of tracked activity.
          </span>
        </div>
      ) : anomalies.length === 0 ? (
        <div className="flex h-full items-center justify-center py-6 text-center text-muted-foreground">
          No notable patterns today.
        </div>
      ) : (
        <ul className="flex flex-col gap-2">
          {anomalies.map((a) => (
            <AnomalyRow key={a.category} anomaly={a} />
          ))}
        </ul>
      )}
    </WidgetCard>
  );
}

function AnomalyRow({ anomaly }: { anomaly: Anomaly }) {
  const up = anomaly.direction === "up";
  const Icon = up ? ArrowUp01Icon : ArrowDown01Icon;
  const color = categoryColor([anomaly.category]);
  return (
    <li className="flex items-start gap-2.5">
      <span
        className="mt-0.5 inline-flex size-6 shrink-0 items-center justify-center rounded-md"
        style={{ background: `color-mix(in oklab, ${color} 25%, transparent)` }}
      >
        <HugeiconsIcon icon={Icon} size={14} style={{ color }} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium">
          <span style={{ color }}>{anomaly.category}</span>{" "}
          <span className="font-mono tabular-nums">
            {up ? "+" : "−"}
            {fmtDuration(Math.abs(anomaly.deltaSec))}
          </span>{" "}
          <span className="text-muted-foreground">vs typical</span>
        </div>
        <div className="truncate text-[0.625rem] text-muted-foreground">
          today {fmtDuration(anomaly.todaySec)} · typical{" "}
          {fmtDuration(anomaly.medianSec)}
        </div>
      </div>
    </li>
  );
}
