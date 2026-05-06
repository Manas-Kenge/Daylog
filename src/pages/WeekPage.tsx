/**
 * Week page · 7-day calendar-week stacked bar chart, Mon → Sun.
 *
 * Calendar week (not trailing 7 days). If today is Wednesday, the bars
 * for Thu/Fri/Sat/Sun show as empty (axis label remains, no bar).
 *
 * The bottom callout names the day with the most Work-categorized hours.
 * Daylog is observational — the wording stays descriptive ("highest"),
 * not motivational ("strongest").
 */

import { useMemo } from "react";
import { addDays, format, subDays } from "date-fns";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { HugeiconsIcon } from "@hugeicons/react";
import { FlashIcon } from "@hugeicons/core-free-icons";
import { Badge } from "@/components/ui/badge";
import { WidgetCard } from "@/components/widgets/Card";
import { TopApps } from "@/components/widgets/TopApps";
import { TopCategories } from "@/components/widgets/TopCategories";
import { WebPanel } from "@/components/widgets/WebPanel";
import { Skeleton } from "@/components/ui/skeleton";
import { useTrailingDays } from "@/hooks/useAw";
import { categoryColor, categoryRoot } from "@/lib/category-colors";
import { LastNDays, type CategorizedEvent } from "@/lib/aw-types";
import { fmtDuration } from "@/lib/format";

const WEEK_RANGE = LastNDays(7);

/** Stable display order for category roots in the legend / stack. */
const ROOT_ORDER = ["Work", "Comms", "Documents", "Browsing", "Media", "Uncategorized"];

interface DayRow {
  /** Mon, Tue, ... — the X-axis tick. */
  day: string;
  /** ISO date string used as a tooltip identifier. */
  dateLabel: string;
  /** True when the day hasn't happened yet this week. */
  isFuture: boolean;
  /** Per-root hours; missing roots stay 0 so all bars stack consistently. */
  [root: string]: number | string | boolean;
}

interface WeekData {
  rows: DayRow[];
  roots: string[];
}

function isoMonday(today: Date): Date {
  const isoDow = today.getDay() === 0 ? 7 : today.getDay();
  return subDays(today, isoDow - 1);
}

function buildWeek(
  today: Date,
  trailing: ReturnType<typeof useTrailingDays>["data"],
): WeekData {
  const monday = isoMonday(today);
  const presentRoots = new Set<string>();
  const perDay: { date: Date; isFuture: boolean; events: CategorizedEvent[] }[] = [];

  for (let i = 0; i < 7; i++) {
    const date = addDays(monday, i);
    const isFuture =
      date.getTime() > today.getTime() &&
      date.toDateString() !== today.toDateString();
    if (isFuture) {
      perDay.push({ date, isFuture: true, events: [] });
      continue;
    }
    const daysAgo = Math.round(
      (today.getTime() - date.getTime()) / (24 * 3600 * 1000),
    );
    const slot = (trailing ?? []).find((d) => d.daysAgo === daysAgo);
    const events = slot?.events ?? [];
    for (const ev of events) presentRoots.add(categoryRoot(ev.category));
    perDay.push({ date, isFuture: false, events });
  }

  const roots = ROOT_ORDER.filter((r) => presentRoots.has(r)).concat(
    [...presentRoots].filter((r) => !ROOT_ORDER.includes(r)),
  );

  const rows: DayRow[] = perDay.map(({ date, isFuture, events }) => {
    const totals: Record<string, number> = {};
    for (const ev of events) {
      const root = categoryRoot(ev.category);
      totals[root] = (totals[root] ?? 0) + ev.duration / 3600;
    }
    const row: DayRow = {
      day: format(date, "EEE"),
      dateLabel: format(date, "MMM d"),
      isFuture,
    };
    for (const root of roots) row[root] = totals[root] ?? 0;
    return row;
  });

  return { rows, roots };
}

const WORK_ROOT = "Work";

export function WeekPage() {
  const { data: trailing, isLoading } = useTrailingDays(7);
  const today = useMemo(() => new Date(), []);
  const { rows, roots } = useMemo(
    () => buildWeek(today, trailing),
    [today, trailing],
  );

  const totalHours = rows.reduce(
    (a, r) => a + roots.reduce((b, root) => b + (r[root] as number), 0),
    0,
  );

  // "This week" stats. Calendar-week (Mon–Sun); future days are skipped
  // so the average reflects elapsed days, not the whole week.
  const weekStats = useMemo(() => {
    const elapsed = rows.filter((r) => !r.isFuture);
    const dayTotals = elapsed.map((r) => ({
      day: r.day,
      dateLabel: r.dateLabel as string,
      hours: roots.reduce((a, root) => a + (r[root] as number), 0),
    }));
    const total = dayTotals.reduce((a, d) => a + d.hours, 0);
    const daysElapsed = elapsed.length;
    const activeDays = dayTotals.filter((d) => d.hours > 0).length;
    const avg = daysElapsed > 0 ? total / daysElapsed : 0;
    const best = dayTotals.reduce<{
      day: string;
      dateLabel: string;
      hours: number;
    } | null>(
      (b, d) => (d.hours > 0 && (!b || d.hours > b.hours) ? d : b),
      null,
    );
    return { total, avg, daysElapsed, activeDays, best };
  }, [rows, roots]);

  // Highest Work day this week. Daylog is observational so we describe
  // facts ("highest"), not value judgments ("strongest").
  const highestWork = rows.reduce<{ day: string; dateLabel: string; hours: number } | null>(
    (best, r) => {
      const h = (r[WORK_ROOT] as number) ?? 0;
      if (h <= 0) return best;
      if (!best || h > best.hours) {
        return { day: r.day, dateLabel: r.dateLabel as string, hours: h };
      }
      return best;
    },
    null,
  );

  return (
    <>
      <div className="flex min-w-0 flex-wrap items-stretch gap-2.5">
      <div className="min-w-0 flex-1">
      <WidgetCard
        title="7-Day Activity Breakdown"
        description="Stacked by category · hours"
        action={<Legend roots={roots} />}
      >
        {isLoading ? (
          <Skeleton className="h-72 w-full rounded-sm" />
        ) : (
          <div className="h-72 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart
                data={rows}
                margin={{ top: 12, right: 12, left: 0, bottom: 8 }}
              >
                <CartesianGrid vertical={false} stroke="var(--border)" strokeDasharray="2 4" />
                <XAxis
                  dataKey="day"
                  tickLine={false}
                  axisLine={false}
                  stroke="var(--muted-foreground)"
                  fontSize={12}
                />
                <YAxis
                  tickLine={false}
                  axisLine={false}
                  width={36}
                  stroke="var(--muted-foreground)"
                  fontSize={12}
                  tickFormatter={(v) => `${v}h`}
                />
                <Tooltip
                  cursor={{ fill: "var(--accent)", opacity: 0.25 }}
                  content={<HoursTooltip roots={roots} />}
                />
                {roots.map((root, i) => (
                  <Bar
                    key={root}
                    dataKey={root}
                    stackId="cats"
                    fill={categoryColor([root])}
                    radius={i === roots.length - 1 ? [3, 3, 0, 0] : 0}
                    isAnimationActive={false}
                    maxBarSize={42}
                  />
                ))}
              </BarChart>
            </ResponsiveContainer>
          </div>
        )}

        {!isLoading && (
          <div className="mt-4">
            {highestWork ? (
              <Insight
                day={highestWork.day}
                dateLabel={highestWork.dateLabel}
                hours={highestWork.hours}
              />
            ) : totalHours === 0 ? (
              <EmptyInsight>
                No tracked activity yet this week. Pattern callouts appear once
                Daylog has data.
              </EmptyInsight>
            ) : (
              <EmptyInsight>
                No Work-categorized time this week — set up category rules in
                Settings to enable Work callouts.
              </EmptyInsight>
            )}
          </div>
        )}
      </WidgetCard>
      </div>
      <div className="min-w-0 w-full md:w-80 lg:w-96">
        <ThisWeekCard
          loading={isLoading}
          total={weekStats.total}
          avg={weekStats.avg}
          activeDays={weekStats.activeDays}
          daysElapsed={weekStats.daysElapsed}
          best={weekStats.best}
        />
      </div>
      </div>

      <section className="grid min-w-0 grid-cols-3 items-start gap-2.5">
        <TopApps
          rangeOverride={WEEK_RANGE}
          title="Top apps · 7 days"
          description="Active time per app, last 7 days"
        />
        <TopCategories
          rangeOverride={WEEK_RANGE}
          title="Top categories · 7 days"
          description="Time per category root, last 7 days"
        />
        <WebPanel
          rangeOverride={WEEK_RANGE}
          title="Top domains · 7 days"
          description="Active time per domain, last 7 days"
        />
      </section>
    </>
  );
}

function Legend({ roots }: { roots: string[] }) {
  return (
    <div className="flex flex-wrap items-center gap-3 text-muted-foreground">
      {roots.map((root) => (
        <span key={root} className="inline-flex items-center gap-1.5">
          <span
            className="size-2 rounded-full"
            style={{ background: categoryColor([root]) }}
          />
          <span>{root}</span>
        </span>
      ))}
    </div>
  );
}

function HoursTooltip({
  active,
  payload,
  label,
  roots,
}: {
  active?: boolean;
  payload?: ReadonlyArray<{ name: string; value: number; color: string }>;
  label?: string;
  roots: string[];
}) {
  if (!active || !payload?.length) return null;
  const total = payload.reduce((a, p) => a + (p.value ?? 0), 0);
  const ordered = roots
    .map((r) => payload.find((p) => p.name === r))
    .filter((p): p is { name: string; value: number; color: string } => p != null);
  return (
    <div className="rounded-md border bg-popover px-3 py-2 text-xs shadow-md">
      <div className="mb-1 font-medium">{label}</div>
      {ordered.map((p) => (
        <div key={p.name} className="flex items-center gap-2">
          <span
            className="size-2 rounded-sm"
            style={{ background: p.color }}
          />
          <span className="text-muted-foreground">{p.name}</span>
          <span className="ml-auto font-mono tabular-nums">
            {fmtDuration(p.value * 3600)}
          </span>
        </div>
      ))}
      <div className="mt-1 flex justify-between border-t pt-1 font-mono tabular-nums">
        <span className="text-muted-foreground">total</span>
        <span>{fmtDuration(total * 3600)}</span>
      </div>
    </div>
  );
}

function Insight({
  day,
  dateLabel,
  hours,
}: {
  day: string;
  dateLabel: string;
  hours: number;
}) {
  return (
    <div className="flex items-center gap-2 rounded-md bg-secondary/60 px-3 py-2.5">
      <HugeiconsIcon
        icon={FlashIcon}
        size={14}
        className="shrink-0 text-foreground"
      />
      <span>
        <span className="font-medium text-foreground">
          {day} ({dateLabel})
        </span>{" "}
        had your highest Work hours this week —{" "}
        <span className="font-mono tabular-nums">{hours.toFixed(1)}h</span>.
      </span>
    </div>
  );
}

function EmptyInsight({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-md bg-secondary/60 px-3 py-2.5 text-muted-foreground">
      {children}
    </div>
  );
}

function ThisWeekCard({
  loading,
  total,
  avg,
  activeDays,
  daysElapsed,
  best,
}: {
  loading: boolean;
  total: number;
  avg: number;
  activeDays: number;
  daysElapsed: number;
  best: { day: string; dateLabel: string; hours: number } | null;
}) {
  return (
    <WidgetCard
      title="This week"
      description="Mon–Sun, at a glance"
      action={
        <Badge variant="outline" className="font-mono tabular-nums uppercase">
          {activeDays}/{daysElapsed} active
        </Badge>
      }
    >
      {loading ? (
        <Skeleton className="h-32 w-full rounded-sm" />
      ) : (
        <dl className="grid grid-cols-1 gap-1.5">
          <Stat label="Total" value={fmtDuration(total * 3600)} />
          <Stat
            label="Daily average"
            value={daysElapsed > 0 ? fmtDuration(avg * 3600) : "—"}
            hint={daysElapsed > 0 ? `over ${daysElapsed} day${daysElapsed === 1 ? "" : "s"}` : undefined}
          />
          <Stat
            label="Best day"
            value={best ? fmtDuration(best.hours * 3600) : "—"}
            hint={best ? `${best.day} · ${best.dateLabel}` : undefined}
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
