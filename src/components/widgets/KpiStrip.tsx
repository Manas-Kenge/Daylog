/**
 * 5-up KPI tray. Discovery-shaped, not score-shaped (PLAN.md §1.0).
 *
 *  1. Active        — total active seconds, vs trailing-7-day median
 *  2. Best Window   — densest 3h focus window of the day
 *  3. Longest       — biggest uninterrupted run on a single category
 *  4. Cadence       — start → end (or 'now') + idle gap count
 *  5. Pattern shift — largest category-time delta vs trailing baseline
 *
 * Each card carries a "vs typical" sub-line gated on ≥1 effective baseline
 * day; below that we show a build-up placeholder.
 */

import { Area, AreaChart, Bar, BarChart, Cell, ReferenceLine, Tooltip } from "recharts";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Activity03Icon,
  Clock01Icon,
  RocketIcon,
  Sun01Icon,
  Target02Icon,
} from "@hugeicons/core-free-icons";
import type { IconSvgElement } from "@hugeicons/react";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import {
  ChartContainer,
  type ChartConfig,
} from "@/components/ui/chart";
import { Skeleton } from "@/components/ui/skeleton";
import {
  useAfkSummary,
  useCategorizedEvents,
  useHourly,
  useTrailingDays,
} from "@/hooks/useAw";
import { fmtClock, fmtDuration } from "@/lib/format";
import {
  firstActivity,
  focusByHour,
  longestFocus,
} from "@/lib/kpi";
import { bestWindow } from "@/lib/best-window";
import { cadence } from "@/lib/cadence";
import { trailingStats } from "@/lib/baselines";
import { dominantShift } from "@/lib/anomaly";
import { categoryRoot } from "@/lib/category-colors";
import type { CategorizedEvent } from "@/lib/aw-types";
import { useId, useMemo, type ReactNode } from "react";

interface SparkProps {
  values: number[];
  configKey: string;
  color: string;
  format: (v: number) => string;
  shape?: "area" | "bar";
  /** Single index to mark visually. Area: vertical dashed line. Bar:
   *  brighter fill at that index. */
  markX?: number | null;
  /** Inclusive range [start, end] to highlight in a bar spark. Used by
   *  the Best Window card to show its 3-hour band. */
  markRange?: [number, number] | null;
}

interface KpiCardProps {
  icon: IconSvgElement;
  label: string;
  value: ReactNode;
  sub?: ReactNode;
  delta?: { text: string; tone: "up" | "down" | "flat" };
  spark?: SparkProps;
  loading?: boolean;
  emptyHint?: string;
  tip?: string;
}

function SparkTooltip(props: {
  active?: boolean;
  payload?: ReadonlyArray<{ payload: { x: number; value: number } }>;
  format: (v: number) => string;
}) {
  if (!props.active || !props.payload?.length) return null;
  const { x, value } = props.payload[0].payload;
  return (
    <div className="rounded-md border bg-popover px-2 py-1 text-xs shadow-md">
      <span className="font-mono tabular-nums text-muted-foreground">
        {String(x).padStart(2, "0")}:00
      </span>{" "}
      <span className="font-mono tabular-nums font-medium text-foreground">
        {props.format(value)}
      </span>
    </div>
  );
}

function KpiCard({
  icon,
  label,
  value,
  sub,
  delta,
  spark,
  loading,
  emptyHint,
  tip,
}: KpiCardProps) {
  const gradId = useId();
  const data = (spark?.values ?? []).map((v, i) => ({ x: i, value: v }));
  const hasSpark = spark !== undefined;
  const hasData = data.some((d) => d.value > 0);
  const colorVar = spark ? `var(--color-${spark.configKey})` : undefined;
  const config: ChartConfig = spark
    ? { [spark.configKey]: { label, color: spark.color } }
    : {};

  const inMarkRange = (i: number): boolean => {
    if (spark?.markRange == null) return false;
    return i >= spark.markRange[0] && i <= spark.markRange[1];
  };

  if (loading) {
    return (
      <Card size="sm" className="gap-0 overflow-hidden py-0">
        <Skeleton className="h-[68px] rounded-none" />
        <div className="flex items-start justify-between gap-3 px-3 py-2.5">
          <div className="min-w-0 flex-1 flex flex-col gap-1.5">
            <Skeleton className="h-3 w-3/4" />
            <Skeleton className="h-3 w-1/2" />
          </div>
          <div className="flex flex-col items-end gap-1.5">
            <Skeleton className="h-4 w-16" />
            <Skeleton className="h-3 w-10" />
          </div>
        </div>
      </Card>
    );
  }

  return (
    <Card size="sm" className="gap-0 overflow-hidden py-0">
      <div className="-mb-px h-[68px]">
        {hasSpark && hasData ? (
          <ChartContainer config={config} className="aspect-auto h-full w-full">
            {spark!.shape === "bar" ? (
              <BarChart data={data} margin={{ top: 4, right: 0, left: 0, bottom: 0 }}>
                <Tooltip
                  cursor={{ fill: "var(--accent)", opacity: 0.4 }}
                  content={(props: unknown) => (
                    <SparkTooltip
                      {...(props as { active?: boolean; payload?: ReadonlyArray<{ payload: { x: number; value: number } }> })}
                      format={spark!.format}
                    />
                  )}
                />
                <Bar dataKey="value" radius={[1, 1, 0, 0]} isAnimationActive={false}>
                  {data.map((_, i) => (
                    <Cell
                      key={i}
                      fill={
                        spark!.markX != null && i === spark!.markX
                          ? "var(--foreground)"
                          : inMarkRange(i)
                            ? "var(--foreground)"
                            : colorVar
                      }
                    />
                  ))}
                </Bar>
              </BarChart>
            ) : (
              <AreaChart data={data} margin={{ top: 4, right: 0, left: 0, bottom: 0 }}>
                <defs>
                  <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor={colorVar} stopOpacity={0.32} />
                    <stop offset="100%" stopColor={colorVar} stopOpacity={0.02} />
                  </linearGradient>
                </defs>
                <ReferenceLine
                  y={spark!.values.reduce((a, b) => a + b, 0) / 24}
                  stroke="var(--border)"
                  strokeDasharray="2 3"
                  strokeWidth={1}
                />
                {spark!.markX != null && (
                  <ReferenceLine
                    x={spark!.markX}
                    stroke="var(--foreground)"
                    strokeDasharray="3 2"
                    strokeWidth={1}
                  />
                )}
                <Tooltip
                  cursor={{ stroke: colorVar, strokeWidth: 1, strokeDasharray: "2 2" }}
                  content={(props: unknown) => (
                    <SparkTooltip
                      {...(props as { active?: boolean; payload?: ReadonlyArray<{ payload: { x: number; value: number } }> })}
                      format={spark!.format}
                    />
                  )}
                />
                <Area
                  type="monotone"
                  dataKey="value"
                  stroke={colorVar}
                  strokeWidth={1.5}
                  fill={`url(#${gradId})`}
                  isAnimationActive={false}
                />
              </AreaChart>
            )}
          </ChartContainer>
        ) : (
          <div className="flex h-full items-center justify-center text-[0.625rem] text-muted-foreground">
            {hasSpark ? "no activity" : ""}
          </div>
        )}
      </div>

      <div className="flex items-start justify-between gap-3 px-3 py-2.5">
        <div className="min-w-0 flex-1">
          <div
            className="flex items-center gap-1.5 truncate text-[0.625rem] font-medium uppercase tracking-wider text-muted-foreground"
            title={tip}
          >
            <HugeiconsIcon icon={icon} size={11} />
            <span className="truncate">{label}</span>
          </div>
          {sub && (
            <div className="mt-0.5 truncate text-muted-foreground">{sub}</div>
          )}
          {emptyHint && (
            <div className="mt-0.5 truncate text-[0.625rem] text-muted-foreground/70">
              {emptyHint}
            </div>
          )}
        </div>
        <div className="flex shrink-0 flex-col items-end gap-0.5">
          <div className="font-mono tabular-nums text-lg font-semibold leading-tight tracking-tight text-foreground">
            {value}
          </div>
          {delta && (
            <Badge
              variant={
                delta.tone === "down"
                  ? "destructive"
                  : delta.tone === "up"
                    ? "outline"
                    : "secondary"
              }
              className="font-mono tabular-nums"
            >
              {delta.text}
            </Badge>
          )}
        </div>
      </div>
    </Card>
  );
}

/** Format a "vs typical" sub-line. Returns null when baseline isn't ready. */
function vsTypical(
  todayVal: number,
  baselineMedian: number,
  effectiveDays: number,
  fmt: (n: number) => string,
): string | null {
  if (effectiveDays === 0) return null;
  const delta = todayVal - baselineMedian;
  if (delta === 0) return "matches typical";
  const sign = delta > 0 ? "+" : "−";
  return `${sign}${fmt(Math.abs(delta))} vs typical`;
}

/** Build the per-category-root totals map a single day's events. */
function rootTotals(events: readonly CategorizedEvent[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const ev of events) {
    const root = categoryRoot(ev.category);
    out[root] = (out[root] ?? 0) + ev.duration;
  }
  return out;
}

export function KpiStrip() {
  const { data: today, isLoading: todayLoading } = useCategorizedEvents();
  const { data: hourly, isLoading: hourlyLoading } = useHourly();
  const { data: afk, isLoading: afkLoading } = useAfkSummary(true);
  const { data: trailing, isLoading: trailingLoading } = useTrailingDays(8);

  // Today's derived metrics
  const todayEvents = today ?? [];
  const activeSec = afk?.active_seconds ?? 0;
  const afkAvailable = afk != null;
  const longest = longestFocus(todayEvents);
  const window = bestWindow(todayEvents);
  const focusSpark = focusByHour(todayEvents);
  const activeSpark = (hourly ?? []).map((h) => h.duration);
  const cad = cadence(todayEvents, afk?.intervals ?? []);

  // Trailing-7 baselines (skip index 0 which is today itself)
  const past = useMemo(
    () => (trailing ?? []).filter((d) => d.daysAgo > 0 && d.events != null),
    [trailing],
  );
  const pastActiveSec = past.map((d) => d.activeSec ?? 0);

  const activeStats = trailingStats(pastActiveSec, pastActiveSec);
  const longestStats = trailingStats(
    past.map((d) => longestFocus(d.events ?? []).seconds),
    pastActiveSec,
  );
  const bestWindowStats = trailingStats(
    past.map((d) => bestWindow(d.events ?? [])?.seconds ?? 0),
    pastActiveSec,
  );

  // Pattern shift: per-category-root totals across past days vs today
  const todayRoots = useMemo(() => rootTotals(todayEvents), [todayEvents]);
  const trailingRootData = useMemo(
    () =>
      past.map((d) => ({
        totals: rootTotals(d.events ?? []),
        activeSec: d.activeSec ?? 0,
      })),
    [past],
  );
  const shift = useMemo(
    () => dominantShift({ today: todayRoots, trailing: trailingRootData }),
    [todayRoots, trailingRootData],
  );

  const baselineDaysReady = activeStats.effectiveDays;
  const baselinePlaceholder =
    baselineDaysReady === 0
      ? `building baseline (${past.length}/7 days)`
      : null;

  const loading =
    todayLoading || hourlyLoading || afkLoading || trailingLoading;

  return (
    <section className="grid grid-cols-5 gap-1.5 rounded-xl bg-secondary p-1.5">
      {/* 1. Active */}
      <KpiCard
        icon={Activity03Icon}
        label="Active time"
        value={afkAvailable ? fmtDuration(activeSec) : "—"}
        sub={
          baselinePlaceholder ??
          vsTypical(activeSec, activeStats.median, activeStats.effectiveDays, fmtDuration) ??
          "today"
        }
        spark={{
          values: activeSpark,
          configKey: "active",
          color: "var(--chart-1)",
          format: fmtDuration,
        }}
        loading={loading}
        emptyHint={!afkAvailable ? "no AFK bucket — set up tracker" : undefined}
        tip="Total time you weren't AFK today, with the median active total of the trailing 7 days as baseline."
      />

      {/* 2. Best Window */}
      <KpiCard
        icon={Target02Icon}
        label="Best window"
        value={
          window
            ? `${String(window.startHour).padStart(2, "0")}–${String(window.endHour).padStart(2, "0")}`
            : "—"
        }
        sub={
          window
            ? (baselinePlaceholder ??
              vsTypical(
                window.seconds,
                bestWindowStats.median,
                bestWindowStats.effectiveDays,
                fmtDuration,
              ) ??
              `${fmtDuration(window.seconds)} focused`)
            : "no focused window yet"
        }
        spark={{
          values: focusSpark,
          configKey: "bestwin",
          color: "var(--chart-2)",
          format: fmtDuration,
          shape: "bar",
          markRange: window ? [window.startHour, window.endHour - 1] : null,
        }}
        loading={loading}
        tip="Densest 3-hour window of focused stretches today (≥2m on a single category root). Highlighted bars show that window."
      />

      {/* 3. Longest stretch */}
      <KpiCard
        icon={RocketIcon}
        label="Longest stretch"
        value={longest.seconds > 0 ? fmtDuration(longest.seconds) : "—"}
        sub={
          longest.seconds > 0
            ? (vsTypical(
                longest.seconds,
                longestStats.median,
                longestStats.effectiveDays,
                fmtDuration,
              ) ?? `in ${longest.root ?? ""}`)
            : "no focused stretches yet"
        }
        spark={{
          values: focusSpark,
          configKey: "longest",
          color: "var(--chart-2)",
          format: fmtDuration,
          shape: "bar",
        }}
        loading={loading}
        emptyHint={
          activeSec > 0 && longest.seconds === 0
            ? "No focused runs ≥ 2m yet"
            : undefined
        }
        tip="Longest uninterrupted run on a single category root today (≥2m), with the trailing-7-day median as baseline."
      />

      {/* 4. Cadence */}
      <KpiCard
        icon={Sun01Icon}
        label="Cadence"
        value={cad.start ? fmtClock(cad.start) : "—"}
        sub={
          cad.start
            ? cadenceSub(cad.start, cad.end, cad.idleGaps)
            : "no activity yet"
        }
        spark={{
          values: activeSpark,
          configKey: "cadence",
          color: "var(--chart-4)",
          format: fmtDuration,
          markX: cad.start ? cad.start.getHours() : null,
        }}
        loading={loading}
        tip="Start → end of your day plus idle gaps ≥ 10min. Dashed line marks your start hour."
      />

      {/* 5. Pattern shift */}
      <KpiCard
        icon={Clock01Icon}
        label="Pattern shift"
        value={shift ? formatShiftValue(shift.deltaSec) : "—"}
        sub={
          shift
            ? `${shift.category} vs typical`
            : (baselinePlaceholder ?? "no notable shifts")
        }
        loading={loading}
        emptyHint={
          shift && shift.zScore !== 0
            ? `Z-score ${shift.zScore >= 0 ? "+" : ""}${shift.zScore.toFixed(1)}`
            : undefined
        }
        tip="The category whose absolute deviation from your trailing-7-day median is largest today. Pulse is observational; this is what's notable, not a target to chase."
      />
    </section>
  );
}

function cadenceSub(
  _start: Date,
  end: Date | null,
  gaps: number,
): string {
  const endStr = end ? `→ ${fmtClock(end)}` : "→ now";
  if (gaps === 0) return endStr;
  return `${endStr} · ${gaps} idle gap${gaps === 1 ? "" : "s"}`;
}

function formatShiftValue(deltaSec: number): string {
  const sign = deltaSec > 0 ? "+" : "−";
  return `${sign}${fmtDuration(Math.abs(deltaSec))}`;
}

// firstActivity is no longer used directly in this component; kept exported
// from kpi.ts for tests and future use.
void firstActivity;
