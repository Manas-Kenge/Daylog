/**
 * 5-up KPI tray. The five most-asked questions about a tracked day:
 *  1. Active   — total active seconds (vs yesterday)
 *  2. Productive — time in the "Work" root (configurable later)
 *  3. Longest focus — biggest uninterrupted run on a single category
 *  4. Started — first event of the day
 *  5. Peak hour — hour of day with most activity
 *
 * Sparklines render through shadcn ChartContainer with config-driven
 * colors and tooltips, so the visual pipeline matches TopCategories.
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
  useAfkTodayVsYesterday,
  useCategorizedEvents,
  useHourly,
} from "@/hooks/useAw";
import { fmtClock, fmtDuration, fmtPercent } from "@/lib/format";
import {
  firstActivity,
  focusByHour,
  longestFocus,
  peakHour,
  productiveByHour,
  productiveSeconds,
} from "@/lib/kpi";
import { useId, type ReactNode } from "react";

interface SparkProps {
  values: number[];
  configKey: string;
  color: string;
  format: (v: number) => string;
  shape?: "area" | "bar";
  /** x-index (0-23) to mark visually. Area: vertical dashed line at that x.
   *  Bar: that bar gets a brighter fill (foreground color). */
  markX?: number | null;
}

interface KpiCardProps {
  icon: IconSvgElement;
  label: string;
  value: ReactNode;
  /** Small sub-label under the value, replaces the static peak chip. */
  sub?: ReactNode;
  delta?: { text: string; tone: "up" | "down" | "flat" };
  spark?: SparkProps;
  loading?: boolean;
  /** Hint shown muted under value when value is empty/zero, e.g.
   *  "No matched 'Work' time — check category rules." */
  emptyHint?: string;
  /** Tooltip on the icon. Useful for explaining metric definitions. */
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

      {/* 2×2 block. Top: label / value. Bottom: sub / trend.
          Icon inlines with the label so the metric retains identity
          without claiming a third column. */}
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

export function KpiStrip() {
  const { today: afkToday, yesterday: afkYest } = useAfkTodayVsYesterday();
  const { data: hourly, isLoading: hourlyLoading } = useHourly();
  const { data: categorized, isLoading: catzdLoading } = useCategorizedEvents();

  // 1. Active
  const activeSec = afkToday.data?.active_seconds ?? 0;
  const yestActiveSec = afkYest.data?.active_seconds ?? 0;
  const activeDelta = activeSec - yestActiveSec;
  const activeSpark = (hourly ?? []).map((h) => h.duration);

  // 2. Productive
  const productiveSec = categorized ? productiveSeconds(categorized) : 0;
  const productiveSpark = categorized ? productiveByHour(categorized) : [];
  const productivePct = activeSec > 0 ? productiveSec / activeSec : 0;

  // 3. Longest focus + per-hour focused minutes (only counts time in qualifying runs)
  const focus = categorized ? longestFocus(categorized) : { seconds: 0, root: null };
  const focusSpark = categorized ? focusByHour(categorized) : [];

  // 4. Started at — marker x = the hour the day began
  const started = categorized ? firstActivity(categorized) : null;
  const startedHour = started ? started.getHours() : null;

  // 5. Peak hour — marker x = the peak hour itself
  const peak = hourly ? peakHour(hourly) : null;
  const peakSpark = (hourly ?? []).map((h) => h.duration);

  return (
    <section className="grid grid-cols-5 gap-1.5 rounded-xl bg-secondary p-1.5">
      <KpiCard
        icon={Activity03Icon}
        label="Active time"
        value={fmtDuration(activeSec)}
        sub={
          afkYest.data
            ? `vs yesterday ${fmtDuration(yestActiveSec)}`
            : "today"
        }
        delta={
          afkYest.data
            ? {
                text: `${activeDelta >= 0 ? "↑" : "↓"} ${fmtDuration(Math.abs(activeDelta))}`,
                tone: activeDelta > 0 ? "up" : activeDelta < 0 ? "down" : "flat",
              }
            : undefined
        }
        spark={{
          values: activeSpark,
          configKey: "active",
          color: "var(--chart-1)",
          format: fmtDuration,
        }}
        loading={afkToday.isLoading || hourlyLoading}
        tip="Total time you weren't AFK today"
      />

      <KpiCard
        icon={RocketIcon}
        label="Productive time"
        value={fmtDuration(productiveSec)}
        sub={
          activeSec > 0
            ? `${fmtPercent(productivePct)} of active`
            : "no active time yet"
        }
        spark={{
          values: productiveSpark,
          configKey: "productive",
          color: "var(--chart-1)",
          format: fmtDuration,
        }}
        loading={catzdLoading}
        emptyHint={
          activeSec > 0 && productiveSec === 0
            ? "No 'Work' time — set up category rules"
            : undefined
        }
        tip="Time categorized under 'Work'. Edit rules in Settings."
      />

      <KpiCard
        icon={Target02Icon}
        label="Longest focus"
        value={focus.seconds > 0 ? fmtDuration(focus.seconds) : "—"}
        sub={focus.root ? `in ${focus.root}` : "no focused runs"}
        spark={{
          values: focusSpark,
          configKey: "focus",
          color: "var(--chart-2)",
          format: fmtDuration,
          shape: "bar",
        }}
        loading={catzdLoading}
        emptyHint={
          activeSec > 0 && focus.seconds === 0
            ? "No focused runs ≥ 2m yet"
            : undefined
        }
        tip="Longest uninterrupted run on a single category root (≥2m). Bars show focused minutes per hour."
      />

      <KpiCard
        icon={Sun01Icon}
        label="Started today"
        value={started ? fmtClock(started) : "—"}
        sub={started ? agoLabel(started) : "no activity yet"}
        spark={{
          values: activeSpark,
          configKey: "started",
          color: "var(--chart-4)",
          format: fmtDuration,
          markX: startedHour,
        }}
        loading={catzdLoading || hourlyLoading}
        tip="Time of your first categorized event today. Dashed line marks that hour against today's active flow."
      />

      <KpiCard
        icon={Clock01Icon}
        label="Peak hour"
        value={peak ? `${String(peak.hour).padStart(2, "0")}:00` : "—"}
        sub={peak ? `${fmtDuration(peak.seconds)} active` : "no peak yet"}
        spark={{
          values: peakSpark,
          configKey: "peak",
          color: "var(--chart-5)",
          format: fmtDuration,
          shape: "bar",
          markX: peak?.hour ?? null,
        }}
        loading={hourlyLoading}
        tip="Hour of day with the most active time. Highlighted bar = peak."
      />
    </section>
  );
}

function agoLabel(d: Date): string {
  const min = Math.floor((Date.now() - d.getTime()) / 60_000);
  if (min < 60) return `${min}m ago`;
  const h = Math.floor(min / 60);
  const m = min % 60;
  return m > 0 ? `${h}h ${m}m ago` : `${h}h ago`;
}
