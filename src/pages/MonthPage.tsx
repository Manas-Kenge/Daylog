/**
 * Month page · GitHub-style contribution heatmap of daily active total.
 *
 * Trailing year laid out as ~53 weekly columns × 7 weekday rows, with
 * fixed 14×14 cells. Month labels above the first column of each month;
 * weekday labels (Mon/Wed/Fri) on the left. Today is the most-recent
 * populated cell, highlighted. Days with no activity are left uncolored
 * so the eye picks up only days with real data.
 *
 * Pulls one `awAfkSummary(DaysAgo(n))` per day. Past-day queries dedupe
 * across pages via the shared `aw_afk_summary_daysago` key, so reopening
 * MonthPage after WeekPage/topbar pre-warmed the cache is instant.
 * TODO: consolidate into one Rust command (`aw_daily_active_seconds`)
 * to drop 365 IPC roundtrips to one.
 */

import { useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import { format, subDays } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { WidgetCard } from "@/components/widgets/Card";
import { TopApps } from "@/components/widgets/TopApps";
import { TopCategories } from "@/components/widgets/TopCategories";
import { WebPanel } from "@/components/widgets/WebPanel";
import { Skeleton } from "@/components/ui/skeleton";
import { awAfkSummary } from "@/lib/aw";
import { DaysAgo, LastNDays } from "@/lib/aw-types";
import { fmtDuration } from "@/lib/format";

const DAYS = 365;
const MONTH_RANGE = LastNDays(30);
const PAST_DAY_STALE_MS = 5 * 60_000;

const CELL_PX = 10;
const GAP_PX = 3;

const WEEKDAY_LABELS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;
/** Which weekday-row labels to actually render (others stay reserved-but-blank
 *  so the column visually aligns). Matches GitHub's pattern. */
const VISIBLE_WEEKDAY_INDICES = new Set([1, 3, 5]);

interface DayCell {
  daysAgo: number;
  date: Date;
  weekday: number; // 0 = Sun
  isToday: boolean;
  activeSec: number;
}

interface Column {
  /** First-of-month label rendered above the column when it starts a new month. */
  monthLabel: string | null;
  cells: (DayCell | null)[];
}

export function MonthPage() {
  const today = useMemo(() => new Date(), []);

  const queries = useQueries({
    queries: Array.from({ length: DAYS }, (_, n) => ({
      queryKey: ["aw_afk_summary_daysago", false, n],
      queryFn: () => awAfkSummary(DaysAgo(n), false),
      staleTime: n === 0 ? 0 : PAST_DAY_STALE_MS,
    })),
  });

  const isLoading = queries.some((q) => q.isLoading);

  const cells = useMemo<DayCell[]>(() => {
    return queries.map((q, n) => {
      const date = subDays(today, n);
      return {
        daysAgo: n,
        date,
        weekday: date.getDay(),
        isToday: n === 0,
        activeSec: q.data?.active_seconds ?? 0,
      };
    });
  }, [queries, today]);

  // Build the column layout: today's column is rightmost; today's cell
  // sits at row=today.weekday. Each preceding cell walks the row index
  // back; on row underflow we move one column left.
  const columns = useMemo<Column[]>(() => {
    const cols: Column[] = [];
    let cur: (DayCell | null)[] = Array.from({ length: 7 }, () => null);
    let row = today.getDay();
    for (const cell of cells) {
      cur[row] = cell;
      row--;
      if (row < 0) {
        cols.push({ monthLabel: null, cells: cur });
        cur = Array.from({ length: 7 }, () => null);
        row = 6;
      }
    }
    cols.push({ monthLabel: null, cells: cur });
    cols.reverse(); // oldest column on the left

    // Tag the first column of each month with its label.
    let lastMonth = -1;
    for (const col of cols) {
      const firstDay = col.cells.find((c) => c != null);
      if (!firstDay) continue;
      const m = firstDay.date.getMonth();
      if (m !== lastMonth) {
        col.monthLabel = format(firstDay.date, "MMM");
        lastMonth = m;
      }
    }
    return cols;
  }, [cells, today]);

  const total = cells.reduce((a, c) => a + c.activeSec, 0);
  const maxActive = Math.max(1, ...cells.map((c) => c.activeSec));

  // First 30 entries (n=0..29) = trailing month — same window as the
  // TopApps/TopCategories/WebPanel grid below. Stats reuse data already
  // fetched for the heatmap.
  const monthStats = useMemo(() => {
    const m = cells.slice(0, 30);
    const sum = m.reduce((a, c) => a + c.activeSec, 0);
    const activeDayCount = m.filter((c) => c.activeSec > 0).length;
    const avg = activeDayCount > 0 ? sum / activeDayCount : 0;
    let best: DayCell | null = null;
    for (const c of m) {
      if (c.activeSec > 0 && (!best || c.activeSec > best.activeSec)) best = c;
    }
    // Trailing-edge streak: consecutive active days starting from today.
    let streak = 0;
    for (const c of m) {
      if (c.activeSec > 0) streak++;
      else break;
    }
    return { total: sum, activeDayCount, avg, best, streak };
  }, [cells]);

  return (
    <>
    <div className="flex min-w-0 flex-wrap items-stretch gap-2.5">
    <div className="min-w-0 flex-none">
    <WidgetCard
      title="Daily activity"
      description="Active time per day, GitHub-style"
      action={
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          {fmtDuration(total)} · last year
        </Badge>
      }
    >
      {isLoading ? (
        <Skeleton className="h-32 w-full rounded-sm" />
      ) : (
        <div className="overflow-x-auto pb-2">
          <div
            className="inline-flex flex-col gap-1"
            style={{ minWidth: "fit-content" }}
          >
            {/* Month-label row */}
            <div
              className="flex"
              style={{ gap: `${GAP_PX}px`, paddingLeft: `${CELL_PX * 2 + GAP_PX * 2}px` }}
            >
              {columns.map((col, i) => (
                <div
                  key={i}
                  className="text-[0.625rem] text-muted-foreground"
                  style={{ width: `${CELL_PX}px` }}
                >
                  {col.monthLabel ?? " "}
                </div>
              ))}
            </div>

            {/* Heatmap body: weekday-label column + week columns */}
            <div className="flex" style={{ gap: `${GAP_PX}px` }}>
              {/* Weekday labels */}
              <div
                className="flex flex-col"
                style={{ gap: `${GAP_PX}px`, width: `${CELL_PX * 2 + GAP_PX}px` }}
              >
                {WEEKDAY_LABELS.map((label, i) => (
                  <div
                    key={label}
                    className="text-right text-[0.625rem] text-muted-foreground"
                    style={{
                      height: `${CELL_PX}px`,
                      lineHeight: `${CELL_PX}px`,
                    }}
                  >
                    {VISIBLE_WEEKDAY_INDICES.has(i) ? label : " "}
                  </div>
                ))}
              </div>

              {/* Week columns */}
              {columns.map((col, ci) => (
                <div
                  key={ci}
                  className="flex flex-col"
                  style={{ gap: `${GAP_PX}px` }}
                >
                  {col.cells.map((cell, ri) => (
                    <Cell key={ri} cell={cell} max={maxActive} />
                  ))}
                </div>
              ))}
            </div>

            {/* Legend */}
            <Legend max={maxActive} />
          </div>
        </div>
      )}
    </WidgetCard>
    </div>
    <div className="min-w-0 flex-1">
      <ThisMonthCard
        loading={isLoading}
        total={monthStats.total}
        avg={monthStats.avg}
        activeDays={monthStats.activeDayCount}
        best={monthStats.best}
        streak={monthStats.streak}
      />
    </div>
    </div>

    <section className="grid min-w-0 grid-cols-3 items-start gap-2.5">
      <TopApps
        rangeOverride={MONTH_RANGE}
        showSparklines={false}
        title="Top apps · 30 days"
        description="Active time per app, last 30 days"
      />
      <TopCategories
        rangeOverride={MONTH_RANGE}
        title="Top categories · 30 days"
        description="Time per category root, last 30 days"
      />
      <WebPanel
        rangeOverride={MONTH_RANGE}
        title="Top domains · 30 days"
        description="Active time per domain, last 30 days"
      />
    </section>
    </>
  );
}

function Cell({ cell, max }: { cell: DayCell | null; max: number }) {
  const baseStyle = { width: `${CELL_PX}px`, height: `${CELL_PX}px` };
  if (!cell) {
    return <div className="rounded-sm" style={baseStyle} />;
  }
  const hasData = cell.activeSec > 0;
  const intensity = max > 0 ? Math.min(1, cell.activeSec / max) : 0;
  const bg = hasData
    ? `color-mix(in oklab, var(--chart-1) ${Math.round(intensity * 100)}%, transparent)`
    : "transparent";
  const ring = cell.isToday ? "ring-1 ring-foreground/70" : "";
  const borderClass = hasData ? "border-border/30" : "border-border/15";
  return (
    <div
      className={`rounded-sm border ${borderClass} ${ring}`}
      style={{ ...baseStyle, background: bg }}
      title={`${format(cell.date, "EEE MMM d")} · ${fmtDuration(cell.activeSec)}`}
    />
  );
}

function ThisMonthCard({
  loading,
  total,
  avg,
  activeDays,
  best,
  streak,
}: {
  loading: boolean;
  total: number;
  avg: number;
  activeDays: number;
  best: DayCell | null;
  streak: number;
}) {
  return (
    <WidgetCard
      title="This month"
      description="Last 30 days, at a glance"
      action={
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          {activeDays}/30 active
        </Badge>
      }
    >
      {loading ? (
        <Skeleton className="h-32 w-full rounded-sm" />
      ) : (
        <dl className="grid grid-cols-1 gap-1.5">
          <Stat label="Total active" value={fmtDuration(total)} />
          <Stat
            label="Daily average"
            value={activeDays > 0 ? fmtDuration(avg) : "—"}
            hint={activeDays > 0 ? `over ${activeDays} active days` : undefined}
          />
          <Stat
            label="Best day"
            value={best ? fmtDuration(best.activeSec) : "—"}
            hint={best ? format(best.date, "EEE MMM d") : undefined}
          />
          <Stat
            label="Current streak"
            value={streak > 0 ? `${streak} day${streak === 1 ? "" : "s"}` : "—"}
            hint={
              streak === 0
                ? "no activity today yet"
                : streak >= 30
                  ? "all month"
                  : undefined
            }
          />
        </dl>
      )}
    </WidgetCard>
  );
}

function Stat({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <div className="flex items-baseline justify-between gap-3 rounded-sm bg-muted/30 px-2.5 py-2">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="flex items-baseline gap-2 text-right">
        {hint ? (
          <span className="text-[0.625rem] text-muted-foreground">{hint}</span>
        ) : null}
        <span className="font-mono tabular-nums text-sm">{value}</span>
      </dd>
    </div>
  );
}

function Legend({ max }: { max: number }) {
  return (
    <div
      className="mt-3 flex items-center gap-1.5 text-muted-foreground"
      style={{ paddingLeft: `${CELL_PX * 2 + GAP_PX * 2}px` }}
    >
      <span className="text-[0.625rem]">Less</span>
      {[0, 0.2, 0.4, 0.6, 0.8, 1].map((step) => (
        <span
          key={step}
          className={`rounded-sm border ${step === 0 ? "border-border/15" : "border-border/30"}`}
          style={{
            width: `${CELL_PX}px`,
            height: `${CELL_PX}px`,
            background:
              step === 0
                ? "transparent"
                : `color-mix(in oklab, var(--chart-1) ${Math.round(step * 100)}%, transparent)`,
          }}
        />
      ))}
      <span className="text-[0.625rem]">More</span>
      {max > 0 && (
        <span className="ml-3 font-mono tabular-nums text-[0.625rem]">
          peak {fmtDuration(max)}
        </span>
      )}
    </div>
  );
}
