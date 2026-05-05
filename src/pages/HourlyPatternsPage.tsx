/**
 * Hourly patterns · 7-day × 24-hour heatmap. Fetched via 7 parallel
 * `aw_hourly({DaysAgo: n})` queries; each cell is shaded by relative
 * activity intensity. Surfaces "when do I actually work" patterns that
 * Overview's single-day strip can't show.
 */

import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import { format, subDays } from "date-fns";
import { WidgetCard } from "@/components/widgets/Card";
import { awHourly } from "@/lib/aw";
import { DaysAgo, type HourBucket } from "@/lib/aw-types";
import { fmtDuration } from "@/lib/format";

const DAYS = 7;

interface DayCell {
  date: Date;
  weekday: string;
  buckets: HourBucket[]; // 24 entries, hour 0..23
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
    // Oldest first so the grid reads top-to-bottom in chronological order.
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

  // Per-hour totals across the 7-day window (column footer).
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
      <section className="grid grid-cols-3 gap-[10px]">
        <SummaryStat label="7-day active total" value={fmtDuration(grandTotal)} />
        <SummaryStat label="Daily average" value={fmtDuration(avgDay)} />
        <SummaryStat
          label="Peak hour"
          value={`${String(peakHour.hour).padStart(2, "0")}:00`}
          sub={`${fmtDuration(peakHour.total)} across ${DAYS} days`}
        />
      </section>

      <WidgetCard
        title="Hour × day heatmap"
        description="Cell intensity = active seconds in that hour"
        action={
          <span className="mono text-[10.5px] text-muted-foreground tracking-[0.13em] uppercase">
            {DAYS}-day window
          </span>
        }
      >
        {isLoading ? (
          <div className="text-muted-foreground text-[12px] py-[24px] text-center">
            loading…
          </div>
        ) : (
          <div className="overflow-x-auto">
            {/* Hour column header */}
            <div
              className="grid pl-[64px] gap-[2px] mb-[4px]"
              style={{ gridTemplateColumns: "repeat(24, minmax(0, 1fr))" }}
            >
              {Array.from({ length: 24 }, (_, h) => (
                <div
                  key={h}
                  className="text-[9.5px] mono text-muted-foreground text-center"
                >
                  {h % 3 === 0 ? String(h).padStart(2, "0") : ""}
                </div>
              ))}
            </div>

            {/* Heatmap rows */}
            {days.map((d) => (
              <div
                key={d.date.toISOString()}
                className="grid items-center gap-[2px] mb-[2px]"
                style={{
                  gridTemplateColumns: "64px repeat(24, minmax(0, 1fr))",
                }}
              >
                <div
                  className={
                    "flex items-baseline justify-between pr-[8px] text-[11px] " +
                    (d.isToday ? "text-foreground" : "text-muted-foreground")
                  }
                >
                  <span>{d.weekday}</span>
                  <span className="mono text-[10px] text-muted-foreground">
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
              className="grid items-center gap-[2px] mt-[6px] pt-[6px] border-t border-border"
              style={{
                gridTemplateColumns: "64px repeat(24, minmax(0, 1fr))",
              }}
            >
              <div className="text-[10px] tracking-[0.13em] uppercase text-muted-foreground font-medium pr-[8px]">
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
            <div className="flex items-center gap-[6px] mt-[16px] text-[10.5px] text-muted-foreground">
              <span>less</span>
              {[0, 0.2, 0.4, 0.6, 0.8, 1].map((step) => (
                <span
                  key={step}
                  className="w-[14px] h-[10px] rounded-[2px] border border-border"
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
      className="h-[18px] rounded-[2px] border border-border/30 hover:[box-shadow:0_0_0_1px_var(--brand-coral)] cursor-crosshair"
      style={{ background: bg }}
      title={title}
    />
  );
}

function SummaryStat({
  label,
  value,
  sub,
}: {
  label: string;
  value: string;
  sub?: string;
}) {
  return (
    <div className="bg-card border border-border rounded-[var(--radius-lg)] px-[14px] py-[12px]">
      <div className="text-[10px] tracking-[0.13em] uppercase text-muted-foreground font-medium">
        {label}
      </div>
      <div className="mono text-[20px] font-semibold tracking-tight mt-[2px]">
        {value}
      </div>
      {sub && <div className="text-[11px] text-muted-foreground mt-[2px]">{sub}</div>}
    </div>
  );
}
