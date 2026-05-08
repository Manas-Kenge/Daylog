/**
 * 3 KPI cards. Discovery-shaped, not score-shaped (PLAN.md §1.0).
 *
 * Returns a Fragment of three cards so the parent grid (Overview's
 * Timeline + 3 KPIs row) can lay them out as direct grid children.
 *
 * Each card's "vs typical" sub-line is gated on ≥1 effective baseline
 * day; below that we show a build-up placeholder.
 */

import { Area, AreaChart, Bar, BarChart, Cell, ReferenceLine, Tooltip } from "recharts";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Activity03Icon,
  RocketIcon,
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
import { useHourly, useKpi } from "@/hooks/useAw";
import { fmtDuration } from "@/lib/format";
import { useId, type ReactNode } from "react";

interface SparkProps {
  values: number[];
  configKey: string;
  color: string;
  format: (v: number) => string;
  /** Single index to mark visually. Area: vertical dashed line. Bar:
   *  brighter fill at that index. */
  markX?: number | null;
  /** Inclusive range [start, end] to highlight in a bar spark. Used by
   *  the Best Window card to show its 3-hour band. */
  markRange?: [number, number] | null;
  shape?: "area" | "bar";
}

interface KpiCardProps {
  icon: IconSvgElement;
  label: string;
  value: ReactNode;
  /** A short string is rendered with `truncate`; a ReactNode (multi-line
   *  block) is rendered as-is so callers can compose their own layout. */
  sub?: string | ReactNode;
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
      <Card size="sm" className="flex h-full flex-col gap-0 overflow-hidden py-0">
        <Skeleton className="min-h-[68px] flex-1 rounded-none" />
        <div className="flex items-start justify-between gap-3 px-3 pt-2.5 pb-3">
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
    <Card size="sm" className="flex h-full flex-col gap-0 overflow-hidden py-0">
      <div className="-mb-px min-h-[68px] flex-1">
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

      <div className="flex items-start justify-between gap-3 px-3 pt-2.5">
        <div className="min-w-0 flex-1">
          <div
            className="flex items-center gap-1.5 truncate text-[0.625rem] font-medium uppercase tracking-wider text-muted-foreground"
            title={tip}
          >
            <HugeiconsIcon icon={icon} size={11} />
            <span className="truncate">{label}</span>
          </div>
          {sub != null && (
            typeof sub === "string" ? (
              <div className="mt-0.5 truncate text-muted-foreground">{sub}</div>
            ) : (
              <div className="mt-0.5 text-muted-foreground">{sub}</div>
            )
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

/** Active/AFK split bar — slim two-segment bar showing active% vs afk%. */
function ActiveAfkBar({ activeRatio }: { activeRatio: number }) {
  const a = Math.max(0, Math.min(1, activeRatio));
  return (
    <div className="flex h-1 overflow-hidden rounded-sm bg-secondary">
      <div
        className="h-full bg-foreground"
        style={{ width: `${(a * 100).toFixed(1)}%` }}
      />
      <div
        className="h-full bg-muted-foreground/30"
        style={{ width: `${((1 - a) * 100).toFixed(1)}%` }}
      />
    </div>
  );
}

export function KpiStrip() {
  const { data: kpi, isLoading: kpiLoading } = useKpi();
  const { data: hourly, isLoading: hourlyLoading } = useHourly();

  const activeSec = kpi?.active_secs ?? 0;
  const afkSec = kpi?.afk_secs ?? 0;
  const trackedSec = activeSec + afkSec;
  const activeRatio = kpi?.active_ratio ?? 0;
  // Once kpi resolves, AFK is "available" if the tracker reported anything.
  // The Rust side returns zeros when the AFK bucket is missing entirely.
  const afkAvailable = kpi != null && trackedSec > 0;
  const longest = kpi?.longest_stretch ?? null;
  const window = kpi?.best_window ?? null;
  const focusSpark = kpi?.focus_by_hour ?? new Array(24).fill(0);
  const activeSpark = (hourly ?? []).map((h) => h.duration);

  const activeBaseline = kpi?.active_baseline;
  const longestBaseline = kpi?.longest_baseline;
  const bestWindowBaseline = kpi?.best_window_baseline;
  const baselineDaysReady = activeBaseline?.effective_days ?? 0;
  const baselinePlaceholder =
    kpi != null && baselineDaysReady === 0
      ? `building baseline (${baselineDaysReady}/7 days)`
      : null;

  const loading = kpiLoading || hourlyLoading;

  const activeVsTypical =
    activeBaseline
      ? vsTypical(activeSec, activeBaseline.median, activeBaseline.effective_days, fmtDuration)
      : null;

  return (
    <>
      <KpiCard
        icon={Activity03Icon}
        label="Active / AFK"
        value={afkAvailable ? fmtDuration(activeSec) : "—"}
        sub={
          afkAvailable ? (
            <div className="flex flex-col gap-1.5">
              <span className="truncate">
                {baselinePlaceholder ?? activeVsTypical ?? "today"}
              </span>
              <ActiveAfkBar activeRatio={activeRatio} />
              <span className="truncate font-mono tabular-nums text-[0.625rem] text-muted-foreground/80">
                afk {fmtDuration(afkSec)} · tracked {fmtDuration(trackedSec)}
              </span>
            </div>
          ) : (
            "—"
          )
        }
        spark={{
          values: activeSpark,
          configKey: "active",
          color: "var(--chart-1)",
          format: fmtDuration,
        }}
        loading={loading}
        emptyHint={!afkAvailable ? "no AFK bucket — set up tracker" : undefined}
        tip="Total active time today (you vs AFK), with the trailing-7-day median active total as baseline."
      />

      <KpiCard
        icon={Target02Icon}
        label="Best window"
        value={
          window
            ? `${String(window.start_hour).padStart(2, "0")}:00–${String(window.end_hour).padStart(2, "0")}:00`
            : "—"
        }
        sub={
          window
            ? (baselinePlaceholder ??
              (bestWindowBaseline
                ? vsTypical(
                    window.seconds,
                    bestWindowBaseline.median,
                    bestWindowBaseline.effective_days,
                    fmtDuration,
                  )
                : null) ??
              `${fmtDuration(window.seconds)} focused`)
            : "no focused window yet"
        }
        spark={{
          values: focusSpark,
          configKey: "bestwin",
          color: "var(--chart-2)",
          format: fmtDuration,
          shape: "bar",
          markRange: window ? [window.start_hour, window.end_hour - 1] : null,
        }}
        loading={loading}
        tip="Densest 3-hour window of focused stretches today (≥2m on a single category root). Highlighted bars show that window."
      />

      <KpiCard
        icon={RocketIcon}
        label="Longest stretch"
        value={longest && longest.seconds > 0 ? fmtDuration(longest.seconds) : "—"}
        sub={
          longest && longest.seconds > 0
            ? ((longestBaseline
                ? vsTypical(
                    longest.seconds,
                    longestBaseline.median,
                    longestBaseline.effective_days,
                    fmtDuration,
                  )
                : null) ?? `in ${longest.category_root}`)
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
          activeSec > 0 && (longest?.seconds ?? 0) === 0
            ? "No focused runs ≥ 2m yet"
            : undefined
        }
        tip="Longest uninterrupted run on a single category root today (≥2m), with the trailing-7-day median as baseline."
      />
    </>
  );
}
