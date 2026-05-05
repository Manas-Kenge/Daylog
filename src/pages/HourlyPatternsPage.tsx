/**
 * Hourly patterns · 7-day × 24-hour heatmap. Fetched via 7 parallel
 * `aw_hourly({DaysAgo: n})` queries; each cell is shaded by relative
 * activity intensity.
 */

import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import { format, subDays } from "date-fns";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { WidgetCard } from "@/components/widgets/Card";
import { awHourly } from "@/lib/aw";
import { DaysAgo, type HourBucket } from "@/lib/aw-types";
import { fmtDuration } from "@/lib/format";

const DAYS = 7;

interface DayCell {
  date: Date;
  weekday: string;
  buckets: HourBucket[];
  total: number;
  isToday: boolean;
}

export function HourlyPatternsPage() {
  const today = new Date();

  const queries = useQueries({
    queries: Array.from({ length: DAYS }, (_, n) => ({
      queryKey: ["aw_hourly_pattern", n] as const,
      queryFn: () => awHourly(DaysAgo(n)),
      staleTime: 60_000,
    })),
  });

  const days: DayCell[] = useMemo(() => {
    return Array.from({ length: DAYS }, (_, i) => {
      const ago = DAYS - 1 - i;
      const date = subDays(today, ago);
      const buckets =
        queries[ago]?.data ??
        Array.from({ length: 24 }, (_, h) => ({ hour: h, duration: 0 }));
      const total = buckets.reduce((a, b) => a + b.duration, 0);
      return {
        date,
        weekday: format(date, "EEE"),
        buckets,
        total,
        isToday: ago === 0,
      };
    });
  }, [queries, today]);

  const maxDuration = Math.max(
    1,
    ...days.flatMap((d) => d.buckets.map((b) => b.duration)),
  );

  const hourTotals = useMemo(() => {
    const totals = new Array(24).fill(0);
    for (const d of days) {
      for (const b of d.buckets) totals[b.hour] += b.duration;
    }
    return totals;
  }, [days]);

  const grandTotal = days.reduce((a, d) => a + d.total, 0);
  const peakHour = hourTotals.reduce(
    (best, t, h) => (t > best.total ? { hour: h, total: t } : best),
    { hour: 0, total: 0 },
  );
  const avgDay = grandTotal / DAYS;

  const isLoading = queries.some((q) => q.isLoading);

  return (
    <>
      <section className="grid grid-cols-3 gap-2.5">
        <SummaryStat label="7-day active total" value={fmtDuration(grandTotal)} loading={isLoading} />
        <SummaryStat label="Daily average" value={fmtDuration(avgDay)} loading={isLoading} />
        <SummaryStat
          label="Peak hour"
          value={`${String(peakHour.hour).padStart(2, "0")}:00`}
          sub={`${fmtDuration(peakHour.total)} across ${DAYS} days`}
          loading={isLoading}
        />
      </section>

      <WidgetCard
        title="Hour × day heatmap"
        description="Cell intensity = active seconds in that hour"
        action={
          <Badge variant="outline" className="font-mono tabular-nums uppercase">
            {DAYS}-day window
          </Badge>
        }
      >
        {isLoading ? (
          <Skeleton className="h-48 w-full rounded-sm" />
        ) : (
          <div className="overflow-x-auto">
            {/* Hour column header */}
            <div
              className="mb-1 grid gap-0.5 pl-16"
              style={{ gridTemplateColumns: "repeat(24, minmax(0, 1fr))" }}
            >
              {Array.from({ length: 24 }, (_, h) => (
                <div
                  key={h}
                  className="text-center font-mono tabular-nums text-[0.625rem] text-muted-foreground"
                >
                  {h % 3 === 0 ? String(h).padStart(2, "0") : ""}
                </div>
              ))}
            </div>

            {/* Heatmap rows */}
            {days.map((d) => (
              <div
                key={d.date.toISOString()}
                className="mb-0.5 grid items-center gap-0.5"
                style={{
                  gridTemplateColumns: "64px repeat(24, minmax(0, 1fr))",
                }}
              >
                <div
                  className={
                    "flex items-baseline justify-between pr-2 " +
                    (d.isToday ? "text-foreground" : "text-muted-foreground")
                  }
                >
                  <span>{d.weekday}</span>
                  <span className="font-mono tabular-nums text-[0.625rem] text-muted-foreground">
                    {format(d.date, "MMM d")}
                  </span>
                </div>
                {d.buckets.map((b) => (
                  <HeatCell
                    key={b.hour}
                    duration={b.duration}
                    max={maxDuration}
                    title={`${d.weekday} ${format(d.date, "MMM d")} · ${String(b.hour).padStart(2, "0")}:00 — ${fmtDuration(b.duration)}`}
                  />
                ))}
              </div>
            ))}

            {/* Column footer · totals across the week */}
            <div
              className="mt-1.5 grid items-center gap-0.5 border-t pt-1.5"
              style={{
                gridTemplateColumns: "64px repeat(24, minmax(0, 1fr))",
              }}
            >
              <div className="pr-2 font-medium uppercase tracking-wider text-[0.625rem] text-muted-foreground">
                week
              </div>
              {hourTotals.map((t, h) => (
                <HeatCell
                  key={h}
                  duration={t}
                  max={Math.max(1, ...hourTotals)}
                  title={`${String(h).padStart(2, "0")}:00 across ${DAYS} days — ${fmtDuration(t)}`}
                />
              ))}
            </div>

            {/* Legend */}
            <div className="mt-4 flex items-center gap-1.5 text-muted-foreground">
              <span>less</span>
              {[0, 0.2, 0.4, 0.6, 0.8, 1].map((step) => (
                <span
                  key={step}
                  className="h-2.5 w-3.5 rounded-sm border"
                  style={{
                    background:
                      step === 0
                        ? "var(--secondary)"
                        : `color-mix(in oklab, var(--chart-1) ${Math.round(step * 100)}%, transparent)`,
                  }}
                />
              ))}
              <span>more</span>
            </div>
          </div>
        )}
      </WidgetCard>
    </>
  );
}

function HeatCell({
  duration,
  max,
  title,
}: {
  duration: number;
  max: number;
  title: string;
}) {
  const intensity = Math.min(1, duration / max);
  const bg =
    duration === 0
      ? "var(--secondary)"
      : `color-mix(in oklab, var(--chart-1) ${Math.round(intensity * 100)}%, transparent)`;
  return (
    <div
      className="h-[18px] cursor-crosshair rounded-sm border border-border/30 hover:[box-shadow:0_0_0_1px_var(--ring)]"
      style={{ background: bg }}
      title={title}
    />
  );
}

function SummaryStat({
  label,
  value,
  sub,
  loading,
}: {
  label: string;
  value: string;
  sub?: string;
  loading?: boolean;
}) {
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription className="font-medium uppercase tracking-wider text-[0.625rem]">
          {label}
        </CardDescription>
        <CardTitle className="font-mono tabular-nums text-xl font-semibold tracking-tight">
          {loading ? <Skeleton className="h-6 w-24" /> : value}
        </CardTitle>
      </CardHeader>
      {sub && (
        <CardContent>
          <span className="text-muted-foreground">
            {loading ? <Skeleton className="h-3 w-32" /> : sub}
          </span>
        </CardContent>
      )}
    </Card>
  );
}
