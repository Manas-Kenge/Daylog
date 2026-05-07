/**
 * Calendar week (not trailing 7 days), matching WeekPage's convention —
 * future days render as empty cells.
 *
 * Pulls one `awHourly(DaysAgo(n))` per applicable weekday. Past-day
 * queries share the `aw_hourly_daysago` key so reopening Overview after
 * WeekPage is instant.
 *
 * Layout uses a single CSS grid with explicit row heights — aspect-ratio
 * cells fight the parent's flex-1/min-h-0 constraints in narrow rails.
 */

import { Fragment, useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import { Badge } from "@/components/ui/badge";
import { WidgetCard } from "@/components/widgets/Card";
import { Skeleton } from "@/components/ui/skeleton";
import { awHourly } from "@/lib/aw";
import { DaysAgo, type HourBucket } from "@/lib/aw-types";
import { fmtDuration } from "@/lib/format";
import { cn } from "@/lib/utils";

const PAST_DAY_STALE_MS = 5 * 60_000;
const REFRESH_MS = 5_000;

const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;
const HOUR_BANDS: { label: string; start: number; end: number }[] = [
  { label: "00", start: 0, end: 4 },
  { label: "04", start: 4, end: 8 },
  { label: "08", start: 8, end: 12 },
  { label: "12", start: 12, end: 16 },
  { label: "16", start: 16, end: 20 },
  { label: "20", start: 20, end: 24 },
];

const HEADER_ROW_PX = 14;
const CELL_MIN_ROW_PX = 24;
const LABEL_COL_PX = 22;

export function WeekHeatmap() {
  const today = useMemo(() => new Date(), []);
  // Mon = 0, Sun = 6
  const todayWeekIdx = (today.getDay() + 6) % 7;

  const queries = useQueries({
    queries: Array.from({ length: 7 }, (_, weekIdx) => {
      const daysAgo = todayWeekIdx - weekIdx;
      const isFuture = daysAgo < 0;
      const isToday = daysAgo === 0;
      return {
        queryKey: ["aw_hourly_daysago", daysAgo],
        queryFn: () => awHourly(DaysAgo(daysAgo)),
        enabled: !isFuture,
        refetchInterval: (isToday ? REFRESH_MS : false) as number | false,
        staleTime: isToday ? 0 : PAST_DAY_STALE_MS,
      };
    }),
  });

  const isLoading = queries.some((q) => q.isLoading && q.fetchStatus !== "idle");

  // matrix[bandIdx][weekdayIdx] = duration in seconds
  const matrix = useMemo(() => {
    return HOUR_BANDS.map((band) =>
      queries.map((q) => {
        const buckets: HourBucket[] = q.data ?? [];
        let sum = 0;
        for (const b of buckets) {
          if (b.hour >= band.start && b.hour < band.end) sum += b.duration;
        }
        return sum;
      }),
    );
  }, [queries]);

  const max = useMemo(() => {
    let m = 0;
    for (const row of matrix) for (const v of row) if (v > m) m = v;
    return m;
  }, [matrix]);

  const total = useMemo(() => {
    let t = 0;
    for (const row of matrix) for (const v of row) t += v;
    return t;
  }, [matrix]);

  return (
    <WidgetCard
      title="This week"
      description="Activity by hour-band, calendar week"
      action={
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          {fmtDuration(total)}
        </Badge>
      }
    >
      {isLoading ? (
        <Skeleton className="h-40 w-full rounded-sm" />
      ) : (
        <div
          className="grid h-full w-full gap-1"
          style={{
            gridTemplateColumns: `${LABEL_COL_PX}px repeat(7, minmax(0, 1fr))`,
            gridTemplateRows: `${HEADER_ROW_PX}px repeat(6, minmax(${CELL_MIN_ROW_PX}px, 1fr))`,
          }}
        >
          {/* Top-left corner */}
          <div />
          {/* Column headers (Mon–Sun) */}
          {WEEKDAY_LABELS.map((d, i) => (
            <div
              key={d}
              className={cn(
                "self-center text-center text-[0.625rem] text-muted-foreground",
                i === todayWeekIdx && "font-medium text-foreground",
              )}
            >
              {d.charAt(0)}
            </div>
          ))}
          {/* Body rows */}
          {HOUR_BANDS.map((band, ri) => (
            <Fragment key={band.label}>
              <div className="self-center text-right font-mono text-[0.625rem] text-muted-foreground">
                {band.label}
              </div>
              {matrix[ri].map((value, ci) => (
                <Cell
                  key={`${ri}-${ci}`}
                  value={value}
                  max={max}
                  isFuture={ci > todayWeekIdx}
                  weekday={WEEKDAY_LABELS[ci]}
                  band={band}
                />
              ))}
            </Fragment>
          ))}
        </div>
      )}
    </WidgetCard>
  );
}

function Cell({
  value,
  max,
  isFuture,
  weekday,
  band,
}: {
  value: number;
  max: number;
  isFuture: boolean;
  weekday: string;
  band: { label: string; start: number; end: number };
}) {
  const intensity = max > 0 ? Math.min(1, value / max) : 0;
  const bg = isFuture
    ? "transparent"
    : value === 0
      ? "var(--secondary)"
      : `color-mix(in oklab, var(--chart-1) ${Math.round(intensity * 100)}%, transparent)`;
  const title = isFuture
    ? `${weekday} (future)`
    : `${weekday} ${band.label}–${String(band.end).padStart(2, "0")} · ${fmtDuration(value)}`;
  return (
    <div
      className="h-full w-full rounded-sm border border-border/30"
      style={{ background: bg }}
      title={title}
    />
  );
}
